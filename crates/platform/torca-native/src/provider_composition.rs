//! Concrete provider composition lives here rather than in the native runtime
//! bootstrap.  The bootstrap only selects a deployment-ready provider and
//! receives provider-neutral runtime parts.

#[cfg(feature = "provider-iroh")]
pub(crate) mod iroh;

use std::sync::Arc;
use std::{path::PathBuf, time::Duration};

use torca_client_engine::EngineHandle;
use torca_connectivity::ConnectivityObserver;
use torca_crypto::ProtectedSecretStore;
use torca_foundation::ProviderId;
use torca_pairing_coordinator::{PairingApprovalPort, PairingPeerSecretStore};
use torca_provider_api::{ProviderDeploymentProfile, ProviderDescriptor, ProviderRouting};
use torca_radio_adapters::RadioMediaSystemFactory;
use torca_runtime::{CommunicationLifecycle, PairingDriver, RendezvousProbe};
use torca_transport_api::{CommissioningObserver, PeerTransportFactory};

use crate::composition::NativeCompositionError;

/// Dependencies supplied by the native host to the provider-owned pairing
/// implementation. They deliberately carry no Tor, onion or relay type.
#[allow(dead_code)]
pub(crate) struct ProviderPairingInputs {
    pub engine: EngineHandle,
    pub approval: Box<dyn PairingApprovalPort + Send>,
    pub peer_secrets: Box<dyn PairingPeerSecretStore + Send>,
    pub connectivity: ConnectivityObserver,
}

/// Creates pairing for the selected communication provider.
///
/// A provider may use a rendezvous service, signaling service or a local
/// out-of-band mechanism. The process runtime never needs to know which.
pub(crate) trait ProviderPairingFactory: Send {
    fn build(
        self: Box<Self>,
        inputs: ProviderPairingInputs,
    ) -> Result<Box<dyn PairingDriver>, NativeCompositionError>;
}

/// Provider-owned parts consumed by the generic native runtime composition.
pub(crate) struct ProviderComponents {
    /// Provider identity is carried with the composed bundle so the runtime
    /// cannot accidentally publish metadata for a different implementation
    /// than the one selected by deployment.
    pub provider: ProviderId,
    pub lifecycle: Box<dyn CommunicationLifecycle>,
    pub peer_transport_factory: Box<dyn PeerTransportFactory>,
    pub routing: Arc<dyn ProviderRouting>,
    pub pairing_factory: Box<dyn ProviderPairingFactory>,
    pub rendezvous_probe: Option<Arc<dyn RendezvousProbe>>,
    pub radio_media_factory: Box<dyn RadioMediaSystemFactory>,
}

/// Deployment-neutral inputs for the selected provider's commissioning.
/// `rendezvous_endpoint` is optional because direct providers may instead
/// obtain their signaling details through a platform adapter.
#[allow(dead_code)]
pub(crate) struct ProviderCompositionInputs {
    pub data_dir: PathBuf,
    /// Provider-owned secret namespace. The selected adapter may persist an
    /// endpoint/signalling identity here; other providers never see it.
    pub provider_secret_store: Box<dyn ProtectedSecretStore>,
    pub rendezvous_endpoint: Option<(String, u16)>,
    pub startup_timeout: Duration,
    pub now: torca_foundation::Timestamp,
    pub bootstrap_observer: CommissioningObserver,
}

/// Compile-time native provider plugin. The trait owns provider metadata and
/// composition, while the registry below only maps a validated identifier to
/// one implementation included in this artifact.
pub(crate) trait NativeCommunicationProviderPlugin: Send + Sync {
    fn id(&self) -> ProviderId;
    fn descriptor(&self) -> ProviderDescriptor;
    fn deployment_profile(&self) -> ProviderDeploymentProfile;
    fn compose(
        &self,
        inputs: ProviderCompositionInputs,
    ) -> Result<ProviderComponents, NativeCompositionError>;
}

fn provider_plugin(id: &ProviderId) -> Option<&'static dyn NativeCommunicationProviderPlugin> {
    match id.as_str() {
        #[cfg(feature = "provider-iroh")]
        "iroh" => Some(&iroh::PLUGIN),
        _ => None,
    }
}

/// The only native provider-selection boundary. The process runtime consumes
/// its neutral result and never constructs a Tor/onion/relay component.
pub(crate) fn compose_selected_provider(
    provider_id: ProviderId,
    inputs: ProviderCompositionInputs,
) -> Result<ProviderComponents, NativeCompositionError> {
    let plugin = provider_plugin(&provider_id).ok_or_else(|| {
        NativeCompositionError::new(format!(
            "communication provider '{}' is not included in this native artifact",
            provider_id
        ))
    })?;
    let profile = plugin.deployment_profile();
    if profile.commissioning_service.requires_endpoint() && inputs.rendezvous_endpoint.is_none() {
        return Err(NativeCompositionError::new(
            "selected communication provider requires a rendezvous endpoint",
        ));
    }
    let components = plugin.compose(inputs)?;
    if plugin.id() != provider_id || plugin.descriptor().id != provider_id {
        return Err(NativeCompositionError::new("provider plugin metadata mismatch"));
    }
    if components.provider != provider_id || components.lifecycle.provider_id() != provider_id {
        return Err(NativeCompositionError::new(format!(
            "provider composition mismatch: selected={}, bundle={}, lifecycle={}",
            provider_id,
            components.provider,
            components.lifecycle.provider_id(),
        )));
    }
    Ok(components)
}

/// Reads only configuration owned by the selected provider. Platform services
/// intentionally do not parse or retain another provider's deployment
/// endpoint: a direct provider must never inherit Tor's onion configuration.
pub(crate) fn compiled_rendezvous_endpoint(
    _provider: ProviderId,
) -> Result<Option<(String, u16)>, NativeCompositionError> {
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::{ProviderCompositionInputs, compose_selected_provider};
    use torca_foundation::ProviderId;

    #[test]
    fn unavailable_provider_is_rejected_at_the_single_selection_boundary() {
        let result = compose_selected_provider(
            ProviderId::new("memory").expect("static provider id"),
            ProviderCompositionInputs {
                data_dir: std::env::temp_dir().join("torca-provider-composition-test"),
                provider_secret_store: Box::new(
                    torca_crypto::InMemoryProtectedSecretStore::default(),
                ),
                rendezvous_endpoint: None,
                startup_timeout: std::time::Duration::from_secs(1),
                now: torca_foundation::Timestamp::UNIX_EPOCH,
                bootstrap_observer: std::sync::Arc::new(|_| {}),
            },
        );

        assert!(result.is_err());
    }
}
