//! WebRTC DataChannel adapter for the provider-neutral Torca peer protocol.
//!
//! The actual WebRTC implementation is platform-owned (Android/desktop). It
//! supplies the small `WebRtcDataChannel` boundary below; no SDP, ICE or
//! platform callback types leak into the application protocol.

use std::sync::Arc;
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
pub use host_bridge::{SignalingExchange, WebRtcHostBridge};

pub use torca_transport_api::{WebRtcDataChannel, WebRtcSessionProvider};

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
        let response =
            self.provider.exchange(&encoded, timeout).map_err(|_| RelayTransportError {
                kind: RelayTransportFailureKind::Unavailable,
                request_was_sent: false,
            })?;
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
}

impl WebRtcTransportFactory {
    pub fn new(provider: Arc<dyn WebRtcSessionProvider>) -> Self {
        Self { provider }
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
            .map(|channel| Box::new(WebRtcTransport::new(channel)) as Box<dyn PeerTransport + Send>)
            .map_err(|_| TransportFactoryError::Protocol)
    }

    fn set_waker(&self, waker: Arc<dyn Fn() + Send + Sync>) -> Result<(), TransportFactoryError> {
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
    use std::sync::Mutex;

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

    #[test]
    fn host_bridge_exposes_platform_events_without_sdk_types() {
        let bridge = Arc::new(
            WebRtcHostBridge::new(vec![7, 8], Arc::new(|request, _| Ok(request.to_vec())))
                .expect("valid host bridge"),
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
    }
}
