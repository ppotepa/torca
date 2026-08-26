use std::sync::Arc;

use torca_crypto::RustPairingCrypto;
use torca_pairing_coordinator::{PairingCoordinator, PairingRuntime};
use torca_pairing_driver::{PairingTransportRoute, RuntimePairingDriver};
use torca_radio_adapters::UnsupportedRadioMediaSystemFactory;
use torca_rendezvous_client::RendezvousClient;
use torca_runtime::PairingDriver;
use torca_transport_api::{TransportKind, WebRtcSessionProvider, WebRtcSignalingProvider};
use torca_transport_webrtc::{WebRtcLifecycle, WebRtcSignalingTransport, WebRtcTransportFactory};

use crate::composition::NativeCompositionError;
use crate::provider_composition::{
    ProviderComponents, ProviderPairingFactory, ProviderPairingInputs,
};

/// Compose the platform-owned WebRTC data-channel provider.
///
/// WebRTC media/session negotiation is deliberately injected by the host. The
/// generic runtime can therefore use the provider without depending on SDP,
/// ICE or a particular Android/desktop WebRTC binding. Pairing still requires
/// an external signaling service; until that service is registered this
/// factory fails explicitly instead of silently falling back to Tor.
pub(crate) fn compose(
    session: Arc<dyn WebRtcSessionProvider>,
    signaling: Arc<dyn WebRtcSignalingProvider>,
) -> Result<ProviderComponents, NativeCompositionError> {
    let bootstrap = session.pairing_bootstrap_descriptor().map_err(|error| {
        NativeCompositionError::new(format!("encode WebRTC bootstrap: {error}"))
    })?;
    let route_hint = session
        .local_endpoint_hint()
        .map_err(|error| NativeCompositionError::new(format!("read WebRTC route hint: {error}")))?;
    Ok(ProviderComponents {
        provider: TransportKind::WebRtc,
        lifecycle: Box::new(WebRtcLifecycle::new(Arc::clone(&session))),
        peer_transport_factory: Box::new(WebRtcTransportFactory::new(session)),
        pairing_factory: Box::new(WebRtcPairingFactory { signaling, bootstrap, route_hint }),
        rendezvous_probe: None,
        radio_media_factory: Box::new(UnsupportedRadioMediaSystemFactory::new(
            TransportKind::WebRtc,
        )),
    })
}

struct WebRtcPairingFactory {
    signaling: Arc<dyn WebRtcSignalingProvider>,
    bootstrap: torca_pairing_protocol::PairingBootstrapDescriptor,
    route_hint: Vec<u8>,
}

impl ProviderPairingFactory for WebRtcPairingFactory {
    fn build(
        self: Box<Self>,
        inputs: ProviderPairingInputs,
    ) -> Result<Box<dyn PairingDriver>, NativeCompositionError> {
        let rendezvous = RendezvousClient::new(
            WebRtcSignalingTransport::new(self.signaling),
            std::time::Duration::from_secs(15),
        );
        let coordinator = PairingCoordinator::new(rendezvous, RustPairingCrypto::new());
        let mut runtime = PairingRuntime::new(
            coordinator,
            inputs.engine.clone(),
            inputs.approval,
            inputs.peer_secrets,
        );
        runtime.restore_active_sessions().map_err(|_| {
            NativeCompositionError::new("restore active WebRTC pairing sessions failed")
        })?;
        let route = self.route_hint;
        let bootstrap = self.bootstrap;
        let driver = RuntimePairingDriver::new(
            runtime,
            inputs.engine,
            Box::new(move || Ok(Some(PairingTransportRoute::new("webrtc", route.clone())))),
        )
        .with_bootstrap_source(Box::new(move || Ok(Some(bootstrap.clone()))));
        Ok(Box::new(driver))
    }
}
