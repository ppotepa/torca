use std::sync::Arc;
use torca_crypto::RustPairingCrypto;
use torca_pairing_coordinator::{PairingCoordinator, PairingRuntime};
use torca_pairing_driver::{PairingTransportRoute, RuntimePairingDriver};
use torca_runtime::{PairingDriver, RuntimeDriverError};
use torca_transport_api::TransportKind;
use torca_transport_iroh::{IrohComposition, IrohPairingService, provider_runtime};

use crate::composition::NativeCompositionError;
use crate::provider_composition::{
    ProviderComponents, ProviderPairingFactory, ProviderPairingInputs,
};

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
    let endpoint = composition.pairing.endpoint().clone();
    Ok(ProviderComponents {
        provider: TransportKind::Iroh,
        lifecycle: Box::new(composition.lifecycle),
        peer_transport_factory: Box::new(composition.transport_factory),
        pairing_factory: Box::new(IrohPairingFactory { service: composition.pairing, endpoint }),
        rendezvous_probe: None,
        radio_media_factory: Box::new(composition.radio_media_factory),
    })
}

struct IrohPairingFactory {
    service: IrohPairingService,
    endpoint: torca_transport_iroh::ProviderEndpoint,
}

/// An Iroh endpoint id by itself is not a dialable invitation.  The address
/// lookup/relay/direct discovery task may populate transport addresses after
/// the endpoint has been bound.  Treat that short window as retryable instead
/// of emitting a QR which can only be saved locally and can never be joined.
fn encode_dialable_endpoint(
    endpoint: &torca_transport_iroh::ProviderEndpoint,
) -> Result<Vec<u8>, RuntimeDriverError> {
    let address = endpoint.addr();
    if address.is_empty() {
        return Err(RuntimeDriverError::Pending);
    }
    torca_transport_iroh::encode_endpoint_addr(&address)
        .map_err(|_| RuntimeDriverError::Communication)
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
        let route_endpoint = self.endpoint.clone();
        let bootstrap_endpoint = self.endpoint;
        let driver = RuntimePairingDriver::new(
            runtime,
            inputs.engine,
            Box::new(move || {
                encode_dialable_endpoint(&route_endpoint)
                    .ok()
                    .map(|route| PairingTransportRoute::new("iroh", route))
            }),
        )
        .with_bootstrap_source(Box::new(move || {
            let payload = encode_dialable_endpoint(&bootstrap_endpoint)?;
            torca_pairing_protocol::PairingBootstrapDescriptor::new("iroh", payload)
                .map(Some)
                .map_err(|_| RuntimeDriverError::Pairing)
        }));
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
}
