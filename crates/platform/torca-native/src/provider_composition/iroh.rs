use std::sync::Arc;
use torca_crypto::RustPairingCrypto;
use torca_foundation::ProviderId;
use torca_pairing_coordinator::{PairingCoordinator, PairingRuntime};
use torca_pairing_driver::RuntimePairingDriver;
use torca_provider_api::{ProviderRoute, ProviderRouteError, ProviderRouteState, ProviderRouting};
use torca_runtime::PairingDriver;
use torca_transport_iroh::{IrohComposition, IrohPairingService, provider_runtime};

use crate::composition::NativeCompositionError;
use crate::provider_composition::{
    NativeCommunicationProviderPlugin, ProviderComponents, ProviderCompositionInputs,
    ProviderPairingFactory, ProviderPairingInputs,
};

pub(super) static PLUGIN: IrohProviderPlugin = IrohProviderPlugin;

pub(crate) struct IrohProviderPlugin;

impl NativeCommunicationProviderPlugin for IrohProviderPlugin {
    fn id(&self) -> ProviderId {
        ProviderId::new("iroh").expect("built-in provider id")
    }

    fn descriptor(&self) -> torca_provider_api::ProviderDescriptor {
        torca_provider_api::built_in_descriptor(&self.id()).expect("built-in provider descriptor")
    }

    fn deployment_profile(&self) -> torca_provider_api::ProviderDeploymentProfile {
        torca_provider_api::built_in_deployment_profile(&self.id())
            .expect("built-in provider deployment profile")
    }

    fn compose(
        &self,
        inputs: ProviderCompositionInputs,
    ) -> Result<ProviderComponents, NativeCompositionError> {
        compose(inputs.provider_secret_store)
    }
}

pub(crate) fn compose(
    mut provider_secret_store: Box<dyn torca_crypto::ProtectedSecretStore>,
) -> Result<ProviderComponents, NativeCompositionError> {
    let runtime = Arc::new(
        provider_runtime()
            .map_err(|_| NativeCompositionError::new("start Iroh provider runtime failed"))?,
    );
    let composition = IrohComposition::bind(Arc::clone(&runtime), provider_secret_store.as_mut())
        .map_err(|error| {
        NativeCompositionError::new(format!("bind Iroh provider failed: {error}"))
    })?;
    // Keep the endpoint handle and encode its address lazily. Iroh may add
    // relay/direct address information after `Endpoint::online()` completes;
    // freezing `endpoint.addr()` during composition produced QR invitations
    // that contained only the identity and could not be dialled yet.
    let endpoint = composition.pairing.endpoint_slot();
    let routing: Arc<dyn ProviderRouting> = Arc::new(IrohRouting { endpoint });
    Ok(ProviderComponents {
        provider: ProviderId::new("iroh").expect("built-in provider id"),
        lifecycle: Box::new(composition.lifecycle),
        peer_transport_factory: Box::new(composition.transport_factory),
        pairing_factory: Box::new(IrohPairingFactory {
            service: composition.pairing,
            routing: Arc::clone(&routing),
        }),
        routing,
        rendezvous_probe: None,
        radio_media_factory: Box::new(composition.radio_media_factory),
    })
}

struct IrohPairingFactory {
    service: IrohPairingService,
    routing: Arc<dyn ProviderRouting>,
}

struct IrohRouting {
    endpoint: torca_transport_iroh::ProviderEndpointSlot,
}

impl IrohRouting {
    fn endpoint_bytes(&self) -> Result<Vec<u8>, ProviderRouteError> {
        if !self.endpoint.route_is_fresh() {
            return Err(ProviderRouteError::Stale);
        }
        let address = self.endpoint.current().ok_or(ProviderRouteError::Unavailable)?.addr();
        if address.is_empty() {
            return Err(ProviderRouteError::Unavailable);
        }
        torca_transport_iroh::encode_endpoint_addr(&address)
            .map_err(|_| ProviderRouteError::Invalid)
    }

    fn pairing_endpoint_bytes(&self) -> Result<Vec<u8>, ProviderRouteError> {
        if !self.endpoint.route_is_fresh() {
            return Err(ProviderRouteError::Stale);
        }
        // Do not wait for `Endpoint::online()` here. It waits for a completed
        // relay handshake and can block the command for a mobile network
        // transition. The endpoint slot adds configured relay candidates to
        // the immutable invitation while Iroh connects in the background.
        let address = self.endpoint.address_for_pairing().ok_or(ProviderRouteError::Unavailable)?;
        if address.is_empty() {
            return Err(ProviderRouteError::Unavailable);
        }
        eprintln!(
            "torca-iroh: pairing bootstrap route direct_routes={} relay_routes={} profile={}",
            address.ip_addrs().count(),
            address.relay_urls().count(),
            self.endpoint.profile().wire_value(),
        );
        torca_transport_iroh::encode_endpoint_addr(&address)
            .map_err(|_| ProviderRouteError::Invalid)
    }
}

impl ProviderRouting for IrohRouting {
    fn route_state(&self) -> ProviderRouteState {
        if !self.endpoint.route_is_fresh() {
            ProviderRouteState::Stale
        } else if self.endpoint.current().is_some() {
            ProviderRouteState::Fresh
        } else {
            ProviderRouteState::Unavailable
        }
    }

    fn local_route(&self) -> Result<Option<ProviderRoute>, ProviderRouteError> {
        let endpoint = match self.endpoint_bytes() {
            Ok(endpoint) => endpoint,
            Err(ProviderRouteError::Unavailable) => return Ok(None),
            Err(error) => return Err(error),
        };
        ProviderRoute::new(
            ProviderId::new("iroh").expect("built-in provider id"),
            self.endpoint.route_generation(),
            endpoint,
        )
        .map(Some)
        .ok_or(ProviderRouteError::Invalid)
    }

    fn pairing_bootstrap(
        &self,
    ) -> Result<Option<torca_pairing_protocol::PairingBootstrapDescriptor>, ProviderRouteError>
    {
        let endpoint = self.pairing_endpoint_bytes()?;
        torca_pairing_protocol::PairingBootstrapDescriptor::new("iroh", endpoint)
            .map(Some)
            .map_err(|_| ProviderRouteError::Invalid)
    }
}

impl ProviderPairingFactory for IrohPairingFactory {
    fn build(
        self: Box<Self>,
        inputs: ProviderPairingInputs,
    ) -> Result<Box<dyn PairingDriver>, NativeCompositionError> {
        let coordinator = PairingCoordinator::new(self.service, RustPairingCrypto::new());
        let mut runtime = PairingRuntime::new(
            coordinator,
            inputs.engine.clone(),
            inputs.approval,
            inputs.peer_secrets,
        );
        runtime.restore_active_sessions().map_err(|_| {
            NativeCompositionError::new("restore active Iroh pairing sessions failed")
        })?;
        let driver = RuntimePairingDriver::new(runtime, inputs.engine, self.routing);
        Ok(Box::new(driver))
    }
}

#[cfg(test)]
mod tests {
    use super::compose;
    use torca_crypto::InMemoryProtectedSecretStore;

    #[test]
    fn iroh_composition_is_provider_owned_and_self_contained() {
        let result = compose(Box::new(InMemoryProtectedSecretStore::default()));
        assert!(result.is_ok(), "Iroh composition failed: {:?}", result.err());
    }

    #[test]
    fn iroh_pairing_bootstrap_is_available_after_composition() {
        let result =
            compose(Box::new(InMemoryProtectedSecretStore::default())).expect("Iroh composition");
        let bootstrap = result
            .routing
            .pairing_bootstrap()
            .expect("Iroh pairing bootstrap")
            .expect("Iroh pairing bootstrap descriptor");
        assert_eq!(bootstrap.provider(), "iroh");
        let address = torca_transport_iroh::decode_endpoint_addr(bootstrap.payload())
            .expect("decode Iroh pairing bootstrap");
        assert!(!address.is_empty(), "pairing bootstrap must contain a transport address");
    }
}
