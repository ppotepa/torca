//! WebRTC DataChannel adapter for the provider-neutral Torca peer protocol.
//!
//! The actual WebRTC implementation is platform-owned (Android/desktop). It
//! supplies the small `WebRtcDataChannel` boundary below; no SDP, ICE or
//! platform callback types leak into the application protocol.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use torca_contacts::Contact;
use torca_foundation::Timestamp;
use torca_peer_protocol::MAX_PEER_DATA_LEN;
use torca_relay_protocol::{RelayCodec, RelayRequest, RelayResponse};
use torca_rendezvous_client::{
    PairingServiceTransport, RelayTransportError, RelayTransportFailureKind,
};
use torca_runtime::{
    CommunicationLifecycle, CommunicationState, IncomingReachabilityState, RuntimeDriverError,
};
use torca_transport_api::{
    CommissioningStage, CommissioningState, EnergyClass, LatencyClass, PeerTransport,
    PeerTransportError, PeerTransportFactory, ProviderTransport, TransportCapabilities,
    TransportFactoryError, TransportKind, TransportPath, WebRtcSignalingProvider,
};

mod host_bridge;
pub use host_bridge::{SignalingExchange, SignalingReconnect, WebRtcHostBridge};

pub use torca_transport_api::{WebRtcDataChannel, WebRtcSessionProvider};

type ChannelWake = Arc<dyn Fn() + Send + Sync>;
const MAX_SIGNALING_MESSAGE: usize = 64 * 1024;

/// Adapter from a platform signaling provider to the common pairing client.
pub struct WebRtcSignalingTransport {
    provider: Arc<dyn WebRtcSignalingProvider>,
}

impl WebRtcSignalingTransport {
    pub fn new(provider: Arc<dyn WebRtcSignalingProvider>) -> Self {
        Self { provider }
    }
}

impl PairingServiceTransport for WebRtcSignalingTransport {
    fn invalidate(&mut self) {
        self.provider.invalidate();
    }

    fn reconnect(&mut self) -> Result<(), RelayTransportError> {
        self.provider.reconnect().map_err(|_| RelayTransportError {
            kind: RelayTransportFailureKind::Unavailable,
            request_was_sent: false,
        })
    }

    fn exchange(
        &mut self,
        request: &RelayRequest,
        timeout: Duration,
    ) -> Result<RelayResponse, RelayTransportError> {
        let encoded = RelayCodec::encode_request(request).map_err(|_| RelayTransportError {
            kind: RelayTransportFailureKind::InvalidResponse,
            request_was_sent: false,
        })?;
        if encoded.len() > MAX_SIGNALING_MESSAGE {
            return Err(RelayTransportError {
                kind: RelayTransportFailureKind::InvalidResponse,
                request_was_sent: false,
            });
        }
        let response =
            self.provider.exchange(&encoded, timeout).map_err(|_| RelayTransportError {
                kind: RelayTransportFailureKind::Unavailable,
                request_was_sent: false,
            })?;
        if response.len() > MAX_SIGNALING_MESSAGE {
            return Err(RelayTransportError {
                kind: RelayTransportFailureKind::InvalidResponse,
                request_was_sent: true,
            });
        }
        RelayCodec::decode_response(&response).map_err(|_| RelayTransportError {
            kind: RelayTransportFailureKind::InvalidResponse,
            request_was_sent: true,
        })
    }
}

/// Provider-owned lifecycle projection for an already registered WebRTC
/// session provider.  ICE/signalling remains platform-owned; this type only
/// exposes stable commissioning state to the generic runtime.
pub struct WebRtcLifecycle {
    provider: Arc<dyn WebRtcSessionProvider>,
    stopped: bool,
}

impl WebRtcLifecycle {
    pub fn new(provider: Arc<dyn WebRtcSessionProvider>) -> Self {
        Self { provider, stopped: false }
    }
}

impl CommunicationLifecycle for WebRtcLifecycle {
    fn provider(&self) -> TransportKind {
        TransportKind::WebRtc
    }
    fn runtime_diagnostics(&self) -> torca_transport_api::ProviderRuntimeDiagnostics {
        let commissioning = self.provider.commissioning();
        torca_transport_api::ProviderRuntimeDiagnostics {
            endpoint_active: Some(!self.stopped && commissioning.route_state.is_fresh()),
            route_fresh: Some(commissioning.route_state.is_fresh()),
            route_state: Some(commissioning.route_state),
            energy_class: Some(EnergyClass::Medium),
            ..torca_transport_api::ProviderRuntimeDiagnostics::default()
        }
    }
    fn maintenance(&mut self, _now: Timestamp) -> Result<(), RuntimeDriverError> {
        if self.stopped || !self.provider.commissioning().local_shell_ready() {
            Err(RuntimeDriverError::Communication)
        } else {
            Ok(())
        }
    }
    fn set_waker(&mut self, waker: Arc<dyn Fn() + Send + Sync>) {
        self.provider.set_waker(waker);
    }
    fn set_dormant(&mut self, _dormant: bool) -> Result<(), RuntimeDriverError> {
        Ok(())
    }
    fn refresh_route(&mut self, _now: Timestamp) -> Result<(), RuntimeDriverError> {
        self.provider.refresh_route().map_err(|_| RuntimeDriverError::RouteRefreshRequired)
    }
    fn state(&self) -> CommunicationState {
        if self.stopped {
            return CommunicationState::Stopped;
        }
        match self.provider.commissioning().step(CommissioningStage::LocalRuntime) {
            CommissioningState::Ready | CommissioningState::NotRequired => {
                CommunicationState::Ready
            }
            CommissioningState::Degraded => CommunicationState::Degraded,
            CommissioningState::Failed => CommunicationState::Failed,
            CommissioningState::Pending => CommunicationState::Starting,
        }
    }
    fn local_endpoint_summary(&self) -> Option<String> {
        self.provider.local_endpoint_hint().ok().map(|hint| format!("webrtc:{} bytes", hint.len()))
    }
    fn incoming_reachability_state(&self) -> IncomingReachabilityState {
        if self.stopped {
            return IncomingReachabilityState::Stopped;
        }
        match self.provider.commissioning().step(CommissioningStage::IncomingReachability) {
            CommissioningState::Ready => IncomingReachabilityState::Reachable,
            CommissioningState::Degraded => IncomingReachabilityState::Degraded,
            CommissioningState::Failed => IncomingReachabilityState::Failed,
            CommissioningState::Pending => IncomingReachabilityState::Unknown,
            CommissioningState::NotRequired => IncomingReachabilityState::Reachable,
        }
    }
    fn shutdown(&mut self) {
        self.stopped = true;
    }
}

/// A provider transport over one negotiated RTCDataChannel.
pub struct WebRtcTransport {
    channel: Arc<dyn WebRtcDataChannel>,
    connected: bool,
}

impl WebRtcTransport {
    pub fn new(channel: Arc<dyn WebRtcDataChannel>) -> Self {
        Self { channel, connected: false }
    }
}

impl PeerTransport for WebRtcTransport {
    fn connect(&mut self) -> Result<(), PeerTransportError> {
        self.connected = true;
        Ok(())
    }

    fn send(&mut self, payload: Vec<u8>) -> Result<(), PeerTransportError> {
        if !self.connected {
            return Err(PeerTransportError("WebRTC transport is not connected".into()));
        }
        if payload.len() > MAX_PEER_DATA_LEN {
            return Err(PeerTransportError("WebRTC payload exceeds peer limit".into()));
        }
        self.channel.send(&payload).map_err(PeerTransportError)
    }

    fn try_receive(&mut self) -> Result<Option<Vec<u8>>, PeerTransportError> {
        if !self.connected {
            return Err(PeerTransportError("WebRTC transport is not connected".into()));
        }
        self.channel.try_receive().map_err(PeerTransportError)
    }

    fn receive_timeout(
        &mut self,
        timeout: Duration,
    ) -> Result<Option<Vec<u8>>, PeerTransportError> {
        if let Some(payload) = self.try_receive()? {
            return Ok(Some(payload));
        }
        std::thread::sleep(timeout);
        self.try_receive()
    }

    fn close(&mut self) -> Result<(), PeerTransportError> {
        self.connected = false;
        self.channel.close().map_err(PeerTransportError)
    }

    fn set_waker(&mut self, waker: Arc<dyn Fn() + Send + Sync>) {
        self.channel.set_waker(waker);
    }
}

impl ProviderTransport for WebRtcTransport {
    fn kind(&self) -> TransportKind {
        TransportKind::WebRtc
    }

    fn path(&self) -> TransportPath {
        TransportPath::WebRtcDirect
    }

    fn capabilities(&self) -> TransportCapabilities {
        TransportCapabilities {
            reliable: true,
            ordered: true,
            supports_incoming: true,
            supports_direct_path: true,
            supports_relay_path: true,
            hides_peer_ip: false,
            max_frame_size: MAX_PEER_DATA_LEN,
            latency: LatencyClass::Interactive,
            energy: EnergyClass::Medium,
        }
    }
}

impl Drop for WebRtcTransport {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

/// Factory that makes WebRTC the single provider for one `PeerLink`.
pub struct WebRtcTransportFactory {
    provider: Arc<dyn WebRtcSessionProvider>,
    wake: Arc<Mutex<Option<ChannelWake>>>,
}

impl WebRtcTransportFactory {
    pub fn new(provider: Arc<dyn WebRtcSessionProvider>) -> Self {
        Self { provider, wake: Arc::new(Mutex::new(None)) }
    }

    fn install_waker(&self, channel: &Arc<dyn WebRtcDataChannel>) {
        if let Some(waker) = self.wake.lock().ok().and_then(|slot| slot.clone()) {
            channel.set_waker(waker);
        }
    }
}

impl PeerTransportFactory for WebRtcTransportFactory {
    fn kind(&self) -> TransportKind {
        TransportKind::WebRtc
    }

    fn capabilities(&self) -> TransportCapabilities {
        WebRtcTransport::new(Arc::new(UnsupportedChannel)).capabilities()
    }

    fn accept(&mut self) -> Result<Option<Box<dyn PeerTransport + Send>>, TransportFactoryError> {
        self.provider.accept().map_err(|_| TransportFactoryError::Listener).map(|channel| {
            channel.map(|channel| {
                self.install_waker(&channel);
                Box::new(WebRtcTransport::new(channel)) as Box<dyn PeerTransport + Send>
            })
        })
    }

    fn connect(
        &mut self,
        contact: &Contact,
    ) -> Result<Box<dyn PeerTransport + Send>, TransportFactoryError> {
        self.provider
            .connect(contact)
            .map(|channel| {
                self.install_waker(&channel);
                Box::new(WebRtcTransport::new(channel)) as Box<dyn PeerTransport + Send>
            })
            .map_err(|_| TransportFactoryError::Protocol)
    }

    fn set_waker(&self, waker: ChannelWake) -> Result<(), TransportFactoryError> {
        if let Ok(mut slot) = self.wake.lock() {
            *slot = Some(Arc::clone(&waker));
        }
        self.provider.set_waker(waker);
        Ok(())
    }
}

// Capability metadata does not need an active channel. This private bridge
// keeps the capability construction in one place without inventing a second
// public API surface.
struct UnsupportedChannel;

impl WebRtcDataChannel for UnsupportedChannel {
    fn send(&self, _payload: &[u8]) -> Result<(), String> {
        Err("metadata-only channel".into())
    }
    fn try_receive(&self) -> Result<Option<Vec<u8>>, String> {
        Err("metadata-only channel".into())
    }
    fn close(&self) -> Result<(), String> {
        Ok(())
    }
    fn set_waker(&self, _waker: Arc<dyn Fn() + Send + Sync>) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use torca_contacts::ContactId;

    struct FakeChannel {
        queue: Mutex<VecDeque<Vec<u8>>>,
    }

    impl WebRtcDataChannel for FakeChannel {
        fn send(&self, payload: &[u8]) -> Result<(), String> {
            self.queue.lock().map_err(|_| "poisoned".to_owned())?.push_back(payload.to_vec());
            Ok(())
        }
        fn try_receive(&self) -> Result<Option<Vec<u8>>, String> {
            Ok(self.queue.lock().map_err(|_| "poisoned".to_owned())?.pop_front())
        }
        fn close(&self) -> Result<(), String> {
            Ok(())
        }
        fn set_waker(&self, _waker: Arc<dyn Fn() + Send + Sync>) {}
    }

    struct WakerCountingChannel {
        installed: Arc<AtomicUsize>,
    }

    impl WebRtcDataChannel for WakerCountingChannel {
        fn send(&self, _payload: &[u8]) -> Result<(), String> {
            Ok(())
        }
        fn try_receive(&self) -> Result<Option<Vec<u8>>, String> {
            Ok(None)
        }
        fn close(&self) -> Result<(), String> {
            Ok(())
        }
        fn set_waker(&self, _waker: Arc<dyn Fn() + Send + Sync>) {
            self.installed.fetch_add(1, Ordering::Relaxed);
        }
    }

    #[test]
    fn data_channel_adapter_preserves_payloads_and_provider_identity() {
        let channel = Arc::new(FakeChannel { queue: Mutex::new(VecDeque::new()) });
        let mut transport = WebRtcTransport::new(channel);
        transport.connect().expect("connect");
        transport.send(vec![1, 2, 3]).expect("send");
        assert_eq!(transport.try_receive().expect("receive"), Some(vec![1, 2, 3]));
        assert_eq!(transport.kind(), TransportKind::WebRtc);
        assert_eq!(transport.path(), TransportPath::WebRtcDirect);
    }

    struct FakeSignaling;

    impl WebRtcSignalingProvider for FakeSignaling {
        fn invalidate(&self) {}

        fn reconnect(&self) -> Result<(), String> {
            Ok(())
        }

        fn exchange(&self, request: &[u8], _timeout: Duration) -> Result<Vec<u8>, String> {
            let request = RelayCodec::decode_request(request).map_err(|error| error.to_string())?;
            if matches!(request, RelayRequest::Info) {
                RelayCodec::encode_response(&RelayResponse::Info(
                    torca_relay_protocol::RelayInfo::new("test", "build", "source")
                        .map_err(|error| error.to_string())?,
                ))
                .map_err(|error| error.to_string())
            } else {
                Err("unexpected request".into())
            }
        }
    }

    #[test]
    fn signaling_adapter_uses_the_shared_pairing_wire_contract() {
        let mut transport = WebRtcSignalingTransport::new(Arc::new(FakeSignaling));
        let response = transport
            .exchange(&RelayRequest::Info, Duration::from_secs(1))
            .expect("signaling exchange");
        assert!(matches!(response, RelayResponse::Info(_)));
    }

    struct OversizedSignaling;

    impl WebRtcSignalingProvider for OversizedSignaling {
        fn invalidate(&self) {}
        fn reconnect(&self) -> Result<(), String> {
            Ok(())
        }
        fn exchange(&self, _request: &[u8], _timeout: Duration) -> Result<Vec<u8>, String> {
            Ok(vec![0; MAX_SIGNALING_MESSAGE + 1])
        }
    }

    #[test]
    fn signaling_adapter_rejects_oversized_provider_response() {
        let mut transport = WebRtcSignalingTransport::new(Arc::new(OversizedSignaling));
        let error = transport
            .exchange(&RelayRequest::Info, Duration::from_secs(1))
            .expect_err("oversized signaling response must fail");
        assert_eq!(error.kind, RelayTransportFailureKind::InvalidResponse);
        assert!(error.request_was_sent);
    }

    #[test]
    fn host_bridge_exposes_platform_events_without_sdk_types() {
        let reconnects = Arc::new(AtomicUsize::new(0));
        let reconnect_counter = Arc::clone(&reconnects);
        let bridge = Arc::new(
            WebRtcHostBridge::new(vec![7, 8], Arc::new(|request, _| Ok(request.to_vec())))
                .expect("valid host bridge")
                .with_signaling_reconnect(Arc::new(move || {
                    reconnect_counter.fetch_add(1, Ordering::Relaxed);
                    Ok(())
                })),
        );
        assert_eq!(bridge.local_endpoint_hint().expect("hint"), vec![7, 8]);
        assert_eq!(bridge.commissioning().provider, TransportKind::WebRtc);

        bridge
            .push_incoming(Arc::new(FakeChannel { queue: Mutex::new(VecDeque::new()) }))
            .expect("publish incoming channel");
        assert!(bridge.accept().expect("accept").is_some());

        let request = vec![1, 2, 3];
        assert_eq!(
            WebRtcSignalingProvider::exchange(&*bridge, &request, Duration::from_secs(1))
                .expect("signaling exchange"),
            request
        );
        WebRtcSignalingProvider::reconnect(&*bridge).expect("signaling reconnect");
        assert_eq!(reconnects.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn host_bridge_invalidates_route_until_sdk_reports_fresh_candidates() {
        let bridge = WebRtcHostBridge::new(vec![9], Arc::new(|request, _| Ok(request.to_vec())))
            .expect("valid host bridge");
        let mut commissioning = bridge.commissioning();
        commissioning.route_state = torca_transport_api::ProviderRouteState::Fresh;
        commissioning.steps[1].state = CommissioningState::Ready;
        bridge.set_commissioning(commissioning).expect("publish fresh route");

        WebRtcSessionProvider::refresh_route(&bridge).expect("refresh route");
        let refreshed = bridge.commissioning();
        assert_eq!(refreshed.route_state, torca_transport_api::ProviderRouteState::Stale);
        assert_eq!(refreshed.steps[1].state, CommissioningState::Pending);
    }

    #[test]
    fn host_bridge_propagates_runtime_wakers_to_existing_and_incoming_channels() {
        let bridge = WebRtcHostBridge::new(vec![1], Arc::new(|request, _| Ok(request.to_vec())))
            .expect("valid host bridge");
        let contact_waker_count = Arc::new(AtomicUsize::new(0));
        let incoming_waker_count = Arc::new(AtomicUsize::new(0));
        bridge
            .bind_contact(
                ContactId::from_u128(3),
                Arc::new(WakerCountingChannel { installed: Arc::clone(&contact_waker_count) }),
            )
            .expect("bind contact");
        bridge
            .push_incoming(Arc::new(WakerCountingChannel {
                installed: Arc::clone(&incoming_waker_count),
            }))
            .expect("push incoming");

        WebRtcSessionProvider::set_waker(&bridge, Arc::new(|| {}));
        assert_eq!(contact_waker_count.load(Ordering::Relaxed), 1);
        assert_eq!(incoming_waker_count.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn host_bridge_unbinds_and_closes_one_contact_channel() {
        let bridge = WebRtcHostBridge::new(vec![1], Arc::new(|request, _| Ok(request.to_vec())))
            .expect("valid host bridge");
        let contact = ContactId::from_u128(7);
        bridge
            .bind_contact(contact, Arc::new(FakeChannel { queue: Mutex::new(VecDeque::new()) }))
            .expect("bind contact");
        assert!(bridge.unbind_contact(contact).expect("unbind contact"));
        assert!(!bridge.unbind_contact(contact).expect("second unbind"));
    }

    #[test]
    fn lifecycle_refresh_delegates_to_the_registered_session_provider() {
        let refreshes = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&refreshes);
        let bridge = WebRtcHostBridge::new(vec![4], Arc::new(|request, _| Ok(request.to_vec())))
            .expect("valid host bridge");
        // The host bridge uses the session-provider refresh implementation;
        // wrap it in a tiny provider-owned callback by registering a channel
        // waker that records the lifecycle wake-up as well.
        let bridge = Arc::new(bridge);
        WebRtcSessionProvider::set_waker(
            &*bridge,
            Arc::new(move || {
                counter.fetch_add(1, Ordering::Relaxed);
            }),
        );
        let mut lifecycle = WebRtcLifecycle::new(bridge);
        lifecycle.refresh_route(Timestamp::UNIX_EPOCH).expect("refresh route");
        assert!(refreshes.load(Ordering::Relaxed) >= 1);
    }
}
