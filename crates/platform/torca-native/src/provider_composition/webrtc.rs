use std::sync::Arc;
use torca_foundation::ProviderId;

use torca_crypto::RustPairingCrypto;
use torca_pairing_coordinator::{PairingCoordinator, PairingRuntime};
use torca_pairing_driver::RuntimePairingDriver;
use torca_provider_api::{ProviderRoute, ProviderRouteError, ProviderRouteState, ProviderRouting};
use torca_radio_adapters::UnsupportedRadioMediaSystemFactory;
use torca_rendezvous_client::RendezvousClient;
use torca_runtime::PairingDriver;
use torca_transport_api::{TransportKind, WebRtcSessionProvider, WebRtcSignalingProvider};
use torca_transport_webrtc::{WebRtcLifecycle, WebRtcSignalingTransport, WebRtcTransportFactory};

use crate::composition::NativeCompositionError;
use crate::provider_composition::{
    NativeCommunicationProviderPlugin, ProviderComponents, ProviderCompositionInputs,
    ProviderPairingFactory, ProviderPairingInputs,
};

pub(super) static PLUGIN: WebRtcProviderPlugin = WebRtcProviderPlugin;

pub(crate) struct WebRtcProviderPlugin;

impl NativeCommunicationProviderPlugin for WebRtcProviderPlugin {
    fn id(&self) -> ProviderId {
        ProviderId::new("webrtc").expect("built-in provider id")
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
        _inputs: ProviderCompositionInputs,
    ) -> Result<ProviderComponents, NativeCompositionError> {
        let session = crate::runtime_composition::registered_webrtc_session_provider()?;
        let signaling = crate::runtime_composition::registered_webrtc_signaling_provider()?;
        compose(session, signaling)
    }
}

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
    let routing: Arc<dyn ProviderRouting> = Arc::new(WebRtcRouting { bootstrap, route_hint });
    Ok(ProviderComponents {
        provider: TransportKind::WebRtc,
        lifecycle: Box::new(WebRtcLifecycle::new(Arc::clone(&session))),
        peer_transport_factory: Box::new(WebRtcTransportFactory::new(session)),
        pairing_factory: Box::new(WebRtcPairingFactory {
            signaling,
            routing: Arc::clone(&routing),
        }),
        routing,
        rendezvous_probe: None,
        radio_media_factory: Box::new(UnsupportedRadioMediaSystemFactory::new(
            TransportKind::WebRtc,
        )),
    })
}

struct WebRtcPairingFactory {
    signaling: Arc<dyn WebRtcSignalingProvider>,
    routing: Arc<dyn ProviderRouting>,
}

struct WebRtcRouting {
    bootstrap: torca_pairing_protocol::PairingBootstrapDescriptor,
    route_hint: Vec<u8>,
}

impl ProviderRouting for WebRtcRouting {
    fn route_state(&self) -> ProviderRouteState {
        ProviderRouteState::Fresh
    }

    fn local_route(&self) -> Result<Option<ProviderRoute>, ProviderRouteError> {
        ProviderRoute::new(
            ProviderId::new("webrtc").expect("built-in provider id"),
            0,
            self.route_hint.clone(),
        )
        .map(Some)
        .ok_or(ProviderRouteError::Invalid)
    }

    fn pairing_bootstrap(
        &self,
    ) -> Result<Option<torca_pairing_protocol::PairingBootstrapDescriptor>, ProviderRouteError>
    {
        Ok(Some(self.bootstrap.clone()))
    }
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
        let driver = RuntimePairingDriver::new(runtime, inputs.engine, self.routing);
        Ok(Box::new(driver))
    }
}
