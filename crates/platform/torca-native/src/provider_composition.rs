//! Concrete provider composition lives here rather than in the native runtime
//! bootstrap.  The bootstrap only selects a deployment-ready provider and
//! receives provider-neutral runtime parts.

#[cfg(feature = "provider-iroh")]
pub(crate) mod iroh;
#[cfg(feature = "provider-tor")]
pub(crate) mod tor;
#[cfg(feature = "provider-webrtc")]
pub(crate) mod webrtc;

use std::sync::Arc;
use std::{path::PathBuf, time::Duration};

use torca_client_engine::EngineHandle;
use torca_connectivity::ConnectivityObserver;
use torca_crypto::ProtectedSecretStore;
use torca_pairing_coordinator::{PairingApprovalPort, PairingPeerSecretStore};
use torca_radio_adapters::RadioMediaSystemFactory;
use torca_runtime::{CommunicationLifecycle, PairingDriver, RendezvousProbe};
use torca_transport_api::{CommissioningObserver, PeerTransportFactory, TransportKind};

use crate::composition::NativeCompositionError;

/// Dependencies supplied by the native host to the provider-owned pairing
/// implementation. They deliberately carry no Tor, onion or relay type.
pub(crate) struct ProviderPairingInputs {
    pub engine: EngineHandle,
    pub approval: Box<dyn PairingApprovalPort + Send>,
    pub peer_secrets: Box<dyn PairingPeerSecretStore + Send>,
    #[cfg_attr(not(feature = "provider-tor"), allow(dead_code))]
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
    pub provider: TransportKind,
    pub lifecycle: Box<dyn CommunicationLifecycle>,
    pub peer_transport_factory: Box<dyn PeerTransportFactory>,
    pub pairing_factory: Box<dyn ProviderPairingFactory>,
    pub rendezvous_probe: Option<Arc<dyn RendezvousProbe>>,
    pub radio_media_factory: Box<dyn RadioMediaSystemFactory>,
}

/// Deployment-neutral inputs for the selected provider's commissioning.
/// `rendezvous_endpoint` is optional because direct providers may instead
/// obtain their signaling details through a platform adapter.
pub(crate) struct ProviderCompositionInputs {
    #[cfg_attr(not(feature = "provider-tor"), allow(dead_code))]
    pub data_dir: PathBuf,
    /// Provider-owned secret namespace. The selected adapter may persist an
    /// endpoint/signalling identity here; other providers never see it.
    pub provider_secret_store: Box<dyn ProtectedSecretStore>,
    pub rendezvous_endpoint: Option<(String, u16)>,
    #[cfg_attr(not(feature = "provider-tor"), allow(dead_code))]
    pub startup_timeout: Duration,
    #[cfg_attr(not(feature = "provider-tor"), allow(dead_code))]
    pub now: torca_foundation::Timestamp,
    #[cfg_attr(not(feature = "provider-tor"), allow(dead_code))]
    pub bootstrap_observer: CommissioningObserver,
}

/// The only native provider-selection boundary. The process runtime consumes
/// its neutral result and never constructs a Tor/onion/relay component.
pub(crate) fn compose_selected_provider(
    provider: TransportKind,
    mut inputs: ProviderCompositionInputs,
) -> Result<ProviderComponents, NativeCompositionError> {
    // The selected provider owns this namespace. Tor currently does not need
    // it, but consuming it here prevents the generic host from retaining a
    // secret store after composition and keeps the ownership boundary explicit.
    let _provider_secret_store = &mut inputs.provider_secret_store;
    let profile = provider.deployment_profile();
    if profile.commissioning_service.requires_endpoint() && inputs.rendezvous_endpoint.is_none() {
        return Err(NativeCompositionError::new(
            "selected communication provider requires a rendezvous endpoint",
        ));
    }
    let composed: Result<ProviderComponents, NativeCompositionError> = match provider {
        #[cfg(feature = "provider-tor")]
        TransportKind::Tor => {
            let (relay_host, relay_port) = inputs
                .rendezvous_endpoint
                .expect("validated by selected provider deployment profile");
            tor::compose(
                inputs.data_dir,
                relay_host,
                relay_port,
                inputs.startup_timeout,
                inputs.now,
                inputs.bootstrap_observer,
            )
        }
        #[cfg(not(feature = "provider-tor"))]
        TransportKind::Tor => {
            Err(NativeCompositionError::new("Tor provider is not included in this native artifact"))
        }
        #[cfg(feature = "provider-iroh")]
        TransportKind::Iroh => iroh::compose(inputs.provider_secret_store),
        #[cfg(not(feature = "provider-iroh"))]
        TransportKind::Iroh => Err(NativeCompositionError::new(
            "Iroh provider is not included in this native artifact",
        )),
        #[cfg(feature = "provider-webrtc")]
        TransportKind::WebRtc => {
            let session = crate::runtime_composition::registered_webrtc_session_provider()?;
            let signaling = crate::runtime_composition::registered_webrtc_signaling_provider()?;
            webrtc::compose(session, signaling)
        }
        #[cfg(not(feature = "provider-webrtc"))]
        TransportKind::WebRtc => Err(NativeCompositionError::new(
            "WebRTC provider is not included in this native artifact",
        )),
        TransportKind::Memory => Err(NativeCompositionError::new(format!(
            "communication provider '{}' is available only for simulated runtimes",
            provider.wire_value()
        ))),
    };
    let components = composed?;
    if components.provider != provider || components.lifecycle.provider() != provider {
        return Err(NativeCompositionError::new(format!(
            "provider composition mismatch: selected={}, bundle={}, lifecycle={}",
            provider.wire_value(),
            components.provider.wire_value(),
            components.lifecycle.provider().wire_value(),
        )));
    }
    Ok(components)
}

/// Reads only configuration owned by the selected provider. Platform services
/// intentionally do not parse or retain another provider's deployment
/// endpoint: a direct provider must never inherit Tor's onion configuration.
pub(crate) fn compiled_rendezvous_endpoint(
    provider: TransportKind,
) -> Result<Option<(String, u16)>, NativeCompositionError> {
    match provider {
        #[cfg(feature = "provider-tor")]
        TransportKind::Tor => tor::compiled_rendezvous_endpoint().map(Some),
        #[cfg(not(feature = "provider-tor"))]
        TransportKind::Tor => Ok(None),
        TransportKind::Iroh | TransportKind::WebRtc | TransportKind::Memory => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::{ProviderCompositionInputs, compose_selected_provider};
    use torca_transport_api::TransportKind;

    #[test]
    fn unavailable_provider_is_rejected_at_the_single_selection_boundary() {
        let result = compose_selected_provider(
            TransportKind::Memory,
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
