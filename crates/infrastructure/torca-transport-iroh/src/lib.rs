//! Iroh/QUIC implementation of the provider-neutral Torca transport.
//!
//! The adapter receives a configured Iroh endpoint and remote address from
//! the provider composition layer. Signalling and endpoint identity exchange
//! stay outside this crate; peer protocol and E2EE remain unchanged.

use std::collections::VecDeque;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use iroh::EndpointAddr;
use iroh::endpoint::{Connection, Endpoint, RecvStream, SendStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::runtime::Runtime;
use tokio::sync::Mutex as AsyncMutex;
use torca_contacts::Contact;
use torca_crypto::{ProtectedSecretStore, ProtectedSecretStoreError};
use torca_foundation::Timestamp;
use torca_identity::KeyId;
use torca_pairing_protocol::PairingBootstrapDescriptor;
use torca_peer_protocol::MAX_PEER_DATA_LEN;
use torca_relay_protocol::{RELAY_HEADER_LEN, RelayCodec, RelayRequest, RelayResponse};
use torca_rendezvous_client::{
    PairingServiceTransport, RelayTransportError, RelayTransportFailureKind,
};
use torca_runtime::{
    CommunicationLifecycle, CommunicationState, IncomingReachabilityState, RuntimeDriverError,
};
use torca_transport_api::{
    CommissioningStage, CommissioningState, CommissioningStep, EnergyClass, LatencyClass,
    PeerTransport, PeerTransportError, PeerTransportFactory, ProviderCommissioning,
    ProviderTransport, TransportCapabilities, TransportFactoryError, TransportKind, TransportPath,
};

mod pairing;
pub use pairing::IrohPairingService;

const ALPN: &[u8] = b"torca/peer/1";
/// ALPN used only for the short-lived provider-owned pairing service.
pub const PAIRING_ALPN: &[u8] = b"torca/pairing/1";
/// Reserved provider-owned ALPN for optional Radio media streams.
pub const RADIO_ALPN: &[u8] = b"torca/radio/1";
const MAX_FRAME: usize = MAX_PEER_DATA_LEN;

/// Provider-owned endpoint handle used only by the native composition layer
/// to lazily encode current route/bootstrap metadata. It never crosses the
/// application or FFI boundary.
pub type ProviderEndpoint = Endpoint;

/// Protected-store handle for the Iroh endpoint identity. It is independent
/// from Torca's user identity: Iroh uses it to keep its network route stable
/// across process restarts.
pub const IROH_ENDPOINT_SECRET_HANDLE: KeyId = KeyId::from_u128(0x746f7263615f69726f685f6570);
pub const IROH_PAIRING_SECRET_HANDLE: KeyId = KeyId::from_u128(0x746f7263615f70616972696e675f31);

/// Redaction-safe Iroh endpoint identity persistence failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IrohIdentityError {
    Store(ProtectedSecretStoreError),
    InvalidStoredKey,
    Bind(String),
}

impl fmt::Display for IrohIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(_) => formatter.write_str("protected Iroh endpoint identity store failed"),
            Self::InvalidStoredKey => {
                formatter.write_str("protected Iroh endpoint identity is invalid")
            }
            Self::Bind(error) => write!(formatter, "Iroh endpoint bind failed: {error}"),
        }
    }
}

/// Binds a provider-owned endpoint with a stable identity.  Native composition
/// calls this once per runtime; the application never constructs an Iroh
/// endpoint or handles its secret key directly.
fn bind_endpoint_with_handle(
    runtime: &Runtime,
    store: &mut dyn ProtectedSecretStore,
    handle: KeyId,
) -> Result<Endpoint, IrohIdentityError> {
    let secret = match store.load(handle)? {
        Some(mut bytes) => {
            let key_result: Result<[u8; 32], _> = bytes.as_slice().try_into();
            bytes.fill(0);
            let key = key_result.map_err(|_| IrohIdentityError::InvalidStoredKey)?;
            iroh::SecretKey::from_bytes(&key)
        }
        None => {
            let secret = iroh::SecretKey::generate();
            let mut bytes = secret.to_bytes();
            let result = store.insert(handle, &bytes);
            bytes.fill(0);
            result?;
            secret
        }
    };
    let local_only = std::env::var_os("TORCA_IROH_LOCAL_ONLY").is_some();
    let builder = Endpoint::builder(iroh::endpoint::presets::N0).secret_key(secret).alpns(vec![
        ALPN.to_vec(),
        PAIRING_ALPN.to_vec(),
        RADIO_ALPN.to_vec(),
    ]);
    // The laboratory runner starts several peers in one process namespace.
    // A loopback bind makes their bootstrap descriptor immediately routable
    // and deterministic without depending on the host's Wi-Fi/NAT interface.
    // Production deployments use the normal all-interface bind. Do not call
    // `clear_ip_transports` here: that disables every direct IP route, leaving
    // a provider with an endpoint identity but no usable transport when no
    // relay is configured.
    let bind_addr = if local_only { "127.0.0.1:0" } else { "0.0.0.0:0" };
    let builder =
        builder.bind_addr(bind_addr).map_err(|error| IrohIdentityError::Bind(error.to_string()))?;
    runtime.block_on(builder.bind()).map_err(|error| IrohIdentityError::Bind(error.to_string()))
}

pub fn bind_endpoint(
    runtime: &Runtime,
    store: &mut dyn ProtectedSecretStore,
) -> Result<Endpoint, IrohIdentityError> {
    bind_endpoint_with_handle(runtime, store, IROH_ENDPOINT_SECRET_HANDLE)
}

/// Builds the bounded provider-owned runtime used by native composition.
pub fn provider_runtime() -> Result<Runtime, IrohIdentityError> {
    build_provider_runtime()
}

/// Complete provider-owned communication composition for a direct Iroh
/// deployment. Pairing/signalling remains a separate service port, but the
/// endpoint identity, lifecycle and peer factory are constructed atomically so
/// a host cannot accidentally mix an Iroh endpoint with another provider.
pub struct IrohComposition {
    pub lifecycle: IrohLifecycle,
    pub transport_factory: IrohTransportFactory,
    pub pairing: IrohPairingService,
    pub radio_media_factory: IrohRadioMediaSystemFactory,
}

/// The single accept owner for an Iroh endpoint. `Endpoint::accept()` is a
/// destructive stream; letting peer and pairing services consume it
/// independently races and can hand a pairing connection to the peer
/// protocol. The router classifies by ALPN before any provider subsystem sees
/// the connection.
pub(crate) struct IrohIncomingRouter {
    peer: Mutex<VecDeque<Connection>>,
    pairing: Mutex<VecDeque<Connection>>,
    radio: Mutex<VecDeque<Connection>>,
    peer_wake: Wake,
    notify: Arc<tokio::sync::Notify>,
}

/// Provider-owned Radio media factory. It uses a dedicated ALPN on the same
/// Iroh endpoint, keeping media streams separate from peer and pairing data.
pub struct IrohRadioMediaSystemFactory {
    endpoint: Endpoint,
    runtime: Arc<Runtime>,
    incoming: Arc<IrohIncomingRouter>,
}

impl IrohRadioMediaSystemFactory {
    pub(crate) fn new(
        endpoint: Endpoint,
        runtime: Arc<Runtime>,
        incoming: Arc<IrohIncomingRouter>,
    ) -> Self {
        Self { endpoint, runtime, incoming }
    }
}

struct IrohRadioMediaStream {
    connection: Connection,
    send: SendStream,
    recv: RecvStream,
    runtime: Arc<Runtime>,
    read_timeout: Mutex<Option<Duration>>,
}

impl std::io::Read for IrohRadioMediaStream {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let timeout = self.read_timeout.lock().ok().and_then(|value| *value);
        let result: std::io::Result<Option<usize>> = self.runtime.block_on(async {
            match timeout {
                Some(timeout) => {
                    match tokio::time::timeout(timeout, self.recv.read(buffer)).await {
                        Ok(Ok(value)) => Ok(value),
                        Ok(Err(error)) => Err(std::io::Error::new(
                            std::io::ErrorKind::Interrupted,
                            error.to_string(),
                        )),
                        Err(_) => Err(std::io::Error::new(
                            std::io::ErrorKind::WouldBlock,
                            "radio read timeout",
                        )),
                    }
                }
                None => self.recv.read(buffer).await.map_err(|error| {
                    std::io::Error::new(std::io::ErrorKind::Interrupted, error.to_string())
                }),
            }
        });
        result.map(|value| value.unwrap_or(0))
    }
}

impl std::io::Write for IrohRadioMediaStream {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        self.runtime
            .block_on(self.send.write(buffer))
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::BrokenPipe, error.to_string()))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.runtime.block_on(self.send.flush())
    }
}

impl torca_radio_adapters::RadioMediaStream for IrohRadioMediaStream {
    fn set_read_deadline(
        &self,
        timeout: Duration,
    ) -> Result<(), torca_radio_coordinator::RadioApplicationError> {
        self.read_timeout
            .lock()
            .map_err(|_| torca_radio_coordinator::RadioApplicationError::MediaTransport)
            .map(|mut value| *value = Some(timeout))
    }

    fn close_stream(&self) -> Result<(), torca_radio_coordinator::RadioApplicationError> {
        self.connection.close(0_u32.into(), b"radio close");
        Ok(())
    }
}

impl torca_radio_adapters::RadioMediaConnector for IrohRadioMediaSystemFactory {
    fn connect(
        &mut self,
        route: &torca_radio_adapters::RadioMediaRoute,
        timeout: Duration,
    ) -> Result<
        Box<dyn torca_radio_adapters::RadioMediaStream>,
        torca_radio_coordinator::RadioApplicationError,
    > {
        if route.provider != TransportKind::Iroh.wire_value() {
            return Err(torca_radio_coordinator::RadioApplicationError::MediaTransport);
        }
        let remote = decode_endpoint_addr(&route.endpoint)
            .map_err(|_| torca_radio_coordinator::RadioApplicationError::MediaTransport)?;
        let connection = self
            .runtime
            .block_on(async {
                tokio::time::timeout(timeout, self.endpoint.connect(remote, RADIO_ALPN))
                    .await
                    .map_err(|_| ())?
                    .map_err(|_| ())
            })
            .map_err(|_| torca_radio_coordinator::RadioApplicationError::MediaTransport)?;
        let (send, recv) = self
            .runtime
            .block_on(async {
                tokio::time::timeout(timeout, connection.open_bi())
                    .await
                    .map_err(|_| ())?
                    .map_err(|_| ())
            })
            .map_err(|_| torca_radio_coordinator::RadioApplicationError::MediaTransport)?;
        Ok(Box::new(IrohRadioMediaStream {
            connection,
            send,
            recv,
            runtime: Arc::clone(&self.runtime),
            read_timeout: Mutex::new(None),
        }))
    }

    fn try_accept(
        &mut self,
    ) -> Result<
        Option<Box<dyn torca_radio_adapters::RadioMediaStream>>,
        torca_radio_coordinator::RadioApplicationError,
    > {
        let Some(connection) = self.incoming.take_radio() else { return Ok(None) };
        let (send, recv) = self
            .runtime
            .block_on(connection.accept_bi())
            .map_err(|_| torca_radio_coordinator::RadioApplicationError::MediaTransport)?;
        Ok(Some(Box::new(IrohRadioMediaStream {
            connection,
            send,
            recv,
            runtime: Arc::clone(&self.runtime),
            read_timeout: Mutex::new(None),
        })))
    }

    fn keep_alive_interval(&self) -> Duration {
        // Iroh/QUIC has a shorter transport idle window than the legacy
        // stream providers. Keep the application lane alive before that
        // window can expire while floor/audio negotiation is quiet.
        Duration::from_secs(10)
    }
}

impl torca_radio_adapters::RadioMediaSystemFactory for IrohRadioMediaSystemFactory {
    fn start(
        self: Box<Self>,
        directory: Box<dyn torca_radio_adapters::RadioMediaDirectory>,
    ) -> Result<
        torca_radio_adapters::RadioMediaSystem,
        torca_radio_coordinator::RadioApplicationError,
    > {
        torca_radio_adapters::RadioMediaSystem::start_with_connector(self, directory)
    }
}

impl IrohIncomingRouter {
    fn start(endpoint: Endpoint, runtime: Arc<Runtime>) -> Arc<Self> {
        let router = Arc::new(Self {
            peer: Mutex::new(VecDeque::new()),
            pairing: Mutex::new(VecDeque::new()),
            radio: Mutex::new(VecDeque::new()),
            peer_wake: Arc::new(Mutex::new(None)),
            notify: Arc::new(tokio::sync::Notify::new()),
        });
        let task_router = Arc::clone(&router);
        runtime.spawn(async move {
            while let Some(incoming) = endpoint.accept().await {
                let Ok(accepted) = incoming.accept() else { continue };
                let Ok(connection) = accepted.await else { continue };
                let queue = match connection.alpn() {
                    value if value == ALPN => &task_router.peer,
                    value if value == PAIRING_ALPN => &task_router.pairing,
                    value if value == RADIO_ALPN => &task_router.radio,
                    _ => continue,
                };
                if let Ok(mut entries) = queue.lock() {
                    entries.push_back(connection);
                }
                task_router.notify.notify_one();
                notify(&task_router.peer_wake);
            }
        });
        router
    }

    fn take_peer(&self) -> Option<Connection> {
        self.peer.lock().ok()?.pop_front()
    }

    pub(crate) fn take_pairing(&self) -> Option<Connection> {
        self.pairing.lock().ok()?.pop_front()
    }

    pub(crate) async fn wait_for_connection(&self) {
        self.notify.notified().await;
    }

    #[allow(dead_code)]
    pub(crate) fn take_radio(&self) -> Option<Connection> {
        self.radio.lock().ok()?.pop_front()
    }

    fn set_peer_waker(&self, waker: Arc<dyn Fn() + Send + Sync>) {
        if let Ok(mut slot) = self.peer_wake.lock() {
            *slot = Some(waker);
        }
    }
}

fn build_provider_runtime() -> Result<Runtime, IrohIdentityError> {
    // Iroh creates its own network watchers and relay/address-lookup tasks.
    // Tokio::Runtime::new() otherwise allocates one worker per CPU, which is
    // particularly expensive on mobile devices and does not improve the
    // single-endpoint workload. Keep a small, named pool for observability;
    // desktop retains enough parallelism for concurrent transfers.
    let worker_threads = if cfg!(target_os = "android") { 2 } else { 4 };
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(worker_threads)
        .thread_name("torca-iroh")
        .enable_all()
        .build()
        .map_err(|error| IrohIdentityError::Bind(format!("create Iroh runtime: {error}")))
}

/// Direct Iroh transport for the shared pairing request protocol. It is
/// intentionally separate from `IrohTransport`: pairing has request/response
/// framing while authenticated peer traffic is a long-lived byte stream.
pub struct IrohPairingServiceTransport {
    endpoint: Endpoint,
    remote: EndpointAddr,
    runtime: Arc<Runtime>,
    connection: Option<Connection>,
}

impl IrohPairingServiceTransport {
    pub fn new(endpoint: Endpoint, remote: EndpointAddr, runtime: Arc<Runtime>) -> Self {
        Self { endpoint, remote, runtime, connection: None }
    }

    pub fn from_bootstrap(
        endpoint: Endpoint,
        descriptor: &PairingBootstrapDescriptor,
        runtime: Arc<Runtime>,
    ) -> Result<Self, String> {
        if descriptor.provider() != "iroh" {
            return Err("pairing bootstrap belongs to another provider".into());
        }
        let remote =
            decode_endpoint_addr(descriptor.payload()).map_err(|error| error.to_string())?;
        Ok(Self::new(endpoint, remote, runtime))
    }

    fn transport_error(kind: RelayTransportFailureKind, sent: bool) -> RelayTransportError {
        RelayTransportError { kind, request_was_sent: sent }
    }
}

impl PairingServiceTransport for IrohPairingServiceTransport {
    fn invalidate(&mut self) {
        self.connection.take().map(|connection| connection.close(0_u32.into(), b"pairing reset"));
    }

    fn reconnect(&mut self) -> Result<(), RelayTransportError> {
        self.invalidate();
        let connection = self
            .runtime
            .block_on(self.endpoint.connect(self.remote.clone(), PAIRING_ALPN))
            .map_err(|_| Self::transport_error(RelayTransportFailureKind::Unavailable, false))?;
        self.connection = Some(connection);
        Ok(())
    }

    fn exchange(
        &mut self,
        request: &RelayRequest,
        timeout: Duration,
    ) -> Result<RelayResponse, RelayTransportError> {
        let Some(connection) = self.connection.clone() else {
            return Err(Self::transport_error(RelayTransportFailureKind::Unavailable, false));
        };
        let frame = RelayCodec::encode_request(request).map_err(|_| {
            Self::transport_error(RelayTransportFailureKind::InvalidResponse, false)
        })?;
        let result = self.runtime.block_on(async move {
            let (mut send, mut recv) = tokio::time::timeout(timeout, connection.open_bi())
                .await
                .map_err(|_| RelayTransportFailureKind::Timeout)?
                .map_err(|_| RelayTransportFailureKind::Disconnected)?;
            tokio::time::timeout(timeout, send.write_all(&frame))
                .await
                .map_err(|_| RelayTransportFailureKind::Timeout)?
                .map_err(|_| RelayTransportFailureKind::Disconnected)?;
            send.finish().map_err(|_| RelayTransportFailureKind::Disconnected)?;
            let mut header = [0_u8; RELAY_HEADER_LEN];
            tokio::time::timeout(timeout, recv.read_exact(&mut header))
                .await
                .map_err(|_| RelayTransportFailureKind::Timeout)?
                .map_err(|_| RelayTransportFailureKind::Disconnected)?;
            let frame_len = RelayCodec::frame_len_from_header(&header)
                .map_err(|_| RelayTransportFailureKind::InvalidResponse)?;
            let mut response = Vec::with_capacity(frame_len);
            response.extend_from_slice(&header);
            let mut payload = vec![0_u8; frame_len - RELAY_HEADER_LEN];
            if !payload.is_empty() {
                tokio::time::timeout(timeout, recv.read_exact(&mut payload))
                    .await
                    .map_err(|_| RelayTransportFailureKind::Timeout)?
                    .map_err(|_| RelayTransportFailureKind::Disconnected)?;
                response.extend_from_slice(&payload);
            }
            RelayCodec::decode_response(&response)
                .map_err(|_| RelayTransportFailureKind::InvalidResponse)
        });
        result.map_err(|kind| Self::transport_error(kind, true))
    }
}

impl IrohComposition {
    pub fn bind(
        runtime: Arc<Runtime>,
        store: &mut dyn ProtectedSecretStore,
    ) -> Result<Self, IrohIdentityError> {
        let endpoint = bind_endpoint(&runtime, store)?;
        let incoming = IrohIncomingRouter::start(endpoint.clone(), Arc::clone(&runtime));
        let lifecycle = IrohLifecycle::new(endpoint.clone(), Arc::clone(&runtime));
        let transport_factory = IrohTransportFactory::new(
            endpoint.clone(),
            Arc::clone(&runtime),
            Arc::clone(&incoming),
        );
        let radio_media_factory = IrohRadioMediaSystemFactory::new(
            endpoint.clone(),
            Arc::clone(&runtime),
            Arc::clone(&incoming),
        );
        // Peer traffic and pairing share one provider-owned endpoint. Both
        // protocols are already separated by ALPN, while a second endpoint
        // would duplicate Iroh's network watchers, relay discovery and
        // Android native worker threads for no functional benefit.
        let pairing = IrohPairingService::new(endpoint, Arc::clone(&runtime), incoming);
        Ok(Self { lifecycle, transport_factory, pairing, radio_media_factory })
    }

    pub fn pairing_bootstrap_descriptor(&self) -> Result<PairingBootstrapDescriptor, String> {
        self.pairing.pairing_bootstrap_descriptor()
    }

    pub fn peer_endpoint_bytes(&self) -> Result<Vec<u8>, String> {
        self.lifecycle.peer_endpoint_bytes()
    }
}

impl std::error::Error for IrohIdentityError {}

impl From<ProtectedSecretStoreError> for IrohIdentityError {
    fn from(error: ProtectedSecretStoreError) -> Self {
        Self::Store(error)
    }
}

/// Loads the provider's stable endpoint identity or stores a freshly generated
/// one. No endpoint secret is included in diagnostics or error strings.
pub fn load_or_create_endpoint_secret(
    store: &mut dyn ProtectedSecretStore,
) -> Result<iroh::SecretKey, IrohIdentityError> {
    match store.load(IROH_ENDPOINT_SECRET_HANDLE)? {
        Some(mut stored) => {
            let bytes: [u8; 32] = match stored.as_slice().try_into() {
                Ok(bytes) => bytes,
                Err(_) => {
                    stored.fill(0);
                    return Err(IrohIdentityError::InvalidStoredKey);
                }
            };
            stored.fill(0);
            Ok(iroh::SecretKey::from_bytes(&bytes))
        }
        None => {
            let secret = iroh::SecretKey::generate();
            let mut bytes = secret.to_bytes();
            let result = store.insert(IROH_ENDPOINT_SECRET_HANDLE, &bytes);
            bytes.fill(0);
            result?;
            Ok(secret)
        }
    }
}

/// Stable opaque pairing representation for an Iroh endpoint address.
pub fn encode_endpoint_addr(address: &EndpointAddr) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(address)
}

/// Decode the opaque endpoint hint received in a pairing offer.
pub fn decode_endpoint_addr(bytes: &[u8]) -> Result<EndpointAddr, serde_json::Error> {
    serde_json::from_slice(bytes)
}

type Wake = Arc<Mutex<Option<Arc<dyn Fn() + Send + Sync>>>>;
type Inbound = Arc<(Mutex<VecDeque<Result<Vec<u8>, PeerTransportError>>>, Condvar)>;

/// Lifecycle for a provider-owned Iroh endpoint.
///
/// Binding an endpoint is enough to make the local runtime usable.  Iroh's
/// `online` future independently confirms that endpoint discovery/relay
/// infrastructure is available for incoming reachability.  This is purposely
/// not translated into Tor/onion vocabulary.
pub struct IrohLifecycle {
    endpoint: Endpoint,
    runtime: Arc<Runtime>,
    online: Arc<AtomicBool>,
    stopped: Arc<AtomicBool>,
    dormant: Arc<AtomicBool>,
    wake: Wake,
}

impl IrohLifecycle {
    /// Takes ownership of an endpoint already bound by the native provider
    /// composition.  Identity persistence and endpoint construction belong to
    /// that composition, never to the shared runtime.
    pub fn new(endpoint: Endpoint, runtime: Arc<Runtime>) -> Self {
        let online = Arc::new(AtomicBool::new(false));
        let stopped = Arc::new(AtomicBool::new(false));
        let dormant = Arc::new(AtomicBool::new(false));
        let wake = Arc::new(Mutex::new(None));
        let online_task = Arc::clone(&online);
        let stopped_task = Arc::clone(&stopped);
        let wake_task = Arc::clone(&wake);
        let endpoint_task = endpoint.clone();
        runtime.spawn(async move {
            endpoint_task.online().await;
            if !stopped_task.load(Ordering::Acquire) {
                online_task.store(true, Ordering::Release);
                notify(&wake_task);
            }
        });
        Self { endpoint, runtime, online, stopped, dormant, wake }
    }

    fn endpoint_summary(&self) -> String {
        format!("iroh:{}", self.endpoint.addr().id)
    }

    /// Produces the short-lived direct-QR bootstrap descriptor for this bound
    /// endpoint. It contains only the serialised Iroh address needed to open
    /// the initial pairing stream; it is not a Torca identity or contact route.
    pub fn pairing_bootstrap_descriptor(&self) -> Result<PairingBootstrapDescriptor, String> {
        let address = self.endpoint.addr();
        if address.is_empty() {
            return Err("Iroh endpoint has no dialable transport address yet".into());
        }
        let payload = encode_endpoint_addr(&address).map_err(|error| error.to_string())?;
        PairingBootstrapDescriptor::new("iroh", payload).map_err(|error| error.to_string())
    }

    pub fn peer_endpoint_bytes(&self) -> Result<Vec<u8>, String> {
        encode_endpoint_addr(&self.endpoint.addr()).map_err(|error| error.to_string())
    }
}

impl CommunicationLifecycle for IrohLifecycle {
    fn provider(&self) -> TransportKind {
        TransportKind::Iroh
    }

    fn maintenance(&mut self, _now: Timestamp) -> Result<(), RuntimeDriverError> {
        if self.endpoint.is_closed() && !self.stopped.load(Ordering::Acquire) {
            return Err(RuntimeDriverError::Communication);
        }
        Ok(())
    }

    fn set_waker(&mut self, waker: Arc<dyn Fn() + Send + Sync>) {
        if let Ok(mut slot) = self.wake.lock() {
            *slot = Some(waker);
        }
    }

    fn set_dormant(&mut self, dormant: bool) -> Result<(), RuntimeDriverError> {
        // Iroh currently has no equivalent to Tor's SoftDormant.  The runtime
        // still records this policy transition, while the provider retains a
        // bound endpoint so incoming direct connections remain possible.
        self.dormant.store(dormant, Ordering::Release);
        Ok(())
    }

    fn state(&self) -> CommunicationState {
        if self.stopped.load(Ordering::Acquire) {
            CommunicationState::Stopped
        } else if self.endpoint.is_closed() {
            CommunicationState::Failed
        } else {
            CommunicationState::Ready
        }
    }

    fn local_endpoint_summary(&self) -> Option<String> {
        (!self.stopped.load(Ordering::Acquire)).then(|| self.endpoint_summary())
    }

    fn incoming_reachability_state(&self) -> IncomingReachabilityState {
        if self.stopped.load(Ordering::Acquire) {
            IncomingReachabilityState::Stopped
        } else if self.endpoint.is_closed() {
            IncomingReachabilityState::Failed
        } else if self.online.load(Ordering::Acquire) {
            IncomingReachabilityState::Reachable
        } else {
            IncomingReachabilityState::Publishing
        }
    }

    fn commissioning(&self) -> ProviderCommissioning {
        let local = match self.state() {
            CommunicationState::Ready => CommissioningState::Ready,
            CommunicationState::Starting | CommunicationState::Stopped => {
                CommissioningState::Pending
            }
            CommunicationState::Degraded => CommissioningState::Degraded,
            CommunicationState::Failed => CommissioningState::Failed,
        };
        let incoming = match self.incoming_reachability_state() {
            IncomingReachabilityState::Reachable => CommissioningState::Ready,
            IncomingReachabilityState::Publishing | IncomingReachabilityState::Unknown => {
                CommissioningState::Pending
            }
            IncomingReachabilityState::Degraded => CommissioningState::Degraded,
            IncomingReachabilityState::Failed => CommissioningState::Failed,
            IncomingReachabilityState::Stopped => CommissioningState::NotRequired,
        };
        ProviderCommissioning {
            provider: TransportKind::Iroh,
            steps: vec![
                CommissioningStep {
                    stage: CommissioningStage::LocalRuntime,
                    state: local,
                    required_for_local_shell: true,
                    required_for_pairing: true,
                },
                CommissioningStep {
                    stage: CommissioningStage::IncomingReachability,
                    state: incoming,
                    required_for_local_shell: false,
                    // Creating an invitation is local: the endpoint address
                    // is already part of the QR/bootstrap descriptor.  A
                    // temporary discovery/relay delay must not turn a valid
                    // invitation into a "warming up" gate.  The join path
                    // performs the authoritative reachability attempt.
                    required_for_pairing: false,
                },
            ],
            endpoint_summary: self.local_endpoint_summary(),
            pairing_bootstrap: self.pairing_bootstrap_descriptor().ok(),
        }
    }

    fn shutdown(&mut self) {
        if self.stopped.swap(true, Ordering::AcqRel) {
            return;
        }
        self.online.store(false, Ordering::Release);
        self.runtime.block_on(self.endpoint.close());
    }
}

/// A reliable, ordered bidirectional Iroh stream with Torca payload framing.
pub struct IrohTransport {
    endpoint: Endpoint,
    remote: Option<EndpointAddr>,
    runtime: Arc<Runtime>,
    connection: Option<Connection>,
    sender: Option<Arc<AsyncMutex<SendStream>>>,
    inbound: Inbound,
    wake: Wake,
    connected: bool,
}

impl IrohTransport {
    /// Creates a disconnected outgoing transport.
    pub fn new(endpoint: Endpoint, remote: EndpointAddr, runtime: Arc<Runtime>) -> Self {
        Self {
            endpoint,
            remote: Some(remote),
            runtime,
            connection: None,
            sender: None,
            inbound: Arc::new((Mutex::new(VecDeque::new()), Condvar::new())),
            wake: Arc::new(Mutex::new(None)),
            connected: false,
        }
    }

    /// Wraps an accepted Iroh connection. The first bidirectional stream is
    /// reserved for the Torca peer protocol.
    pub fn from_connection(
        endpoint: Endpoint,
        connection: Connection,
        runtime: Arc<Runtime>,
    ) -> Result<Self, PeerTransportError> {
        let mut transport = Self {
            endpoint,
            remote: None,
            runtime,
            connection: Some(connection),
            sender: None,
            inbound: Arc::new((Mutex::new(VecDeque::new()), Condvar::new())),
            wake: Arc::new(Mutex::new(None)),
            connected: false,
        };
        transport.open_incoming_stream()?;
        Ok(transport)
    }

    fn open_incoming_stream(&mut self) -> Result<(), PeerTransportError> {
        let connection = self
            .connection
            .clone()
            .ok_or_else(|| PeerTransportError("Iroh connection is unavailable".into()))?;
        let (sender, receiver) = self
            .runtime
            .block_on(connection.accept_bi())
            .map_err(|error| PeerTransportError(format!("Iroh accept stream failed: {error}")))?;
        self.install_stream(sender, receiver);
        self.connected = true;
        Ok(())
    }

    fn install_stream(&mut self, sender: SendStream, mut receiver: RecvStream) {
        let inbound = Arc::clone(&self.inbound);
        let wake = Arc::clone(&self.wake);
        self.sender = Some(Arc::new(AsyncMutex::new(sender)));
        self.runtime.spawn(async move {
            loop {
                let length = match receiver.read_u32().await {
                    Ok(length) => length as usize,
                    Err(error) => {
                        push_inbound(
                            &inbound,
                            Err(PeerTransportError(format!("Iroh receive length failed: {error}"))),
                        );
                        notify(&wake);
                        break;
                    }
                };
                if length > MAX_FRAME {
                    push_inbound(
                        &inbound,
                        Err(PeerTransportError("Iroh frame exceeds peer limit".into())),
                    );
                    notify(&wake);
                    break;
                }
                let mut payload = vec![0_u8; length];
                if let Err(error) = receiver.read_exact(&mut payload).await {
                    push_inbound(
                        &inbound,
                        Err(PeerTransportError(format!("Iroh receive payload failed: {error}"))),
                    );
                    notify(&wake);
                    break;
                }
                push_inbound(&inbound, Ok(payload));
                notify(&wake);
            }
        });
    }

    fn map_error(error: impl std::fmt::Display) -> PeerTransportError {
        PeerTransportError(format!("Iroh transport failed: {error}"))
    }
}

impl PeerTransport for IrohTransport {
    fn connect(&mut self) -> Result<(), PeerTransportError> {
        if self.connected {
            return Ok(());
        }
        let remote = self
            .remote
            .clone()
            .ok_or_else(|| PeerTransportError("Iroh remote address is missing".into()))?;
        let connection =
            self.runtime.block_on(self.endpoint.connect(remote, ALPN)).map_err(Self::map_error)?;
        let (sender, receiver) =
            self.runtime.block_on(connection.open_bi()).map_err(Self::map_error)?;
        self.connection = Some(connection);
        self.install_stream(sender, receiver);
        self.connected = true;
        Ok(())
    }

    fn send(&mut self, payload: Vec<u8>) -> Result<(), PeerTransportError> {
        if payload.len() > MAX_FRAME {
            return Err(PeerTransportError("Iroh payload exceeds peer limit".into()));
        }
        let sender = self
            .sender
            .clone()
            .ok_or_else(|| PeerTransportError("Iroh transport is not connected".into()))?;
        self.runtime.block_on(async move {
            let mut sender = sender.lock().await;
            sender.write_u32(payload.len() as u32).await.map_err(Self::map_error)?;
            sender.write_all(&payload).await.map_err(Self::map_error)?;
            sender.flush().await.map_err(Self::map_error)
        })
    }

    fn try_receive(&mut self) -> Result<Option<Vec<u8>>, PeerTransportError> {
        self.inbound
            .0
            .lock()
            .map_err(|_| PeerTransportError("Iroh inbound queue poisoned".into()))?
            .pop_front()
            .transpose()
    }

    fn receive_timeout(
        &mut self,
        timeout: Duration,
    ) -> Result<Option<Vec<u8>>, PeerTransportError> {
        let mut queue = self
            .inbound
            .0
            .lock()
            .map_err(|_| PeerTransportError("Iroh inbound queue poisoned".into()))?;
        if queue.is_empty() {
            let (guard, _) = self
                .inbound
                .1
                .wait_timeout(queue, timeout)
                .map_err(|_| PeerTransportError("Iroh inbound wait poisoned".into()))?;
            queue = guard;
        }
        queue.pop_front().transpose()
    }

    fn close(&mut self) -> Result<(), PeerTransportError> {
        self.connected = false;
        self.sender = None;
        if let Some(connection) = self.connection.take() {
            connection.close(0_u32.into(), b"torca close");
        }
        Ok(())
    }

    fn set_waker(&mut self, waker: Arc<dyn Fn() + Send + Sync>) {
        if let Ok(mut slot) = self.wake.lock() {
            *slot = Some(waker);
            // The reader task can receive the first handshake ACK before the
            // PeerLink has finished installing its callback.  The payload is
            // durable in `inbound`; replay the wake after registration so the
            // runtime cannot strand an otherwise healthy session in
            // `Handshaking`.
            let has_pending = self.inbound.0.lock().map(|queue| !queue.is_empty()).unwrap_or(false);
            if has_pending {
                if let Some(callback) = slot.as_ref() {
                    callback();
                }
            }
        }
    }
}

impl ProviderTransport for IrohTransport {
    fn kind(&self) -> TransportKind {
        TransportKind::Iroh
    }

    fn path(&self) -> TransportPath {
        TransportPath::IrohDirect
    }

    fn capabilities(&self) -> TransportCapabilities {
        TransportCapabilities {
            reliable: true,
            ordered: true,
            supports_incoming: true,
            supports_direct_path: true,
            supports_relay_path: true,
            hides_peer_ip: false,
            max_frame_size: MAX_FRAME,
            latency: LatencyClass::Interactive,
            energy: EnergyClass::Medium,
        }
    }
}

impl Drop for IrohTransport {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

fn push_inbound(queue: &Inbound, item: Result<Vec<u8>, PeerTransportError>) {
    if let Ok(mut entries) = queue.0.lock() {
        entries.push_back(item);
        queue.1.notify_one();
    }
}

fn notify(wake: &Wake) {
    if let Some(callback) = wake.lock().ok().and_then(|slot| slot.clone()) {
        callback();
    }
}

/// Factory used by native composition once the Iroh endpoint is available.
///
/// A remote route is decoded from the current contact on every dial. This is
/// essential because contacts can be created after the runtime starts.
pub struct IrohTransportFactory {
    endpoint: Endpoint,
    runtime: Arc<Runtime>,
    incoming: Arc<IrohIncomingRouter>,
    wake: Wake,
}

impl IrohTransportFactory {
    pub(crate) fn new(
        endpoint: Endpoint,
        runtime: Arc<Runtime>,
        incoming: Arc<IrohIncomingRouter>,
    ) -> Self {
        let wake = Arc::new(Mutex::new(None));
        incoming.set_peer_waker(Arc::new({
            let wake = Arc::clone(&wake);
            move || notify(&wake)
        }));
        Self { endpoint, runtime, incoming, wake }
    }
}

impl PeerTransportFactory for IrohTransportFactory {
    fn kind(&self) -> TransportKind {
        TransportKind::Iroh
    }

    fn capabilities(&self) -> TransportCapabilities {
        IrohTransport {
            endpoint: self.endpoint.clone(),
            remote: None,
            runtime: Arc::clone(&self.runtime),
            connection: None,
            sender: None,
            inbound: Arc::new((Mutex::new(VecDeque::new()), Condvar::new())),
            wake: Arc::clone(&self.wake),
            connected: false,
        }
        .capabilities()
    }

    fn accept(&mut self) -> Result<Option<Box<dyn PeerTransport + Send>>, TransportFactoryError> {
        let Some(connection) = self.incoming.take_peer() else {
            return Ok(None);
        };
        let mut transport = IrohTransport::from_connection(
            self.endpoint.clone(),
            connection,
            Arc::clone(&self.runtime),
        )
        .map_err(|_| TransportFactoryError::Listener)?;
        if let Some(waker) = self.wake.lock().ok().and_then(|slot| slot.clone()) {
            transport.set_waker(waker);
        }
        Ok(Some(Box::new(transport) as Box<dyn PeerTransport + Send>))
    }

    fn connect(
        &mut self,
        contact: &Contact,
    ) -> Result<Box<dyn PeerTransport + Send>, TransportFactoryError> {
        let endpoint = contact
            .route()
            .provider_endpoint(TransportKind::Iroh.wire_value())
            .ok_or(TransportFactoryError::ContactNotFound)?;
        let remote = decode_endpoint_addr(endpoint).map_err(|_| TransportFactoryError::Protocol)?;
        let mut transport =
            IrohTransport::new(self.endpoint.clone(), remote, Arc::clone(&self.runtime));
        if let Some(waker) = self.wake.lock().ok().and_then(|slot| slot.clone()) {
            transport.set_waker(waker);
        }
        Ok(Box::new(transport))
    }

    fn set_waker(&self, waker: Arc<dyn Fn() + Send + Sync>) -> Result<(), TransportFactoryError> {
        if let Ok(mut slot) = self.wake.lock() {
            *slot = Some(waker);
            Ok(())
        } else {
            Err(TransportFactoryError::Listener)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use iroh::endpoint::presets;
    use tokio::runtime::Runtime;
    use torca_crypto::{InMemoryProtectedSecretStore, ProtectedSecretStore};
    use torca_runtime::{CommunicationLifecycle, CommunicationState, IncomingReachabilityState};
    use torca_transport_api::{CommissioningStage, CommissioningState, TransportKind};

    use super::{
        ALPN, IROH_ENDPOINT_SECRET_HANDLE, IrohComposition, IrohLifecycle, bind_endpoint,
        decode_endpoint_addr, load_or_create_endpoint_secret,
    };

    #[test]
    fn endpoint_identity_is_stable_in_the_protected_store() {
        let mut store = InMemoryProtectedSecretStore::default();
        let first = load_or_create_endpoint_secret(&mut store).expect("create Iroh secret");
        let second = load_or_create_endpoint_secret(&mut store).expect("load Iroh secret");

        assert_eq!(first.public(), second.public());
        assert_eq!(
            store
                .load(IROH_ENDPOINT_SECRET_HANDLE)
                .expect("read Iroh secret")
                .map(|bytes| bytes.len()),
            Some(32)
        );
    }

    #[test]
    fn lifecycle_reports_iroh_without_tor_or_onion_projection() {
        let runtime = Arc::new(Runtime::new().expect("Iroh Tokio runtime"));
        let endpoint = runtime
            .block_on(iroh::Endpoint::builder(presets::N0).alpns(vec![ALPN.to_vec()]).bind())
            .expect("bind local Iroh endpoint");
        let endpoint_id = endpoint.addr().id;
        let mut lifecycle = IrohLifecycle::new(endpoint, Arc::clone(&runtime));

        assert_eq!(lifecycle.provider(), TransportKind::Iroh);
        assert_eq!(lifecycle.state(), CommunicationState::Ready);
        assert_eq!(lifecycle.incoming_reachability_state(), IncomingReachabilityState::Publishing);
        let commissioning = lifecycle.commissioning();
        assert_eq!(commissioning.provider, TransportKind::Iroh);
        assert!(commissioning.endpoint_summary.is_some());
        assert_eq!(commissioning.step(CommissioningStage::LocalRuntime), CommissioningState::Ready);
        // Creating an invitation is local and must not wait for discovery to
        // mark the endpoint reachable; the join path owns that network try.
        assert!(commissioning.pairing_ready());
        let descriptor = lifecycle.pairing_bootstrap_descriptor().expect("QR bootstrap descriptor");
        assert_eq!(descriptor.provider(), "iroh");
        assert_eq!(
            decode_endpoint_addr(descriptor.payload()).expect("decode endpoint").id,
            endpoint_id
        );

        lifecycle.shutdown();
        assert_eq!(lifecycle.state(), CommunicationState::Stopped);
    }

    #[test]
    fn endpoint_binding_uses_stable_identity() {
        let runtime = Runtime::new().expect("Iroh Tokio runtime");
        let mut store = InMemoryProtectedSecretStore::default();
        let first = bind_endpoint(&runtime, &mut store).expect("bind endpoint");
        let first_id = first.addr().id;
        runtime.block_on(first.close());
        let second = bind_endpoint(&runtime, &mut store).expect("bind endpoint again");
        assert_eq!(first_id, second.addr().id);
        runtime.block_on(second.close());
    }

    #[test]
    fn composition_exposes_provider_owned_bootstrap_only() {
        let runtime = Arc::new(Runtime::new().expect("Iroh Tokio runtime"));
        let mut store = InMemoryProtectedSecretStore::default();
        let mut composition =
            IrohComposition::bind(Arc::clone(&runtime), &mut store).expect("bind Iroh composition");
        let descriptor = composition.pairing_bootstrap_descriptor().expect("descriptor");
        assert_eq!(descriptor.provider(), "iroh");
        composition.lifecycle.shutdown();
    }
}
