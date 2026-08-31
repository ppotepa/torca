//! Iroh/QUIC implementation of the provider-neutral Torca transport.
//!
//! The adapter receives a configured Iroh endpoint and remote address from
//! the provider composition layer. Signalling and endpoint identity exchange
//! stay outside this crate; peer protocol and E2EE remain unchanged.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use iroh::EndpointAddr;
use iroh::endpoint::{Connection, Endpoint, RecvStream, SendStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::runtime::Runtime;
use tokio::sync::Mutex as AsyncMutex;
use torca_contacts::Contact;
use torca_crypto::{ProtectedSecretStore, ProtectedSecretStoreError};
use torca_foundation::{ProviderId, Timestamp};
use torca_pairing_protocol::PairingBootstrapDescriptor;
use torca_pairing_service_client::{
    PairingServiceTransport, PairingServiceTransportError, PairingServiceTransportFailureKind,
};
use torca_pairing_service_protocol::{
    PAIRING_SERVICE_HEADER_LEN, PairingServiceCodec, PairingServiceRequest, PairingServiceResponse,
};
use torca_peer_protocol::MAX_PEER_DATA_LEN;
use torca_runtime::{
    CommunicationLifecycle, CommunicationState, IncomingReachabilityState, RuntimeDriverError,
};
use torca_transport_api::{
    CommissioningPresentation, CommissioningStage, CommissioningState, CommissioningStep,
    LatencyClass, PeerTransport, PeerTransportError, PeerTransportFactory, ProviderCommissioning,
    ProviderRouteState, ProviderRuntimeDiagnostics, ProviderTransport, TransportCapabilities,
    TransportFactoryError, TransportPath, TransportTopology,
};

mod endpoint;
mod pairing;
mod profile;
use endpoint::bind_endpoint_from_secret;
pub use endpoint::{
    IROH_ENDPOINT_SECRET_HANDLE, IROH_PAIRING_SECRET_HANDLE, IrohEndpointSlot, IrohIdentityError,
    ProviderEndpoint, ProviderEndpointSlot, bind_endpoint, provider_runtime,
};
pub use pairing::IrohPairingService;
pub use profile::IrohEndpointProfile;

#[cfg(test)]
use profile::IrohServiceConfig;
use profile::{COMPILED_IROH_LOCAL_ONLY, COMPILED_IROH_RUNTIME_THREADS, configured_flag};

const ALPN: &[u8] = b"torca/peer/1";
const NETWORK_CHANGE_TIMEOUT: Duration = Duration::from_secs(30);
/// ALPN used only for the short-lived provider-owned pairing service.
pub const PAIRING_ALPN: &[u8] = b"torca/pairing/1";
/// Reserved provider-owned ALPN for optional Radio media streams.
pub const RADIO_ALPN: &[u8] = b"torca/radio/1";

fn iroh_provider_id() -> ProviderId {
    ProviderId::new("iroh").expect("static provider id")
}

const MAX_FRAME: usize = MAX_PEER_DATA_LEN;
const PEER_CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const PEER_WRITE_TIMEOUT: Duration = Duration::from_secs(10);
const RADIO_WRITE_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_PENDING_INCOMING_STREAMS: usize = 32;
const MAX_PENDING_PAIRING_CONNECTIONS: usize = 32;
/// Bound decoded peer frames waiting for the application worker. Without a
/// cap, a fast peer (or a malicious one) could keep allocating while the
/// runtime is backgrounded, defeating the provider's idle/battery policy.
const MAX_PENDING_INBOUND_FRAMES: usize = 256;
/// A failed incoming-reachability check must not become a permanent mobile
/// timer.  A later OS network-generation event or explicit provider wake
/// starts a fresh bounded sequence.
const MAX_ONLINE_PROBE_ATTEMPTS: u32 = 3;

struct OnlineProbeGuard(Arc<AtomicBool>);

impl Drop for OnlineProbeGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
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
    peer_streams: Mutex<VecDeque<(Connection, SendStream, RecvStream)>>,
    pairing: Mutex<VecDeque<Connection>>,
    radio_streams: Mutex<VecDeque<(Connection, SendStream, RecvStream)>>,
    peer_wake: Wake,
    radio_wake: Wake,
    notify: Arc<tokio::sync::Notify>,
    closed: AtomicBool,
}

/// Provider-owned Radio media factory. It uses a dedicated ALPN on the same
/// Iroh endpoint, keeping media streams separate from peer and pairing data.
pub struct IrohRadioMediaSystemFactory {
    endpoint: ProviderEndpointSlot,
    runtime: Arc<Runtime>,
    incoming: Arc<IrohIncomingRouter>,
}

impl IrohRadioMediaSystemFactory {
    #[allow(dead_code)]
    pub(crate) fn new(
        endpoint: Endpoint,
        runtime: Arc<Runtime>,
        incoming: Arc<IrohIncomingRouter>,
    ) -> Self {
        Self::new_with_slot(
            IrohEndpointSlot::static_endpoint(endpoint, Arc::clone(&runtime)),
            runtime,
            incoming,
        )
    }

    pub(crate) fn new_with_slot(
        endpoint: ProviderEndpointSlot,
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
                        Ok(Err(error)) => {
                            eprintln!("torca-iroh: radio read failed: {error}");
                            Err(std::io::Error::new(
                                // `Interrupted` is reserved for a temporary
                                // read deadline in the provider-neutral
                                // worker. A QUIC stream error/reset must be
                                // fatal for this generation so the worker
                                // immediately emits Interrupted and starts
                                // reconnecting instead of waiting for the
                                // 180-second idle safety limit.
                                std::io::ErrorKind::ConnectionReset,
                                error.to_string(),
                            ))
                        }
                        Err(_) => Err(std::io::Error::new(
                            std::io::ErrorKind::WouldBlock,
                            "radio read timeout",
                        )),
                    }
                }
                None => self.recv.read(buffer).await.map_err(|error| {
                    eprintln!("torca-iroh: radio read failed: {error}");
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
            .block_on(async {
                tokio::time::timeout(RADIO_WRITE_TIMEOUT, self.send.write(buffer))
                    .await
                    .map_err(|_| {
                        std::io::Error::new(std::io::ErrorKind::TimedOut, "radio write timeout")
                    })?
                    .map_err(|error| {
                        std::io::Error::new(std::io::ErrorKind::BrokenPipe, error.to_string())
                    })
            })
            .map_err(|error| {
                eprintln!("torca-iroh: radio write failed: {error}");
                error
            })
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.runtime
            .block_on(async {
                tokio::time::timeout(RADIO_WRITE_TIMEOUT, self.send.flush())
                    .await
                    .map_err(|_| {
                        std::io::Error::new(std::io::ErrorKind::TimedOut, "radio flush timeout")
                    })?
                    .map_err(|error| {
                        std::io::Error::new(std::io::ErrorKind::BrokenPipe, error.to_string())
                    })
            })
            .map_err(|error| {
                eprintln!("torca-iroh: radio flush failed: {error}");
                error
            })
    }
}

impl torca_radio_adapters::RadioMediaStream for IrohRadioMediaStream {
    fn configure(
        &self,
        read: Duration,
        _write: Duration,
    ) -> Result<(), torca_radio_coordinator::RadioApplicationError> {
        // `LiveSession::pump_live` performs its first read immediately after
        // sending Hello.  Without an initial deadline this first read used to
        // wait forever when the remote Radio worker had not opened/answered
        // yet, so the media worker could not process commands, floor timeout,
        // or reconnect.  Keep the provider boundary blocking, but guarantee
        // that every provider honours the common bounded-read contract.
        self.read_timeout
            .lock()
            .map_err(|_| torca_radio_coordinator::RadioApplicationError::MediaTransport)
            .map(|mut value| *value = Some(read))
    }

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
    fn capabilities(&self) -> torca_transport_api::RealtimeCapabilities {
        torca_transport_api::RealtimeCapabilities {
            reliable: true,
            ordered: true,
            // The current media adapter is stream-backed. Keep the capability
            // truthful until a dedicated QUIC datagram lane is implemented;
            // advertising datagrams here would make the coordinator select a
            // path that this provider cannot actually service.
            supports_datagrams: false,
            max_frame_size: 64 * 1024,
            max_idle_interval_ms: 10_000,
            requires_application_keep_alive: false,
        }
    }

    fn connect(
        &mut self,
        route: &torca_radio_adapters::RadioMediaRoute,
        timeout: Duration,
    ) -> Result<
        Box<dyn torca_radio_adapters::RadioMediaStream>,
        torca_radio_coordinator::RadioApplicationError,
    > {
        if route.provider != iroh_provider_id().as_str() {
            return Err(torca_radio_coordinator::RadioApplicationError::MediaEndpointUnavailable);
        }
        // A platform network transition invalidates the local route before
        // Iroh finishes migrating the endpoint.  Refuse a Radio dial during
        // that short window; the coordinator retries after the provider
        // waker reports a fresh route instead of entering a reconnect loop.
        if !self.endpoint.route_is_fresh() {
            return Err(torca_radio_coordinator::RadioApplicationError::MediaEndpointUnavailable);
        }
        let remote = decode_endpoint_addr(&route.endpoint).map_err(|error| {
            eprintln!("torca-iroh: radio dial failed: {error}");
            torca_radio_coordinator::RadioApplicationError::MediaEndpointUnavailable
        })?;
        let endpoint = self
            .endpoint
            .current()
            .ok_or(torca_radio_coordinator::RadioApplicationError::MediaEndpointUnavailable)?;
        let connection = self
            .runtime
            .block_on(async {
                tokio::time::timeout(timeout, endpoint.connect(remote, RADIO_ALPN))
                    .await
                    .map_err(|_| ())?
                    .map_err(|_| ())
            })
            .map_err(|_| {
                eprintln!("torca-iroh: radio dial timed out or was rejected");
                torca_radio_coordinator::RadioApplicationError::MediaConnectTimeout
            })?;
        let (send, recv) = self
            .runtime
            .block_on(async {
                tokio::time::timeout(timeout, connection.open_bi())
                    .await
                    .map_err(|_| ())?
                    .map_err(|_| ())
            })
            .map_err(|_| {
                eprintln!("torca-iroh: radio outgoing stream open timed out or was rejected");
                torca_radio_coordinator::RadioApplicationError::MediaConnectTimeout
            })?;
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
        if self.endpoint.current().is_none() {
            return Ok(None);
        }
        let Some((connection, send, recv)) = self.incoming.take_radio_stream() else {
            return Ok(None);
        };
        Ok(Some(Box::new(IrohRadioMediaStream {
            connection,
            send,
            recv,
            runtime: Arc::clone(&self.runtime),
            read_timeout: Mutex::new(None),
        })))
    }

    fn set_incoming_waker(&mut self, waker: Arc<dyn Fn() + Send + Sync>) {
        self.incoming.set_radio_waker(waker);
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
    #[allow(dead_code)]
    fn start(endpoint: Endpoint, runtime: Arc<Runtime>) -> Arc<Self> {
        Self::start_with_slot(
            IrohEndpointSlot::static_endpoint(endpoint, Arc::clone(&runtime)),
            runtime,
        )
    }

    fn start_with_slot(slot: ProviderEndpointSlot, runtime: Arc<Runtime>) -> Arc<Self> {
        let router = Arc::new(Self {
            peer_streams: Mutex::new(VecDeque::new()),
            pairing: Mutex::new(VecDeque::new()),
            radio_streams: Mutex::new(VecDeque::new()),
            peer_wake: Arc::new(Mutex::new(None)),
            radio_wake: Arc::new(Mutex::new(None)),
            notify: Arc::new(tokio::sync::Notify::new()),
            closed: AtomicBool::new(false),
        });
        let task_router = Arc::clone(&router);
        let listener_runtime = Arc::clone(&runtime);
        listener_runtime.spawn(async move {
            let mut observed_generation = None;
            loop {
                let Some(endpoint) = slot.wait_current().await else {
                    task_router.closed.store(true, Ordering::Release);
                    task_router.notify.notify_waiters();
                    break;
                };
                let generation = slot.generation();
                if observed_generation.is_some_and(|previous| previous != generation) {
                    // A previous endpoint may have accepted streams just before
                    // dormancy. Drop those handles before consuming the new
                    // generation so a stale peer/radio/pairing event cannot be
                    // delivered through a freshly rebound endpoint.
                    task_router.clear_pending();
                }
                observed_generation = Some(generation);
                while let Some(incoming) = endpoint.accept().await {
                    if slot.generation() != generation {
                        task_router.clear_pending();
                        break;
                    }
                    let Ok(accepted) = incoming.accept() else { continue };
                    let Ok(connection) = accepted.await else { continue };
                    match connection.alpn() {
                        value if value == PAIRING_ALPN => {
                            if let Ok(mut entries) = task_router.pairing.lock() {
                                if entries.len() >= MAX_PENDING_PAIRING_CONNECTIONS {
                                    eprintln!(
                                        "torca-iroh: dropping excess pending pairing connection"
                                    );
                                    connection.close(0_u32.into(), b"pairing queue full");
                                    continue;
                                }
                                entries.push_back(connection);
                            }
                            task_router.notify.notify_one();
                        }
                        value if value == ALPN || value == RADIO_ALPN => {
                            // Opening the first bidirectional stream is provider
                            // work, not application work. Do it on a child task so
                            // a half-open QUIC connection cannot stall the peer or
                            // radio worker and leave its state in "requesting".
                            let router = Arc::clone(&task_router);
                            let child_runtime = Arc::clone(&runtime);
                            let is_radio = value == RADIO_ALPN;
                            child_runtime.spawn(async move {
                                let accepted = tokio::time::timeout(
                                    PEER_CONNECT_TIMEOUT,
                                    connection.accept_bi(),
                                )
                                .await;
                                let Ok(Ok((send, recv))) = accepted else {
                                    eprintln!(
                                        "torca-iroh: incoming {} stream was not opened",
                                        if is_radio { "radio" } else { "peer" }
                                    );
                                    connection.close(0_u32.into(), b"stream not opened");
                                    return;
                                };
                                if is_radio {
                                    if let Ok(mut entries) = router.radio_streams.lock() {
                                        if entries.len() >= MAX_PENDING_INCOMING_STREAMS {
                                            eprintln!(
                                                "torca-iroh: dropping excess pending radio stream"
                                            );
                                            connection.close(0_u32.into(), b"radio queue full");
                                            return;
                                        }
                                        entries.push_back((connection, send, recv));
                                    }
                                    notify(&router.radio_wake);
                                } else {
                                    if let Ok(mut entries) = router.peer_streams.lock() {
                                        if entries.len() >= MAX_PENDING_INCOMING_STREAMS {
                                            eprintln!(
                                                "torca-iroh: dropping excess pending peer stream"
                                            );
                                            connection.close(0_u32.into(), b"peer queue full");
                                            return;
                                        }
                                        entries.push_back((connection, send, recv));
                                    }
                                    notify(&router.peer_wake);
                                }
                                router.notify.notify_one();
                            });
                        }
                        _ => connection.close(0_u32.into(), b"unsupported ALPN"),
                    }
                }
                // Closing a dormant endpoint invalidates any connections that
                // were accepted just before the transition. Do not retain
                // those handles across a later endpoint generation.
                if slot.generation() != generation
                    || (slot.current().is_none() && !slot.is_terminated())
                {
                    task_router.clear_pending();
                }
            }
        });
        router
    }

    fn take_peer_stream(&self) -> Option<(Connection, SendStream, RecvStream)> {
        self.peer_streams.lock().ok()?.pop_front()
    }

    fn clear_pending(&self) {
        if let Ok(mut entries) = self.peer_streams.lock() {
            for (connection, _, _) in entries.drain(..) {
                connection.close(0_u32.into(), b"endpoint generation closed");
            }
        }
        if let Ok(mut entries) = self.radio_streams.lock() {
            for (connection, _, _) in entries.drain(..) {
                connection.close(0_u32.into(), b"endpoint generation closed");
            }
        }
        if let Ok(mut entries) = self.pairing.lock() {
            for connection in entries.drain(..) {
                connection.close(0_u32.into(), b"endpoint generation closed");
            }
        }
    }

    pub(crate) fn take_pairing(&self) -> Option<Connection> {
        self.pairing.lock().ok()?.pop_front()
    }

    pub(crate) async fn wait_for_connection(&self) -> bool {
        if self.closed.load(Ordering::Acquire) {
            return false;
        }
        self.notify.notified().await;
        !self.closed.load(Ordering::Acquire)
    }

    pub(crate) fn take_radio_stream(&self) -> Option<(Connection, SendStream, RecvStream)> {
        self.radio_streams.lock().ok()?.pop_front()
    }

    pub(crate) fn set_radio_waker(&self, waker: Arc<dyn Fn() + Send + Sync>) {
        if let Ok(mut slot) = self.radio_wake.lock() {
            *slot = Some(waker);
            // The provider listener can accept a connection before the media
            // worker finishes installing its callback. Replay the wake when
            // a radio connection is already queued so it cannot remain
            // invisible until the fallback poll deadline.
            let has_pending =
                self.radio_streams.lock().map(|queue| !queue.is_empty()).unwrap_or(false);
            if has_pending {
                if let Some(callback) = slot.as_ref() {
                    callback();
                }
            }
        }
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
    let worker_threads = configured_runtime_threads();
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(worker_threads)
        .thread_name("torca-iroh")
        .enable_all()
        .build()
        .map_err(|error| IrohIdentityError::Bind(format!("create Iroh runtime: {error}")))
}

fn configured_runtime_threads() -> usize {
    let default_threads = if cfg!(target_os = "android") { 2 } else { 4 };
    // `build.rs` emits an explicit empty value when no build-time override
    // exists. Treat that as the packaged default rather than consulting the
    // host process environment after artifact verification.
    let configured_threads = COMPILED_IROH_RUNTIME_THREADS.map(str::to_owned);
    configured_threads
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|threads| (1..=8).contains(threads))
        .unwrap_or(default_threads)
}

/// Direct Iroh transport for the shared pairing request protocol. It is
/// intentionally separate from `IrohTransport`: pairing has request/response
/// framing while authenticated peer traffic is a long-lived byte stream.
pub struct IrohPairingServiceTransport {
    endpoint: ProviderEndpointSlot,
    remote: EndpointAddr,
    runtime: Arc<Runtime>,
    connection: Option<Connection>,
}

impl IrohPairingServiceTransport {
    pub fn new(endpoint: Endpoint, remote: EndpointAddr, runtime: Arc<Runtime>) -> Self {
        Self::new_with_slot(
            IrohEndpointSlot::static_endpoint(endpoint, Arc::clone(&runtime)),
            remote,
            runtime,
        )
    }

    pub fn new_with_slot(
        endpoint: ProviderEndpointSlot,
        remote: EndpointAddr,
        runtime: Arc<Runtime>,
    ) -> Self {
        Self { endpoint, remote, runtime, connection: None }
    }

    pub fn from_bootstrap(
        endpoint: ProviderEndpointSlot,
        descriptor: &PairingBootstrapDescriptor,
        runtime: Arc<Runtime>,
    ) -> Result<Self, String> {
        if descriptor.provider() != "iroh" {
            return Err("pairing bootstrap belongs to another provider".into());
        }
        let remote =
            decode_endpoint_addr(descriptor.payload()).map_err(|error| error.to_string())?;
        Ok(Self::new_with_slot(endpoint, remote, runtime))
    }

    fn transport_error(
        kind: PairingServiceTransportFailureKind,
        sent: bool,
    ) -> PairingServiceTransportError {
        PairingServiceTransportError { kind, request_was_sent: sent }
    }
}

impl PairingServiceTransport for IrohPairingServiceTransport {
    fn invalidate(&mut self) {
        if let Some(connection) = self.connection.take() {
            connection.close(0_u32.into(), b"pairing reset");
        }
    }

    fn reconnect(&mut self) -> Result<(), PairingServiceTransportError> {
        self.invalidate();
        // The pairing descriptor is an opaque snapshot of the provider route.
        // Never dial it while this endpoint is between network generations;
        // report a retryable unavailable result to the rendezvous client.
        if !self.endpoint.route_is_fresh() {
            return Err(Self::transport_error(
                PairingServiceTransportFailureKind::Unavailable,
                false,
            ));
        }
        let endpoint = self.endpoint.current().ok_or_else(|| {
            Self::transport_error(PairingServiceTransportFailureKind::Unavailable, false)
        })?;
        let connection =
            self.runtime.block_on(endpoint.connect(self.remote.clone(), PAIRING_ALPN)).map_err(
                |_| Self::transport_error(PairingServiceTransportFailureKind::Unavailable, false),
            )?;
        self.connection = Some(connection);
        Ok(())
    }

    fn exchange(
        &mut self,
        request: &PairingServiceRequest,
        timeout: Duration,
    ) -> Result<PairingServiceResponse, PairingServiceTransportError> {
        let Some(connection) = self.connection.clone() else {
            return Err(Self::transport_error(
                PairingServiceTransportFailureKind::Unavailable,
                false,
            ));
        };
        let frame = PairingServiceCodec::encode_request(request).map_err(|_| {
            Self::transport_error(PairingServiceTransportFailureKind::InvalidResponse, false)
        })?;
        let result = self.runtime.block_on(async move {
            let (mut send, mut recv) = tokio::time::timeout(timeout, connection.open_bi())
                .await
                .map_err(|_| PairingServiceTransportFailureKind::Timeout)?
                .map_err(|_| PairingServiceTransportFailureKind::Disconnected)?;
            tokio::time::timeout(timeout, send.write_all(&frame))
                .await
                .map_err(|_| PairingServiceTransportFailureKind::Timeout)?
                .map_err(|_| PairingServiceTransportFailureKind::Disconnected)?;
            send.finish().map_err(|_| PairingServiceTransportFailureKind::Disconnected)?;
            let mut header = [0_u8; PAIRING_SERVICE_HEADER_LEN];
            tokio::time::timeout(timeout, recv.read_exact(&mut header))
                .await
                .map_err(|_| PairingServiceTransportFailureKind::Timeout)?
                .map_err(|_| PairingServiceTransportFailureKind::Disconnected)?;
            let frame_len = PairingServiceCodec::frame_len_from_header(&header)
                .map_err(|_| PairingServiceTransportFailureKind::InvalidResponse)?;
            let mut response = Vec::with_capacity(frame_len);
            response.extend_from_slice(&header);
            let mut payload = vec![0_u8; frame_len - PAIRING_SERVICE_HEADER_LEN];
            if !payload.is_empty() {
                tokio::time::timeout(timeout, recv.read_exact(&mut payload))
                    .await
                    .map_err(|_| PairingServiceTransportFailureKind::Timeout)?
                    .map_err(|_| PairingServiceTransportFailureKind::Disconnected)?;
                response.extend_from_slice(&payload);
            }
            PairingServiceCodec::decode_response(&response)
                .map_err(|_| PairingServiceTransportFailureKind::InvalidResponse)
        });
        result.map_err(|kind| Self::transport_error(kind, true))
    }
}

impl IrohComposition {
    pub fn bind(
        runtime: Arc<Runtime>,
        store: &mut dyn ProtectedSecretStore,
    ) -> Result<Self, IrohIdentityError> {
        let profile = IrohEndpointProfile::from_environment();
        let local_only = configured_flag(COMPILED_IROH_LOCAL_ONLY, "TORCA_IROH_LOCAL_ONLY")
            || matches!(profile, IrohEndpointProfile::LocalOnly);
        Self::bind_with_profile(runtime, store, profile, local_only)
    }

    /// Binds a composition with an explicit deployment profile. Native hosts
    /// normally use [`Self::bind`]; conformance and deployment tools use this
    /// entry point so their topology does not depend on process environment.
    pub fn bind_with_profile(
        runtime: Arc<Runtime>,
        store: &mut dyn ProtectedSecretStore,
        profile: IrohEndpointProfile,
        local_only: bool,
    ) -> Result<Self, IrohIdentityError> {
        let secret = load_or_create_endpoint_secret(store)?;
        let endpoint = bind_endpoint_from_secret(&runtime, secret.clone(), profile, local_only)?;
        let slot = IrohEndpointSlot::new(
            endpoint.clone(),
            Arc::clone(&runtime),
            secret,
            profile,
            local_only,
        );
        let incoming = IrohIncomingRouter::start_with_slot(Arc::clone(&slot), Arc::clone(&runtime));
        let lifecycle = IrohLifecycle::new_with_slot(Arc::clone(&slot), Arc::clone(&runtime));
        let transport_factory = IrohTransportFactory::new_with_slot(
            Arc::clone(&slot),
            Arc::clone(&runtime),
            Arc::clone(&incoming),
        );
        let radio_media_factory = IrohRadioMediaSystemFactory::new_with_slot(
            Arc::clone(&slot),
            Arc::clone(&runtime),
            Arc::clone(&incoming),
        );
        // Peer traffic and pairing share one provider-owned endpoint. Both
        // protocols are already separated by ALPN, while a second endpoint
        // would duplicate Iroh's network watchers, relay discovery and
        // Android native worker threads for no functional benefit.
        let pairing = IrohPairingService::new_with_slot(slot, Arc::clone(&runtime), incoming);
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
/// infrastructure is available for incoming reachability.
pub struct IrohLifecycle {
    endpoint: ProviderEndpointSlot,
    runtime: Arc<Runtime>,
    online: Arc<AtomicBool>,
    /// Reachability is demand-driven.  Keeping the endpoint bound is cheap,
    /// but an `online()` network report can wake cellular radios and relay
    /// workers, so it must only run while incoming reachability is requested.
    reachability_demand: Arc<AtomicBool>,
    /// Prevents a network callback and a foreground command from spawning
    /// overlapping `online()` futures for the same endpoint generation.
    online_probe_in_flight: Arc<AtomicBool>,
    /// Serializes provider online reports across network generations. A new
    /// generation waits for the cancelled report instead of losing its wake.
    online_probe_serial: Arc<tokio::sync::Mutex<()>>,
    /// Cancels an in-flight online report when demand is withdrawn or the
    /// platform network generation changes.
    online_probe_cancel: Arc<tokio::sync::Notify>,
    stopped: Arc<AtomicBool>,
    dormant: Arc<AtomicBool>,
    network_generation: Arc<AtomicU64>,
    /// Iroh endpoint migration is asynchronous. Serialize migrations so two
    /// rapid Android network callbacks cannot update the same endpoint in
    /// parallel and publish an older route after a newer generation.
    network_change_serial: Arc<tokio::sync::Mutex<()>>,
    online_probe_attempts: Arc<AtomicU64>,
    online_probe_failures: Arc<AtomicU64>,
    wake: Wake,
}

impl IrohLifecycle {
    fn spawn_online_probe(
        endpoint: Endpoint,
        runtime: &Arc<Runtime>,
        online: &Arc<AtomicBool>,
        stopped: &Arc<AtomicBool>,
        dormant: &Arc<AtomicBool>,
        reachability_demand: &Arc<AtomicBool>,
        probe_in_flight: &Arc<AtomicBool>,
        probe_serial: &Arc<tokio::sync::Mutex<()>>,
        probe_cancel: &Arc<tokio::sync::Notify>,
        route_generation: &Arc<AtomicU64>,
        generation: &Arc<AtomicU64>,
        wake: &Wake,
        probe_attempts: &Arc<AtomicU64>,
        probe_failures: &Arc<AtomicU64>,
        probe_generation: u64,
    ) {
        let online = Arc::clone(online);
        let stopped = Arc::clone(stopped);
        let dormant = Arc::clone(dormant);
        let reachability_demand = Arc::clone(reachability_demand);
        let probe_in_flight = Arc::clone(probe_in_flight);
        let probe_serial = Arc::clone(probe_serial);
        let probe_cancel = Arc::clone(probe_cancel);
        let route_generation = Arc::clone(route_generation);
        let generation = Arc::clone(generation);
        let wake = Arc::clone(wake);
        let probe_attempts = Arc::clone(probe_attempts);
        let probe_failures = Arc::clone(probe_failures);
        runtime.spawn(async move {
            // A replacement probe must wait for the cancelled generation to
            // release the endpoint. Returning while the old probe is in
            // flight can lose the only post-migration reachability attempt.
            let _serial_guard = probe_serial.lock().await;
            // `Endpoint::online` has no timeout and can otherwise leave the
            // provider stuck in Publishing forever on a captive/offline
            // network. A bounded probe keeps commissioning responsive and
            // lets a later network event retry it.
            if stopped.load(Ordering::Acquire)
                || dormant.load(Ordering::Acquire)
                || !reachability_demand.load(Ordering::Acquire)
                || generation.load(Ordering::Acquire) != probe_generation
            {
                return;
            }
            probe_in_flight.store(true, Ordering::Release);
            let _probe_guard = OnlineProbeGuard(probe_in_flight);
            let mut retry_delay = Duration::from_secs(30);
            let mut attempts = 0_u32;
            loop {
                if stopped.load(Ordering::Acquire)
                    || dormant.load(Ordering::Acquire)
                    || !reachability_demand.load(Ordering::Acquire)
                    || generation.load(Ordering::Acquire) != probe_generation
                {
                    return;
                }
                attempts = attempts.saturating_add(1);
                probe_attempts.fetch_add(1, Ordering::Relaxed);
                let route_before = endpoint.addr();
                let reachable = tokio::select! {
                    _ = probe_cancel.notified() => return,
                    result = tokio::time::timeout(Duration::from_secs(30), endpoint.online()) => {
                        // Iroh's `online()` future resolves to `()`: a
                        // completed future is the provider's positive online
                        // evidence, while `Err(Elapsed)` means the bounded
                        // reachability attempt did not complete. Do not
                        // interpret the timeout wrapper as a nested provider
                        // result (there is no inner `Result` in this API).
                        result.is_ok()
                    }
                };
                if endpoint.addr() != route_before {
                    route_generation.fetch_add(1, Ordering::AcqRel);
                }
                if stopped.load(Ordering::Acquire)
                    || dormant.load(Ordering::Acquire)
                    || generation.load(Ordering::Acquire) != probe_generation
                {
                    return;
                }
                online.store(reachable, Ordering::Release);
                notify(&wake);
                if reachable {
                    return;
                }
                probe_failures.fetch_add(1, Ordering::Relaxed);
                if attempts >= MAX_ONLINE_PROBE_ATTEMPTS {
                    // Keep the endpoint usable for local UI and outbound
                    // demand, but stop application-owned work until a
                    // network event or lifecycle wake provides new evidence.
                    return;
                }
                // Cancellation is part of the demand contract. Do not leave
                // a retry task sleeping for the full backoff after the user
                // closes the screen, the app backgrounds, or a newer network
                // generation supersedes this probe.
                tokio::select! {
                    _ = probe_cancel.notified() => return,
                    _ = tokio::time::sleep(retry_delay) => {}
                }
                retry_delay = (retry_delay * 2).min(Duration::from_secs(5 * 60));
            }
        });
    }

    /// Takes ownership of an endpoint already bound by the native provider
    /// composition.  Identity persistence and endpoint construction belong to
    /// that composition, never to the shared runtime.
    pub fn new(endpoint: Endpoint, runtime: Arc<Runtime>) -> Self {
        let endpoint_slot = IrohEndpointSlot::static_endpoint(endpoint, Arc::clone(&runtime));
        Self::new_with_slot(endpoint_slot, runtime)
    }

    pub(crate) fn new_with_slot(endpoint: ProviderEndpointSlot, runtime: Arc<Runtime>) -> Self {
        let online = Arc::new(AtomicBool::new(false));
        let reachability_demand = Arc::new(AtomicBool::new(false));
        let online_probe_in_flight = Arc::new(AtomicBool::new(false));
        let online_probe_serial = Arc::new(tokio::sync::Mutex::new(()));
        let online_probe_cancel = Arc::new(tokio::sync::Notify::new());
        let stopped = Arc::new(AtomicBool::new(false));
        let dormant = Arc::new(AtomicBool::new(false));
        let network_generation = Arc::new(AtomicU64::new(0));
        let network_change_serial = Arc::new(tokio::sync::Mutex::new(()));
        let online_probe_attempts = Arc::new(AtomicU64::new(0));
        let online_probe_failures = Arc::new(AtomicU64::new(0));
        let wake = Arc::new(Mutex::new(None));
        Self {
            endpoint,
            runtime,
            online,
            reachability_demand,
            online_probe_in_flight,
            online_probe_serial,
            online_probe_cancel,
            stopped,
            dormant,
            network_generation,
            network_change_serial,
            online_probe_attempts,
            online_probe_failures,
            wake,
        }
    }

    fn endpoint_summary(&self) -> String {
        self.endpoint
            .address()
            .map(|address| format!("iroh:{}", address.id))
            .unwrap_or_else(|| "iroh:dormant".to_owned())
    }

    fn endpoint_route_state(&self) -> ProviderRouteState {
        if !self.endpoint.route_is_fresh() {
            ProviderRouteState::Stale
        } else if self.endpoint.address().is_some() {
            ProviderRouteState::Fresh
        } else {
            ProviderRouteState::Unavailable
        }
    }

    /// Produces the short-lived direct-QR bootstrap descriptor for this bound
    /// endpoint. It contains only the serialised Iroh address needed to open
    /// the initial pairing stream; it is not a Torca identity or contact route.
    pub fn pairing_bootstrap_descriptor(&self) -> Result<PairingBootstrapDescriptor, String> {
        if !self.endpoint.route_is_fresh() {
            return Err("Iroh endpoint route is migrating".to_owned());
        }
        let address =
            self.endpoint.address().ok_or_else(|| "Iroh endpoint is dormant".to_owned())?;
        if address.is_empty() {
            return Err("Iroh endpoint has no dialable transport address yet".into());
        }
        let payload = encode_endpoint_addr(&address).map_err(|error| error.to_string())?;
        PairingBootstrapDescriptor::new("iroh", payload).map_err(|error| error.to_string())
    }

    pub fn peer_endpoint_bytes(&self) -> Result<Vec<u8>, String> {
        if !self.endpoint.route_is_fresh() {
            return Err("Iroh endpoint route is migrating".to_owned());
        }
        let address =
            self.endpoint.address().ok_or_else(|| "Iroh endpoint is dormant".to_owned())?;
        encode_endpoint_addr(&address).map_err(|error| error.to_string())
    }
}

impl CommunicationLifecycle for IrohLifecycle {
    fn provider_id(&self) -> ProviderId {
        iroh_provider_id()
    }

    fn provider_profile(&self) -> Option<&'static str> {
        Some(self.endpoint.profile().wire_value())
    }

    fn background_grace(&self) -> Duration {
        // Direct/local endpoints have no discovery or relay workers to keep
        // warm, so UI-only attention can be released quickly. The
        // relay-backed profile keeps a short handoff window for an in-flight
        // foreground transition without creating a periodic wakeup.
        match self.endpoint.profile() {
            IrohEndpointProfile::DirectOnly | IrohEndpointProfile::LocalOnly => {
                Duration::from_secs(5)
            }
            IrohEndpointProfile::AlwaysReachable => Duration::from_secs(15),
        }
    }

    fn runtime_diagnostics(&self) -> ProviderRuntimeDiagnostics {
        ProviderRuntimeDiagnostics {
            endpoint_generation: Some(self.endpoint.generation()),
            route_generation: Some(self.endpoint.route_generation()),
            network_generation: Some(self.network_generation.load(Ordering::Acquire)),
            endpoint_active: Some(self.endpoint.current().is_some()),
            route_fresh: Some(self.endpoint.route_is_fresh()),
            route_state: Some(self.endpoint_route_state()),
            runtime_threads: u16::try_from(configured_runtime_threads()).ok(),
            energy_class: Some(self.endpoint.profile().energy_class()),
            reachability_demanded: Some(self.reachability_demand.load(Ordering::Acquire)),
            online_probe_attempts: Some(self.online_probe_attempts.load(Ordering::Relaxed)),
            online_probe_failures: Some(self.online_probe_failures.load(Ordering::Relaxed)),
            incoming_reachability: Some(
                match self.incoming_reachability_state() {
                    IncomingReachabilityState::Unknown => "unknown",
                    IncomingReachabilityState::Publishing => "publishing",
                    IncomingReachabilityState::Reachable => "reachable",
                    IncomingReachabilityState::Degraded => "degraded",
                    IncomingReachabilityState::Failed => "failed",
                    IncomingReachabilityState::Stopped => "stopped",
                }
                .to_owned(),
            ),
        }
    }

    fn maintenance(&mut self, _now: Timestamp) -> Result<(), RuntimeDriverError> {
        if self.endpoint.current().is_some_and(|endpoint| endpoint.is_closed())
            && !self.stopped.load(Ordering::Acquire)
        {
            return Err(RuntimeDriverError::Communication);
        }
        Ok(())
    }

    fn network_changed(&mut self, _now: Timestamp) {
        let generation = self.network_generation.fetch_add(1, Ordering::AcqRel) + 1;
        // Invalidate the previously advertised address synchronously. The
        // endpoint migration below is asynchronous, so no route update may
        // be emitted during this gap.
        self.endpoint.route_stale.store(true, Ordering::Release);
        self.endpoint.route_generation.fetch_add(1, Ordering::AcqRel);
        self.online_probe_cancel.notify_waiters();
        self.online.store(false, Ordering::Release);
        if let Some(endpoint) = self.endpoint.current() {
            let runtime = Arc::clone(&self.runtime);
            let route_generation = Arc::clone(&self.endpoint.route_generation);
            let endpoint_slot = Arc::clone(&self.endpoint);
            let network_generation = Arc::clone(&self.network_generation);
            let network_change_serial = Arc::clone(&self.network_change_serial);
            let stopped = Arc::clone(&self.stopped);
            let wake = Arc::clone(&self.wake);
            let route_before = endpoint.addr();
            runtime.spawn(async move {
                let _migration_guard = network_change_serial.lock().await;
                // A newer platform event superseded this migration while it
                // was queued. Let that newer task perform the migration and
                // never clear route_stale for an obsolete generation.
                if stopped.load(Ordering::Acquire)
                    || network_generation.load(Ordering::Acquire) != generation
                {
                    return;
                }
                // Route migration is provider-owned work, but it must still
                // have a terminal state. A platform callback can arrive
                // while the network is captive or already gone; do not leave
                // a Tokio worker awaiting migration forever in that case.
                if tokio::time::timeout(NETWORK_CHANGE_TIMEOUT, endpoint.network_change())
                    .await
                    .is_err()
                {
                    notify(&wake);
                    return;
                }
                if stopped.load(Ordering::Acquire)
                    || network_generation.load(Ordering::Acquire) != generation
                {
                    return;
                }
                // Iroh updates concrete direct/relay addresses asynchronously.
                // Only after that update is complete may the route be
                // advertised again. A further generation bump records a
                // concrete address change separately from invalidation.
                if endpoint.addr() != route_before {
                    route_generation.fetch_add(1, Ordering::AcqRel);
                }
                // `route_stale` is set by the owner before this task starts;
                // the endpoint address now represents the post-migration
                // route, even when its serialized value remained unchanged.
                endpoint_slot.route_stale.store(false, Ordering::Release);
                notify(&wake);
            });
        }
        let Some(endpoint) = self.endpoint.current() else {
            notify(&self.wake);
            return;
        };
        if self.endpoint.profile().supports_incoming_reachability()
            && self.reachability_demand.load(Ordering::Acquire)
        {
            Self::spawn_online_probe(
                endpoint,
                &self.runtime,
                &self.online,
                &self.stopped,
                &self.dormant,
                &self.reachability_demand,
                &self.online_probe_in_flight,
                &self.online_probe_serial,
                &self.online_probe_cancel,
                &self.endpoint.route_generation,
                &self.network_generation,
                &self.wake,
                &self.online_probe_attempts,
                &self.online_probe_failures,
                generation,
            );
        }
        notify(&self.wake);
    }

    fn set_waker(&mut self, waker: Arc<dyn Fn() + Send + Sync>) {
        if let Ok(mut slot) = self.wake.lock() {
            *slot = Some(waker);
        }
    }

    fn set_reachability_demand(&mut self, demanded: bool) {
        let was_demanded = self.reachability_demand.swap(demanded, Ordering::AcqRel);
        if !demanded {
            // A cancelled probe will notice the flag on its next await and
            // the guard releases the single-flight gate.  Clear evidence
            // immediately so commissioning never reports a stale reachable
            // state after the caller has withdrawn the lease.
            self.online.store(false, Ordering::Release);
            self.online_probe_cancel.notify_waiters();
        } else if !was_demanded
            && !self.dormant.load(Ordering::Acquire)
            && !self.stopped.load(Ordering::Acquire)
            && self.endpoint.profile().supports_incoming_reachability()
            && !self.online.load(Ordering::Acquire)
            && let Some(endpoint) = self.endpoint.current()
        {
            Self::spawn_online_probe(
                endpoint,
                &self.runtime,
                &self.online,
                &self.stopped,
                &self.dormant,
                &self.reachability_demand,
                &self.online_probe_in_flight,
                &self.online_probe_serial,
                &self.online_probe_cancel,
                &self.endpoint.route_generation,
                &self.network_generation,
                &self.wake,
                &self.online_probe_attempts,
                &self.online_probe_failures,
                self.network_generation.load(Ordering::Acquire),
            );
        }
        if was_demanded != demanded {
            notify(&self.wake);
        }
    }

    fn set_dormant(&mut self, dormant: bool) -> Result<(), RuntimeDriverError> {
        // Iroh 1.x has no mutable relay-mode equivalent to Tor's
        // SoftDormant. We nevertheless make the application policy truthful:
        // dormant suppresses reachability evidence and prevents new online
        // probes; foreground/durable demand starts a fresh bounded probe.
        let was_dormant = self.dormant.swap(dormant, Ordering::AcqRel);
        // Invalidate an in-flight online probe whenever the endpoint
        // generation changes. Calling set_dormant(false) repeatedly is a
        // no-op for generation purposes, so normal network commands do not
        // cancel a useful probe.
        let generation = if was_dormant == dormant {
            self.network_generation.load(Ordering::Acquire)
        } else {
            self.network_generation.fetch_add(1, Ordering::AcqRel) + 1
        };
        self.online.store(false, Ordering::Release);
        if was_dormant != dormant {
            self.online_probe_cancel.notify_waiters();
        }
        if dormant {
            // Direct/local profiles already have relay and address-lookup
            // disabled. Closing their UDP socket buys little, but rebinding
            // on resume can change the advertised port and invalidate the
            // opaque endpoint stored in existing contacts. Keep this cheap,
            // route-stable listener alive and only suppress reachability
            // probes. The relay-backed profile still closes fully to release
            // its background network tasks and socket activity.
            if self.endpoint.profile().supports_incoming_reachability() {
                self.endpoint.deactivate();
            }
        } else if was_dormant {
            if self.endpoint.current().is_none() {
                self.endpoint.activate().map_err(|_| RuntimeDriverError::Communication)?;
            }
        }
        if !dormant
            && was_dormant
            && self.endpoint.profile().supports_incoming_reachability()
            && self.reachability_demand.load(Ordering::Acquire)
        {
            Self::spawn_online_probe(
                self.endpoint.current().ok_or(RuntimeDriverError::Communication)?,
                &self.runtime,
                &self.online,
                &self.stopped,
                &self.dormant,
                &self.reachability_demand,
                &self.online_probe_in_flight,
                &self.online_probe_serial,
                &self.online_probe_cancel,
                &self.endpoint.route_generation,
                &self.network_generation,
                &self.wake,
                &self.online_probe_attempts,
                &self.online_probe_failures,
                generation,
            );
        }
        notify(&self.wake);
        Ok(())
    }

    fn state(&self) -> CommunicationState {
        if self.stopped.load(Ordering::Acquire) {
            CommunicationState::Stopped
        } else if self.dormant.load(Ordering::Acquire) {
            // Dormancy intentionally closes the provider endpoint to avoid
            // background Iroh work, but the local runtime remains usable.
            CommunicationState::Ready
        } else if self.endpoint.current().is_none() {
            CommunicationState::Degraded
        } else if self.endpoint.current().is_some_and(|endpoint| endpoint.is_closed()) {
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
        } else if !self.endpoint.profile().supports_incoming_reachability() {
            // Direct/local profiles deliberately do not publish through a
            // relay or lookup service. This is a completed capability choice,
            // not a provider that is still warming up.
            IncomingReachabilityState::Stopped
        } else if self.endpoint.current().is_none() {
            IncomingReachabilityState::Degraded
        } else if self.endpoint.current().is_some_and(|endpoint| endpoint.is_closed()) {
            IncomingReachabilityState::Failed
        } else if self.dormant.load(Ordering::Acquire) {
            IncomingReachabilityState::Degraded
        } else if !self.reachability_demand.load(Ordering::Acquire) {
            // The endpoint is bound, but no runtime lease currently asks us
            // to prove public reachability. Reporting `Publishing` here would
            // make an intentionally suppressed probe look like a stuck
            // warm-up state in diagnostics and commissioning UI.
            IncomingReachabilityState::Unknown
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
            IncomingReachabilityState::Publishing => CommissioningState::Pending,
            // Iroh proves public reachability only while the runtime has a
            // matching demand (pairing, durable receive, etc.). An idle
            // endpoint is already commissioned for the local shell; treating
            // suppressed probing as Pending leaves the warm-up UI spinning
            // forever with no operation capable of completing that probe.
            IncomingReachabilityState::Unknown => CommissioningState::NotRequired,
            IncomingReachabilityState::Degraded => CommissioningState::Degraded,
            IncomingReachabilityState::Failed => CommissioningState::Failed,
            IncomingReachabilityState::Stopped => CommissioningState::NotRequired,
        };
        ProviderCommissioning {
            provider: iroh_provider_id(),
            steps: vec![
                CommissioningStep {
                    stage: CommissioningStage::LocalRuntime,
                    state: local,
                    required_for_local_shell: true,
                    required_for_pairing: true,
                    presentation: Some(CommissioningPresentation {
                        label: "Iroh endpoint",
                        pending_summary: "Binding the encrypted Iroh endpoint…",
                        ready_summary: "Iroh endpoint is ready",
                    }),
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
                    presentation: Some(CommissioningPresentation {
                        label: "Iroh reachability",
                        pending_summary: "Publishing a fresh Iroh route…",
                        ready_summary: "Iroh route can receive connections",
                    }),
                },
            ],
            endpoint_summary: self.local_endpoint_summary(),
            route_state: if !self.endpoint.route_is_fresh() {
                ProviderRouteState::Stale
            } else if self.endpoint.current().is_some() {
                ProviderRouteState::Fresh
            } else {
                ProviderRouteState::Unavailable
            },
            pairing_bootstrap: self.pairing_bootstrap_descriptor().ok(),
        }
    }

    fn shutdown(&mut self) {
        if self.stopped.swap(true, Ordering::AcqRel) {
            return;
        }
        self.online.store(false, Ordering::Release);
        self.online_probe_cancel.notify_waiters();
        self.endpoint.terminate();
    }
}

/// A reliable, ordered bidirectional Iroh stream with Torca payload framing.
pub struct IrohTransport {
    endpoint: Endpoint,
    remote: Option<EndpointAddr>,
    runtime: Arc<Runtime>,
    /// Provider-owned freshness marker. It is shared with the endpoint slot
    /// so a network callback can cancel a queued dial before it opens a QUIC
    /// connection.
    route_stale: Option<Arc<AtomicBool>>,
    /// The profile is carried with each session so diagnostics and capability
    /// checks describe the actual route policy (relay enabled vs direct/local
    /// only), not the conservative provider default.
    profile: IrohEndpointProfile,
    connection: Option<Connection>,
    sender: Option<Arc<AsyncMutex<SendStream>>>,
    inbound: Inbound,
    wake: Wake,
    /// The reader task owns the actual stream lifetime.  Keeping this bit
    /// separate from the mutable transport flag prevents a remote FIN/reset
    /// from leaving the peer link in a zombie `connected=true` state until
    /// the next write happens to fail.
    stream_alive: Arc<AtomicBool>,
    connected: bool,
    /// Last selected QUIC path. Iroh can migrate a session from relay to a
    /// direct path, so this is observed from the connection rather than
    /// inferred solely from the deployment profile.
    path: TransportPath,
}

impl IrohTransport {
    /// Creates a disconnected outgoing transport.
    pub fn new(endpoint: Endpoint, remote: EndpointAddr, runtime: Arc<Runtime>) -> Self {
        Self::new_with_profile(
            endpoint,
            remote,
            runtime,
            IrohEndpointProfile::AlwaysReachable,
            None,
        )
    }

    fn new_with_profile(
        endpoint: Endpoint,
        remote: EndpointAddr,
        runtime: Arc<Runtime>,
        profile: IrohEndpointProfile,
        route_stale: Option<Arc<AtomicBool>>,
    ) -> Self {
        Self {
            endpoint,
            remote: Some(remote),
            runtime,
            route_stale,
            profile,
            connection: None,
            sender: None,
            inbound: Arc::new((Mutex::new(VecDeque::new()), Condvar::new())),
            wake: Arc::new(Mutex::new(None)),
            stream_alive: Arc::new(AtomicBool::new(false)),
            connected: false,
            path: TransportPath {
                provider: iroh_provider_id(),
                topology: TransportTopology::Unknown,
            },
        }
    }

    /// Wraps an accepted Iroh connection. The first bidirectional stream is
    /// reserved for the Torca peer protocol.
    pub fn from_connection(
        endpoint: Endpoint,
        connection: Connection,
        runtime: Arc<Runtime>,
    ) -> Result<Self, PeerTransportError> {
        Self::from_connection_with_profile(
            endpoint,
            connection,
            runtime,
            IrohEndpointProfile::AlwaysReachable,
        )
    }

    fn from_connection_with_profile(
        endpoint: Endpoint,
        connection: Connection,
        runtime: Arc<Runtime>,
        profile: IrohEndpointProfile,
    ) -> Result<Self, PeerTransportError> {
        let path = selected_path(&connection, profile);
        let mut transport = Self {
            endpoint,
            remote: None,
            runtime,
            route_stale: None,
            profile,
            connection: Some(connection),
            sender: None,
            inbound: Arc::new((Mutex::new(VecDeque::new()), Condvar::new())),
            wake: Arc::new(Mutex::new(None)),
            stream_alive: Arc::new(AtomicBool::new(false)),
            connected: false,
            path,
        };
        transport.open_incoming_stream()?;
        Ok(transport)
    }

    fn from_accepted_stream_with_profile(
        endpoint: Endpoint,
        connection: Connection,
        sender: SendStream,
        receiver: RecvStream,
        runtime: Arc<Runtime>,
        profile: IrohEndpointProfile,
    ) -> Self {
        let path = selected_path(&connection, profile);
        let mut transport = Self {
            endpoint,
            remote: None,
            runtime,
            route_stale: None,
            profile,
            connection: Some(connection),
            sender: None,
            inbound: Arc::new((Mutex::new(VecDeque::new()), Condvar::new())),
            wake: Arc::new(Mutex::new(None)),
            stream_alive: Arc::new(AtomicBool::new(false)),
            connected: false,
            path,
        };
        transport.install_stream(sender, receiver);
        transport.connected = true;
        transport
    }

    fn open_incoming_stream(&mut self) -> Result<(), PeerTransportError> {
        let connection = self
            .connection
            .clone()
            .ok_or_else(|| PeerTransportError("Iroh connection is unavailable".into()))?;
        let accepted = self.runtime.block_on(async {
            tokio::time::timeout(PEER_CONNECT_TIMEOUT, connection.accept_bi()).await
        });
        let (sender, receiver) = match accepted {
            Ok(Ok(streams)) => streams,
            Ok(Err(error)) => {
                connection.close(0_u32.into(), b"peer stream rejected");
                return Err(PeerTransportError(format!("Iroh accept stream failed: {error}")));
            }
            Err(_) => {
                connection.close(0_u32.into(), b"peer stream timeout");
                return Err(PeerTransportError("Iroh accept stream timed out".into()));
            }
        };
        self.install_stream(sender, receiver);
        self.connected = true;
        Ok(())
    }

    fn install_stream(&mut self, sender: SendStream, mut receiver: RecvStream) {
        let inbound = Arc::clone(&self.inbound);
        let wake = Arc::clone(&self.wake);
        // Use a generation-local flag.  A reader belonging to an older stream
        // must not be able to mark a newly reconnected stream dead when its
        // task finishes after the reconnect.
        let stream_alive = Arc::new(AtomicBool::new(true));
        self.stream_alive = Arc::clone(&stream_alive);
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
                        stream_alive.store(false, Ordering::Release);
                        break;
                    }
                };
                if length > MAX_FRAME {
                    push_inbound(
                        &inbound,
                        Err(PeerTransportError("Iroh frame exceeds peer limit".into())),
                    );
                    notify(&wake);
                    stream_alive.store(false, Ordering::Release);
                    break;
                }
                let mut payload = vec![0_u8; length];
                if let Err(error) = receiver.read_exact(&mut payload).await {
                    push_inbound(
                        &inbound,
                        Err(PeerTransportError(format!("Iroh receive payload failed: {error}"))),
                    );
                    notify(&wake);
                    stream_alive.store(false, Ordering::Release);
                    break;
                }
                if !push_inbound(&inbound, Ok(payload)) {
                    // The bounded queue has reported an overflow marker and
                    // the stream reader must stop. Dropping this generation
                    // lets PeerLink reconnect/replay durable envelopes after
                    // the application drains the marker.
                    stream_alive.store(false, Ordering::Release);
                    notify(&wake);
                    break;
                }
                notify(&wake);
            }
            stream_alive.store(false, Ordering::Release);
        });
    }

    fn map_error(error: impl std::fmt::Display) -> PeerTransportError {
        PeerTransportError(format!("Iroh transport failed: {error}"))
    }
}

fn selected_path(connection: &Connection, profile: IrohEndpointProfile) -> TransportPath {
    connection
        .paths()
        .iter()
        .find(|path| path.is_selected())
        .map(|path| {
            if path.is_relay() {
                TransportPath { provider: iroh_provider_id(), topology: TransportTopology::Relay }
            } else if path.is_ip() {
                TransportPath { provider: iroh_provider_id(), topology: TransportTopology::Direct }
            } else {
                TransportPath { provider: iroh_provider_id(), topology: TransportTopology::Unknown }
            }
        })
        .unwrap_or_else(|| {
            // A path may not be published in the first snapshot immediately
            // after a handshake. Keep the profile as a conservative fallback
            // until the next reconnect/observation.
            if profile_supports_relay(profile) {
                TransportPath { provider: iroh_provider_id(), topology: TransportTopology::Relay }
            } else {
                TransportPath { provider: iroh_provider_id(), topology: TransportTopology::Direct }
            }
        })
}

impl PeerTransport for IrohTransport {
    fn connect(&mut self) -> Result<(), PeerTransportError> {
        if self.connected && self.stream_alive.load(Ordering::Acquire) {
            return Ok(());
        }
        if self.route_stale.as_ref().is_some_and(|stale| stale.load(Ordering::Acquire)) {
            return Err(PeerTransportError(
                "Iroh local route is stale; waiting for provider migration".to_owned(),
            ));
        }
        // A previous reader may have observed a remote close. Clear the
        // mutable side before dialing so reconnect never reuses a dead sender.
        self.connected = false;
        self.sender = None;
        self.connection = None;
        let remote = self
            .remote
            .clone()
            .ok_or_else(|| PeerTransportError("Iroh remote address is missing".into()))?;
        let connection = self
            .runtime
            .block_on(async {
                tokio::time::timeout(PEER_CONNECT_TIMEOUT, self.endpoint.connect(remote, ALPN))
                    .await
            })
            .map_err(|_| PeerTransportError("Iroh peer connect timed out".into()))?
            .map_err(Self::map_error)?;
        if self.route_stale.as_ref().is_some_and(|stale| stale.load(Ordering::Acquire)) {
            connection.close(0_u32.into(), b"local route migrated");
            return Err(PeerTransportError("Iroh local route changed during dial".to_owned()));
        }
        let path = selected_path(&connection, self.profile);
        let (sender, receiver) = self
            .runtime
            .block_on(async {
                tokio::time::timeout(PEER_CONNECT_TIMEOUT, connection.open_bi()).await
            })
            .map_err(|_| PeerTransportError("Iroh peer stream open timed out".into()))?
            .map_err(Self::map_error)?;
        self.connection = Some(connection);
        self.path = path;
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
        if !self.stream_alive.load(Ordering::Acquire) {
            self.connected = false;
            self.sender = None;
            self.connection = None;
            return Err(PeerTransportError("Iroh peer stream is no longer alive".into()));
        }
        let result = self.runtime.block_on(async move {
            let mut sender = sender.lock().await;
            tokio::time::timeout(PEER_WRITE_TIMEOUT, async {
                sender.write_u32(payload.len() as u32).await.map_err(Self::map_error)?;
                sender.write_all(&payload).await.map_err(Self::map_error)?;
                sender.flush().await.map_err(Self::map_error)
            })
            .await
            .map_err(|_| PeerTransportError("Iroh peer write timed out".into()))?
        });
        if result.is_err() {
            // A timed-out QUIC write is no longer a usable stream. Force the
            // next delivery attempt through the reconnect path.
            self.connected = false;
            self.sender = None;
            self.connection = None;
        }
        result
    }

    fn send_batch(&mut self, payloads: Vec<Vec<u8>>) -> Result<(), PeerTransportError> {
        if payloads.is_empty() {
            return Ok(());
        }
        if payloads.iter().any(|payload| payload.len() > MAX_FRAME) {
            return Err(PeerTransportError("Iroh payload exceeds peer limit".into()));
        }
        let sender = self
            .sender
            .clone()
            .ok_or_else(|| PeerTransportError("Iroh transport is not connected".into()))?;
        if !self.stream_alive.load(Ordering::Acquire) {
            self.connected = false;
            self.sender = None;
            self.connection = None;
            return Err(PeerTransportError("Iroh peer stream is no longer alive".into()));
        }
        let result = self.runtime.block_on(async move {
            let mut sender = sender.lock().await;
            tokio::time::timeout(PEER_WRITE_TIMEOUT, async {
                for payload in payloads {
                    sender.write_u32(payload.len() as u32).await.map_err(Self::map_error)?;
                    sender.write_all(&payload).await.map_err(Self::map_error)?;
                }
                sender.flush().await.map_err(Self::map_error)
            })
            .await
            .map_err(|_| PeerTransportError("Iroh peer batch write timed out".into()))?
        });
        if result.is_err() {
            self.connected = false;
            self.sender = None;
            self.connection = None;
        }
        result
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
        self.stream_alive.store(false, Ordering::Release);
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
    fn provider_id(&self) -> ProviderId {
        iroh_provider_id()
    }

    fn path(&self) -> TransportPath {
        // Iroh may migrate an established QUIC connection from a relay to a
        // direct path (or back) without rebuilding the stream. Observe the
        // selected path on every capability read so diagnostics and policy do
        // not report the deployment profile as the actual route.
        self.connection
            .as_ref()
            .map(|connection| selected_path(connection, self.profile))
            .unwrap_or_else(|| self.path.clone())
    }

    fn capabilities(&self) -> TransportCapabilities {
        TransportCapabilities {
            reliable: true,
            ordered: true,
            supports_incoming: true,
            supports_direct_path: true,
            supports_relay_path: profile_supports_relay(self.profile),
            hides_peer_ip: false,
            max_frame_size: MAX_FRAME,
            latency: LatencyClass::Interactive,
            energy: self.profile.energy_class(),
        }
    }
}

impl Drop for IrohTransport {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

fn push_inbound(queue: &Inbound, item: Result<Vec<u8>, PeerTransportError>) -> bool {
    let Ok(mut entries) = queue.0.lock() else { return false };
    if entries.len() >= MAX_PENDING_INBOUND_FRAMES {
        // Preserve bounded memory and make the overflow visible to the
        // transport worker. The oldest queued frame is intentionally
        // discarded: the authenticated delivery layer can request it again,
        // while retaining an unbounded queue would turn backpressure into a
        // process-wide memory problem.
        entries.pop_front();
        entries.push_back(Err(PeerTransportError(
            "Iroh inbound frame queue limit exceeded".to_owned(),
        )));
        queue.1.notify_one();
        return false;
    }
    entries.push_back(item);
    queue.1.notify_one();
    true
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
    endpoint: ProviderEndpointSlot,
    runtime: Arc<Runtime>,
    incoming: Arc<IrohIncomingRouter>,
    wake: Wake,
}

fn profile_supports_relay(profile: IrohEndpointProfile) -> bool {
    matches!(profile, IrohEndpointProfile::AlwaysReachable)
}

impl IrohTransportFactory {
    #[allow(dead_code)]
    pub(crate) fn new(
        endpoint: Endpoint,
        runtime: Arc<Runtime>,
        incoming: Arc<IrohIncomingRouter>,
    ) -> Self {
        Self::new_with_slot(
            IrohEndpointSlot::static_endpoint(endpoint, Arc::clone(&runtime)),
            runtime,
            incoming,
        )
    }

    pub(crate) fn new_with_slot(
        endpoint: ProviderEndpointSlot,
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
    fn provider_id(&self) -> ProviderId {
        iroh_provider_id()
    }

    fn capabilities(&self) -> TransportCapabilities {
        TransportCapabilities {
            reliable: true,
            ordered: true,
            supports_incoming: true,
            supports_direct_path: true,
            supports_relay_path: profile_supports_relay(self.endpoint.profile()),
            hides_peer_ip: false,
            max_frame_size: MAX_FRAME,
            latency: LatencyClass::Interactive,
            energy: self.endpoint.profile().energy_class(),
        }
    }

    fn accept(&mut self) -> Result<Option<Box<dyn PeerTransport + Send>>, TransportFactoryError> {
        let Some((connection, sender, receiver)) = self.incoming.take_peer_stream() else {
            return Ok(None);
        };
        let mut transport = IrohTransport::from_accepted_stream_with_profile(
            // The factory owns the selected profile; preserve it on the
            // session so capability consumers cannot accidentally assume a
            // relay path for a direct/local deployment.
            self.endpoint.current().ok_or(TransportFactoryError::Listener)?,
            connection,
            sender,
            receiver,
            Arc::clone(&self.runtime),
            self.endpoint.profile(),
        );
        if let Some(waker) = self.wake.lock().ok().and_then(|slot| slot.clone()) {
            transport.set_waker(waker);
        }
        Ok(Some(Box::new(transport) as Box<dyn PeerTransport + Send>))
    }

    fn connect(
        &mut self,
        contact: &Contact,
    ) -> Result<Box<dyn PeerTransport + Send>, TransportFactoryError> {
        // A network callback invalidates the local route before Iroh finishes
        // migrating the endpoint. Never create a session during that gap:
        // dialing then uses an address which is already unsafe to advertise
        // and turns a transient migration into a reconnect storm.
        if !self.endpoint.route_is_fresh() {
            return Err(TransportFactoryError::RouteStale);
        }
        let endpoint = contact
            .route()
            .provider_endpoint(iroh_provider_id().as_str())
            .ok_or(TransportFactoryError::ContactNotFound)?;
        let local_endpoint = self.endpoint.current().ok_or(TransportFactoryError::Listener)?;
        let remote = decode_endpoint_addr(endpoint).map_err(|_| TransportFactoryError::Protocol)?;
        let mut transport = IrohTransport::new_with_profile(
            local_endpoint,
            remote,
            Arc::clone(&self.runtime),
            self.endpoint.profile(),
            Some(self.endpoint.route_stale_flag()),
        );
        if let Some(waker) = self.wake.lock().ok().and_then(|slot| slot.clone()) {
            transport.set_waker(waker);
        }
        Ok(Box::new(transport))
    }

    fn preserves_sessions_on_network_change(&self) -> bool {
        // Android can replace Wi-Fi with LTE without keeping the old UDP
        // socket usable. In that case an apparently live QUIC session becomes
        // a zombie: the peer link keeps it in `Ready`, while new envelopes
        // never reach the peer. Iroh still refreshes its endpoint route in
        // `network_changed`; let the peer link close and redial the session
        // after that refresh. Durable delivery keeps queued messages safe.
        false
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
    use super::iroh_provider_id;
    use std::collections::{BTreeMap, VecDeque};
    use std::io::{Read, Write};
    use std::sync::atomic::Ordering;
    use std::sync::{Arc, Condvar, Mutex};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use iroh::endpoint::presets;
    use tokio::runtime::Runtime;
    use torca_contacts::{
        Contact, ContactError, ContactId, ContactRepository, ContactRoute, PeerCredential,
        PeerCredentialRepository,
    };
    use torca_crypto::{
        InMemoryProtectedSecretStore, ManagedIdentityKeys, OwnedHandshakeSigner,
        ProtectedSecretStore, RustCryptoProvider,
    };
    use torca_foundation::{OpaqueId, Timestamp};
    use torca_identity::{
        IdentityId, IdentityKey, IdentityKeyProvider, KeyAlgorithm, KeyId, PublicIdentity,
    };
    use torca_peer_link::{LinkAck, PeerConnectionState, PeerLink};
    use torca_peer_protocol::AckStatus;
    use torca_radio_adapters::{RadioMediaConnector, RadioMediaRoute};
    use torca_runtime::{CommunicationLifecycle, CommunicationState, IncomingReachabilityState};
    use torca_transport_api::{
        CommissioningStage, CommissioningState, EnergyClass, PeerTransport, PeerTransportFactory,
        ProviderRouteState, TransportFactoryError,
    };

    use super::{
        ALPN, IROH_ENDPOINT_SECRET_HANDLE, IrohComposition, IrohEndpointProfile, IrohEndpointSlot,
        IrohIncomingRouter, IrohLifecycle, IrohRadioMediaSystemFactory, IrohServiceConfig,
        IrohTransport, IrohTransportFactory, RADIO_ALPN, bind_endpoint, decode_endpoint_addr,
        encode_endpoint_addr, load_or_create_endpoint_secret,
    };

    fn contact_for_iroh_endpoint(endpoint: &iroh::EndpointAddr) -> Contact {
        let route = ContactRoute::for_provider_endpoint(
            OpaqueId::from_u128(31),
            iroh_provider_id().as_str(),
            encode_endpoint_addr(endpoint).expect("encode Iroh endpoint"),
        )
        .expect("valid Iroh contact route");
        let key = IdentityKey::new(KeyId::from_u128(33), KeyAlgorithm::Ed25519, vec![7; 32])
            .expect("valid remote identity key");
        Contact::new(
            ContactId::from_u128(30),
            PublicIdentity::new(IdentityId::from_u128(32), key, 0),
            route,
            Timestamp::UNIX_EPOCH,
        )
    }

    fn direct_endpoint(runtime: &Runtime) -> iroh::Endpoint {
        runtime
            .block_on(
                iroh::Endpoint::builder(presets::N0)
                    .clear_address_lookup()
                    .relay_mode(iroh::RelayMode::Disabled)
                    .alpns(vec![ALPN.to_vec()])
                    .bind_addr("127.0.0.1:0")
                    .expect("loopback bind")
                    .bind(),
            )
            .expect("bind direct Iroh endpoint")
    }

    fn direct_slot(
        endpoint: iroh::Endpoint,
        runtime: &Arc<Runtime>,
    ) -> super::ProviderEndpointSlot {
        let secret = endpoint.secret_key().clone();
        IrohEndpointSlot::new(
            endpoint,
            Arc::clone(runtime),
            secret,
            IrohEndpointProfile::DirectOnly,
            false,
        )
    }

    #[derive(Default)]
    struct TestRelationships {
        contacts: BTreeMap<ContactId, Contact>,
        credentials: BTreeMap<ContactId, PeerCredential>,
    }

    impl TestRelationships {
        fn persisted(contact: Contact, credential: PeerCredential) -> Self {
            Self {
                contacts: BTreeMap::from([(contact.id(), contact)]),
                credentials: BTreeMap::from([(credential.contact_id(), credential)]),
            }
        }
    }

    impl ContactRepository for TestRelationships {
        fn insert(&mut self, contact: Contact) -> Result<(), ContactError> {
            if self.contacts.insert(contact.id(), contact).is_some() {
                return Err(ContactError::AlreadyExists);
            }
            Ok(())
        }

        fn get(&self, id: ContactId) -> Result<Option<Contact>, ContactError> {
            Ok(self.contacts.get(&id).cloned())
        }

        fn update(&mut self, contact: Contact) -> Result<(), ContactError> {
            if !self.contacts.contains_key(&contact.id()) {
                return Err(ContactError::NotFound);
            }
            self.contacts.insert(contact.id(), contact);
            Ok(())
        }

        fn list(&self) -> Result<Vec<Contact>, ContactError> {
            Ok(self.contacts.values().cloned().collect())
        }
    }

    impl PeerCredentialRepository for TestRelationships {
        fn insert_credential(&mut self, credential: PeerCredential) -> Result<(), ContactError> {
            if self.credentials.insert(credential.contact_id(), credential).is_some() {
                return Err(ContactError::AlreadyExists);
            }
            Ok(())
        }

        fn credential_for_contact(
            &self,
            contact_id: ContactId,
        ) -> Result<Option<PeerCredential>, ContactError> {
            Ok(self.credentials.get(&contact_id).copied())
        }
    }

    fn test_identity(
        identity_id: IdentityId,
    ) -> (PublicIdentity, OwnedHandshakeSigner<RustCryptoProvider, InMemoryProtectedSecretStore>)
    {
        let mut keys =
            ManagedIdentityKeys::new(RustCryptoProvider, InMemoryProtectedSecretStore::default());
        let generated = keys.generate_signing_key().expect("generate handshake identity");
        let identity_key =
            IdentityKey::new(generated.key_id, generated.algorithm, generated.public_key)
                .expect("valid generated public key");
        let signer = OwnedHandshakeSigner::new(keys, generated.key_id);
        (PublicIdentity::new(identity_id, identity_key, 0), signer)
    }

    fn current_timestamp() -> Timestamp {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time after epoch")
            .as_millis();
        Timestamp::from_unix_millis(i64::try_from(millis).expect("timestamp fits i64"))
            .expect("valid current timestamp")
    }

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
    fn endpoint_profiles_are_explicit_and_unknown_values_are_safe() {
        assert_eq!(IrohEndpointProfile::from_wire("always"), IrohEndpointProfile::AlwaysReachable);
        assert_eq!(IrohEndpointProfile::from_wire("direct-only"), IrohEndpointProfile::DirectOnly);
        assert_eq!(IrohEndpointProfile::from_wire("local"), IrohEndpointProfile::LocalOnly);
        assert_eq!(IrohEndpointProfile::from_wire("unexpected").wire_value(), "always");
        assert!(IrohEndpointProfile::AlwaysReachable.supports_incoming_reachability());
        assert!(!IrohEndpointProfile::DirectOnly.supports_incoming_reachability());
        assert!(!IrohEndpointProfile::LocalOnly.supports_incoming_reachability());
    }

    #[test]
    fn service_configuration_is_explicit_and_direct_profiles_ignore_it() {
        let config = IrohServiceConfig::from_values(
            IrohEndpointProfile::AlwaysReachable,
            Some("https://use1-1.relay.n0.iroh.link., https://euw-1.relay.n0.iroh.link."),
            Some("https://dns.iroh.link/pkarr"),
        )
        .expect("valid custom Iroh services");
        assert!(config.is_custom());
        assert_eq!(config.relay_urls.len(), 2);
        assert!(config.pkarr_url.is_some());

        let ignored = IrohServiceConfig::from_values(
            IrohEndpointProfile::DirectOnly,
            Some("not a relay URL"),
            Some("not a URL"),
        )
        .expect("direct profile must not parse disabled services");
        assert!(!ignored.is_custom());
    }

    #[test]
    fn direct_profile_does_not_spawn_relay_probe_or_rebind_on_dormancy() {
        let runtime = Arc::new(Runtime::new().expect("Iroh Tokio runtime"));
        let endpoint = runtime
            .block_on(
                iroh::Endpoint::builder(presets::N0)
                    .clear_address_lookup()
                    .relay_mode(iroh::RelayMode::Disabled)
                    .alpns(vec![ALPN.to_vec()])
                    .bind_addr("127.0.0.1:0")
                    .expect("loopback bind")
                    .bind(),
            )
            .expect("bind direct endpoint");
        let endpoint_addr = endpoint.addr();
        let secret = endpoint.secret_key().clone();
        let slot = IrohEndpointSlot::new(
            endpoint,
            Arc::clone(&runtime),
            secret,
            IrohEndpointProfile::DirectOnly,
            false,
        );
        let mut lifecycle = IrohLifecycle::new_with_slot(Arc::clone(&slot), Arc::clone(&runtime));

        assert_eq!(lifecycle.incoming_reachability_state(), IncomingReachabilityState::Stopped);
        lifecycle.set_dormant(true).expect("enter direct dormant");
        assert!(slot.current().is_some(), "direct dormancy must preserve the bound route");
        assert_eq!(slot.current().expect("endpoint").addr(), endpoint_addr);
        lifecycle.set_dormant(false).expect("leave direct dormant");
        assert_eq!(slot.current().expect("endpoint").addr(), endpoint_addr);
        lifecycle.shutdown();
    }

    #[test]
    fn lifecycle_reports_iroh_without_tor_or_onion_projection() {
        let runtime = Arc::new(Runtime::new().expect("Iroh Tokio runtime"));
        let endpoint = runtime
            .block_on(iroh::Endpoint::builder(presets::N0).alpns(vec![ALPN.to_vec()]).bind())
            .expect("bind local Iroh endpoint");
        let endpoint_id = endpoint.addr().id;
        let mut lifecycle = IrohLifecycle::new(endpoint, Arc::clone(&runtime));

        assert_eq!(lifecycle.provider_id(), iroh_provider_id());
        assert_eq!(lifecycle.state(), CommunicationState::Ready);
        assert_eq!(lifecycle.incoming_reachability_state(), IncomingReachabilityState::Unknown);
        assert_eq!(lifecycle.runtime_diagnostics().route_state, Some(ProviderRouteState::Fresh));
        assert_eq!(lifecycle.runtime_diagnostics().energy_class, Some(EnergyClass::Medium));
        assert_eq!(lifecycle.runtime_diagnostics().reachability_demanded, Some(false));
        assert_eq!(lifecycle.runtime_diagnostics().online_probe_attempts, Some(0));
        let commissioning = lifecycle.commissioning();
        assert_eq!(commissioning.provider, iroh_provider_id());
        assert!(commissioning.endpoint_summary.is_some());
        assert_eq!(commissioning.step(CommissioningStage::LocalRuntime), CommissioningState::Ready);
        // Creating an invitation is local and must not wait for discovery to
        // mark the endpoint reachable; the join path owns that network try.
        assert!(commissioning.pairing_ready());
        assert_eq!(lifecycle.background_grace(), Duration::from_secs(15));
        let descriptor = lifecycle.pairing_bootstrap_descriptor().expect("QR bootstrap descriptor");
        assert_eq!(descriptor.provider(), "iroh");
        assert_eq!(
            decode_endpoint_addr(descriptor.payload()).expect("decode endpoint").id,
            endpoint_id
        );

        lifecycle.set_dormant(true).expect("enter dormant");
        assert_eq!(lifecycle.state(), CommunicationState::Ready);
        assert_eq!(lifecycle.incoming_reachability_state(), IncomingReachabilityState::Degraded);
        lifecycle.set_dormant(false).expect("leave dormant");
        assert_eq!(lifecycle.state(), CommunicationState::Ready);
        assert_eq!(
            lifecycle.endpoint.current().expect("reactivated endpoint").addr().id,
            endpoint_id
        );

        lifecycle.shutdown();
        assert_eq!(lifecycle.state(), CommunicationState::Stopped);
    }

    #[test]
    fn network_change_invalidates_reachability_evidence() {
        let runtime = Arc::new(Runtime::new().expect("Iroh Tokio runtime"));
        let endpoint = runtime
            .block_on(iroh::Endpoint::builder(presets::N0).alpns(vec![ALPN.to_vec()]).bind())
            .expect("bind local Iroh endpoint");
        let mut lifecycle = IrohLifecycle::new(endpoint, Arc::clone(&runtime));

        lifecycle.set_reachability_demand(true);
        lifecycle.online.store(true, Ordering::Release);
        assert_eq!(lifecycle.incoming_reachability_state(), IncomingReachabilityState::Reachable);
        let route_generation = lifecycle.endpoint.route_generation();
        lifecycle.network_changed(Timestamp::UNIX_EPOCH);
        assert_eq!(lifecycle.incoming_reachability_state(), IncomingReachabilityState::Publishing);
        assert_eq!(lifecycle.endpoint.route_generation(), route_generation + 1);

        lifecycle.shutdown();
    }

    #[test]
    fn bootstrap_descriptors_are_not_created_from_stale_routes() {
        let runtime = Arc::new(Runtime::new().expect("Iroh Tokio runtime"));
        let endpoint = runtime
            .block_on(iroh::Endpoint::builder(presets::N0).alpns(vec![ALPN.to_vec()]).bind())
            .expect("bind local Iroh endpoint");
        let slot = IrohEndpointSlot::new(
            endpoint,
            Arc::clone(&runtime),
            iroh::SecretKey::generate(),
            IrohEndpointProfile::AlwaysReachable,
            false,
        );
        let lifecycle = IrohLifecycle::new_with_slot(Arc::clone(&slot), Arc::clone(&runtime));
        let incoming = IrohIncomingRouter::start_with_slot(Arc::clone(&slot), Arc::clone(&runtime));
        let pairing = crate::pairing::IrohPairingService::new_with_slot(
            Arc::clone(&slot),
            Arc::clone(&runtime),
            incoming,
        );

        slot.route_stale.store(true, Ordering::Release);
        assert!(lifecycle.pairing_bootstrap_descriptor().is_err());
        assert!(pairing.pairing_bootstrap_descriptor().is_err());
        assert!(lifecycle.peer_endpoint_bytes().is_err());

        let mut lifecycle = lifecycle;
        lifecycle.shutdown();
    }

    #[test]
    fn inbound_frames_are_bounded_and_report_overflow() {
        let queue = Arc::new((Mutex::new(VecDeque::new()), Condvar::new()));
        for _ in 0..super::MAX_PENDING_INBOUND_FRAMES {
            assert!(super::push_inbound(&queue, Ok(vec![1])));
        }
        assert!(!super::push_inbound(&queue, Ok(vec![2])));
        let entries = queue.0.lock().expect("inbound queue");
        assert_eq!(entries.len(), super::MAX_PENDING_INBOUND_FRAMES);
        assert!(entries.back().is_some_and(Result::is_err));
    }

    #[test]
    fn authenticated_peer_lane_round_trips_after_provider_route_exchange() {
        let runtime = Arc::new(Runtime::new().expect("Iroh Tokio runtime"));
        let bind = || {
            runtime
                .block_on(
                    iroh::Endpoint::builder(presets::N0)
                        .clear_address_lookup()
                        .relay_mode(iroh::RelayMode::Disabled)
                        .alpns(vec![ALPN.to_vec()])
                        .bind_addr("127.0.0.1:0")
                        .expect("loopback bind")
                        .bind(),
                )
                .expect("bind Iroh peer endpoint")
        };
        let first = bind();
        let second = bind();
        let incoming = IrohIncomingRouter::start(second.clone(), Arc::clone(&runtime));
        let (wake_sender, wake_receiver) = std::sync::mpsc::sync_channel(1);
        incoming.set_peer_waker(Arc::new(move || {
            let _ = wake_sender.try_send(());
        }));
        let mut outgoing = IrohTransport::new_with_profile(
            first.clone(),
            second.addr(),
            Arc::clone(&runtime),
            IrohEndpointProfile::DirectOnly,
            None,
        );

        outgoing.connect().expect("dial exchanged provider route");
        // QUIC opens streams lazily: the remote endpoint observes the first
        // bidirectional stream only after the dialer writes its handshake or
        // first framed payload. PeerLink follows the same ordering.
        outgoing.send(b"paired-message".to_vec()).expect("send paired message");
        wake_receiver.recv_timeout(Duration::from_secs(3)).expect("incoming peer wake");
        let (connection, sender, receiver) =
            incoming.take_peer_stream().expect("accepted peer stream");
        let mut recipient = IrohTransport::from_accepted_stream_with_profile(
            second.clone(),
            connection,
            sender,
            receiver,
            Arc::clone(&runtime),
            IrohEndpointProfile::DirectOnly,
        );

        assert_eq!(
            recipient.receive_timeout(Duration::from_secs(1)).expect("receive paired message"),
            Some(b"paired-message".to_vec())
        );
        recipient.send(b"delivery-receipt".to_vec()).expect("send delivery receipt");
        assert_eq!(
            outgoing.receive_timeout(Duration::from_secs(1)).expect("receive delivery receipt"),
            Some(b"delivery-receipt".to_vec())
        );

        outgoing.close().expect("close outgoing transport");
        recipient.close().expect("close incoming transport");
        runtime.block_on(first.close());
        runtime.block_on(second.close());
    }

    #[test]
    fn queued_peer_dial_is_rejected_when_route_becomes_stale() {
        let runtime = Arc::new(Runtime::new().expect("Iroh Tokio runtime"));
        let endpoint = runtime
            .block_on(
                iroh::Endpoint::builder(presets::N0)
                    .alpns(vec![ALPN.to_vec()])
                    .bind_addr("127.0.0.1:0")
                    .expect("loopback bind")
                    .bind(),
            )
            .expect("bind local Iroh endpoint");
        let route_stale = Arc::new(std::sync::atomic::AtomicBool::new(true));
        let mut transport = IrohTransport::new_with_profile(
            endpoint.clone(),
            endpoint.addr(),
            Arc::clone(&runtime),
            IrohEndpointProfile::DirectOnly,
            Some(route_stale),
        );
        let error = transport.connect().expect_err("stale route must not dial");
        assert!(error.0.contains("route is stale"));
        runtime.block_on(endpoint.close());
    }

    #[test]
    fn factory_created_peer_transport_connects_with_a_fresh_route() {
        let runtime = Arc::new(Runtime::new().expect("Iroh Tokio runtime"));
        let first = direct_endpoint(&runtime);
        let second = direct_endpoint(&runtime);
        let first_slot = direct_slot(first.clone(), &runtime);
        let second_slot = direct_slot(second.clone(), &runtime);
        let first_incoming =
            IrohIncomingRouter::start_with_slot(Arc::clone(&first_slot), Arc::clone(&runtime));
        let second_incoming =
            IrohIncomingRouter::start_with_slot(Arc::clone(&second_slot), Arc::clone(&runtime));
        let mut first_factory =
            IrohTransportFactory::new_with_slot(first_slot, Arc::clone(&runtime), first_incoming);
        let mut second_factory =
            IrohTransportFactory::new_with_slot(second_slot, Arc::clone(&runtime), second_incoming);
        let contact = contact_for_iroh_endpoint(&second.addr());

        let mut outgoing = first_factory.connect(&contact).expect("fresh factory route");
        outgoing.connect().expect("connect factory-created transport");
        outgoing.send(b"factory-route".to_vec()).expect("send through factory transport");

        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        let mut accepted = loop {
            if let Some(transport) = second_factory.accept().expect("accept remote transport") {
                break transport;
            }
            assert!(std::time::Instant::now() < deadline, "remote router did not accept stream");
            std::thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(
            accepted.receive_timeout(Duration::from_secs(1)).expect("receive factory payload"),
            Some(b"factory-route".to_vec())
        );

        outgoing.close().expect("close outgoing transport");
        accepted.close().expect("close accepted transport");
        runtime.block_on(first.close());
        runtime.block_on(second.close());
    }

    #[test]
    fn factory_rejects_a_route_that_is_stale_before_transport_creation() {
        let runtime = Arc::new(Runtime::new().expect("Iroh Tokio runtime"));
        let first = direct_endpoint(&runtime);
        let second = direct_endpoint(&runtime);
        let first_slot = direct_slot(first.clone(), &runtime);
        let second_slot = direct_slot(second.clone(), &runtime);
        let incoming =
            IrohIncomingRouter::start_with_slot(Arc::clone(&first_slot), Arc::clone(&runtime));
        let second_incoming =
            IrohIncomingRouter::start_with_slot(Arc::clone(&second_slot), Arc::clone(&runtime));
        let mut factory = IrohTransportFactory::new_with_slot(
            Arc::clone(&first_slot),
            Arc::clone(&runtime),
            incoming,
        );
        let contact = contact_for_iroh_endpoint(&second.addr());
        first_slot.route_stale.store(true, Ordering::Release);

        assert!(matches!(factory.connect(&contact), Err(TransportFactoryError::RouteStale)));

        // A provider refresh must reopen the factory path; the stale guard is
        // a transient dial barrier, not a permanent disablement of the peer.
        first_slot.route_stale.store(false, Ordering::Release);
        let mut refreshed = factory.connect(&contact).expect("refreshed route creates transport");
        refreshed.connect().expect("refreshed route can dial");
        refreshed.close().expect("close refreshed transport");

        drop(second_incoming);

        runtime.block_on(first.close());
        runtime.block_on(second.close());
    }

    #[test]
    fn factory_created_transport_rejects_route_staled_before_dial() {
        let runtime = Arc::new(Runtime::new().expect("Iroh Tokio runtime"));
        let first = direct_endpoint(&runtime);
        let second = direct_endpoint(&runtime);
        let first_slot = direct_slot(first.clone(), &runtime);
        let incoming =
            IrohIncomingRouter::start_with_slot(Arc::clone(&first_slot), Arc::clone(&runtime));
        let mut factory = IrohTransportFactory::new_with_slot(
            Arc::clone(&first_slot),
            Arc::clone(&runtime),
            incoming,
        );
        let contact = contact_for_iroh_endpoint(&second.addr());
        let mut transport = factory.connect(&contact).expect("fresh route creates transport");
        first_slot.route_stale.store(true, Ordering::Release);

        let error = transport.connect().expect_err("staled queued route must not dial");
        assert!(error.0.contains("route is stale"));

        runtime.block_on(first.close());
        runtime.block_on(second.close());
    }

    #[test]
    fn persisted_contacts_complete_peer_handshake_and_delivery_without_pairing_transport() {
        let runtime = Arc::new(Runtime::new().expect("Iroh Tokio runtime"));
        let first_endpoint = direct_endpoint(&runtime);
        let second_endpoint = direct_endpoint(&runtime);
        let first_slot = direct_slot(first_endpoint.clone(), &runtime);
        let second_slot = direct_slot(second_endpoint.clone(), &runtime);
        let first_incoming =
            IrohIncomingRouter::start_with_slot(Arc::clone(&first_slot), Arc::clone(&runtime));
        let second_incoming =
            IrohIncomingRouter::start_with_slot(Arc::clone(&second_slot), Arc::clone(&runtime));
        let first_factory =
            IrohTransportFactory::new_with_slot(first_slot, Arc::clone(&runtime), first_incoming);
        let second_factory =
            IrohTransportFactory::new_with_slot(second_slot, Arc::clone(&runtime), second_incoming);

        let (first_identity, first_signer) = test_identity(IdentityId::from_u128(101));
        let (second_identity, second_signer) = test_identity(IdentityId::from_u128(202));
        let first_contact_id = ContactId::from_u128(11);
        let second_contact_id = ContactId::from_u128(22);
        let first_capability = OpaqueId::from_u128(1_001);
        let second_capability = OpaqueId::from_u128(2_002);
        let first_contact = Contact::new(
            first_contact_id,
            first_identity.clone(),
            ContactRoute::for_provider_endpoint(
                first_capability,
                iroh_provider_id().as_str(),
                encode_endpoint_addr(&first_endpoint.addr()).expect("encode first route"),
            )
            .expect("first persisted route"),
            Timestamp::UNIX_EPOCH,
        );
        let second_contact = Contact::new(
            second_contact_id,
            second_identity.clone(),
            ContactRoute::for_provider_endpoint(
                second_capability,
                iroh_provider_id().as_str(),
                encode_endpoint_addr(&second_endpoint.addr()).expect("encode second route"),
            )
            .expect("second persisted route"),
            Timestamp::UNIX_EPOCH,
        );
        let first_relationships = TestRelationships::persisted(
            second_contact,
            PeerCredential::new(second_contact_id, first_capability, OpaqueId::from_u128(3_003))
                .expect("first peer credential"),
        );
        let second_relationships = TestRelationships::persisted(
            first_contact,
            PeerCredential::new(first_contact_id, second_capability, OpaqueId::from_u128(4_004))
                .expect("second peer credential"),
        );
        let mut first_link = PeerLink::with_transport_factory(
            Box::new(first_factory),
            first_relationships,
            first_signer,
            first_identity.identity_id().to_opaque(),
        );
        let mut second_link = PeerLink::with_transport_factory(
            Box::new(second_factory),
            second_relationships,
            second_signer,
            second_identity.identity_id().to_opaque(),
        );

        let now = current_timestamp();
        assert!(first_link.ensure_connected(second_contact_id, now).expect("start peer dial"));
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while first_link.connection_state(second_contact_id) != PeerConnectionState::Ready
            || second_link.connection_state(first_contact_id) != PeerConnectionState::Ready
        {
            let _ = second_link
                .maintenance(&[first_contact_id], current_timestamp())
                .expect("maintain recipient handshake");
            let _ = first_link
                .maintenance(&[second_contact_id], current_timestamp())
                .expect("maintain initiator handshake");
            assert!(std::time::Instant::now() < deadline, "persisted-contact handshake timed out");
            std::thread::sleep(Duration::from_millis(10));
        }

        let envelope_id = OpaqueId::from_u128(9_001);
        first_link
            .send_envelope(second_contact_id, envelope_id, 7, b"hello over peer lane".to_vec())
            .expect("send authenticated text");
        let inbound = loop {
            let _ = second_link
                .maintenance(&[first_contact_id], current_timestamp())
                .expect("receive authenticated text");
            if let Some(inbound) = second_link.take_inbound() {
                break inbound;
            }
            assert!(std::time::Instant::now() < deadline, "peer text did not arrive");
            std::thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(inbound.contact_id, first_contact_id);
        assert_eq!(inbound.envelope_id, envelope_id);
        assert_eq!(inbound.ciphertext, b"hello over peer lane");
        second_link
            .send_ack(first_contact_id, envelope_id, AckStatus::Accepted)
            .expect("send transport receipt");

        let receipt = loop {
            let _ = first_link
                .maintenance(&[second_contact_id], current_timestamp())
                .expect("receive transport receipt");
            if let Some(receipt) = first_link
                .poll_envelope_ack(second_contact_id, envelope_id)
                .expect("poll transport receipt")
            {
                break receipt;
            }
            assert!(std::time::Instant::now() < deadline, "transport receipt did not arrive");
            std::thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(receipt, LinkAck::Accepted);

        first_link.shutdown();
        second_link.shutdown();
        runtime.block_on(first_endpoint.close());
        runtime.block_on(second_endpoint.close());
    }

    #[test]
    fn reachability_probe_is_not_started_until_a_runtime_demand_exists() {
        let runtime = Arc::new(Runtime::new().expect("Iroh Tokio runtime"));
        let endpoint = runtime
            .block_on(iroh::Endpoint::builder(presets::N0).alpns(vec![ALPN.to_vec()]).bind())
            .expect("bind local Iroh endpoint");
        let mut lifecycle = IrohLifecycle::new(endpoint, Arc::clone(&runtime));

        assert_eq!(
            lifecycle.runtime_diagnostics().online_probe_attempts,
            Some(0),
            "idle construction must not start Endpoint::online"
        );
        lifecycle.set_reachability_demand(false);
        assert_eq!(lifecycle.runtime_diagnostics().online_probe_attempts, Some(0));
        assert_eq!(
            lifecycle.commissioning().step(CommissioningStage::IncomingReachability),
            CommissioningState::NotRequired,
            "suppressed demand is a completed provider choice, not an endless warm-up"
        );
        lifecycle.shutdown();
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
        let initial_id = decode_endpoint_addr(descriptor.payload()).expect("decode descriptor").id;
        composition.lifecycle.set_dormant(true).expect("deactivate endpoint");
        assert!(composition.pairing_bootstrap_descriptor().is_err());
        composition.lifecycle.set_dormant(false).expect("reactivate endpoint");
        let resumed = composition.pairing_bootstrap_descriptor().expect("resumed descriptor");
        assert_eq!(decode_endpoint_addr(resumed.payload()).expect("decode resumed").id, initial_id);
        assert_eq!(composition.pairing.endpoint_slot().generation(), 2);
        composition.lifecycle.shutdown();
    }

    #[test]
    fn radio_provider_stream_round_trips_without_waiting_for_a_second_accept_bi() {
        let runtime = Arc::new(Runtime::new().expect("Iroh Tokio runtime"));
        let first_endpoint = runtime
            .block_on(
                iroh::Endpoint::builder(presets::N0)
                    .alpns(vec![RADIO_ALPN.to_vec()])
                    .bind_addr("127.0.0.1:0")
                    .expect("first loopback bind")
                    .bind(),
            )
            .expect("first Iroh endpoint");
        let second_endpoint = runtime
            .block_on(
                iroh::Endpoint::builder(presets::N0)
                    .alpns(vec![RADIO_ALPN.to_vec()])
                    .bind_addr("127.0.0.1:0")
                    .expect("second loopback bind")
                    .bind(),
            )
            .expect("second Iroh endpoint");
        let first_router = IrohIncomingRouter::start(first_endpoint.clone(), Arc::clone(&runtime));
        let second_router =
            IrohIncomingRouter::start(second_endpoint.clone(), Arc::clone(&runtime));
        let mut first =
            IrohRadioMediaSystemFactory::new(first_endpoint, Arc::clone(&runtime), first_router);
        let mut second = IrohRadioMediaSystemFactory::new(
            second_endpoint.clone(),
            Arc::clone(&runtime),
            second_router,
        );
        let route = RadioMediaRoute {
            provider: iroh_provider_id().as_str().to_owned(),
            endpoint: encode_endpoint_addr(&second_endpoint.addr()).expect("encode route"),
            local_identity: torca_foundation::OpaqueId::from_u128(1),
            remote_identity: torca_foundation::OpaqueId::from_u128(2),
        };
        let mut outgoing = first.connect(&route, Duration::from_secs(5)).expect("radio connect");
        outgoing
            .configure(Duration::from_millis(50), Duration::from_secs(1))
            .expect("bounded read");
        outgoing.write_all(b"radio-smoke").expect("write radio payload");
        outgoing.flush().expect("flush radio payload");

        let mut incoming = None;
        for _ in 0..50 {
            incoming = second.try_accept().expect("radio accept");
            if incoming.is_some() {
                break;
            }
            runtime.block_on(async { tokio::time::sleep(Duration::from_millis(10)).await });
        }
        let mut incoming = incoming.expect("incoming radio stream");
        incoming
            .configure(Duration::from_millis(50), Duration::from_secs(1))
            .expect("bounded incoming read");
        let mut payload = [0_u8; 11];
        incoming.read_exact(&mut payload).expect("read radio payload");
        assert_eq!(&payload, b"radio-smoke");

        let started = std::time::Instant::now();
        let mut probe = [0_u8; 1];
        let error = incoming.read(&mut probe).expect_err("idle read must be bounded");
        assert_eq!(error.kind(), std::io::ErrorKind::WouldBlock);
        assert!(started.elapsed() < Duration::from_millis(250));

        let _ = outgoing.close_stream();
        let _ = incoming.close_stream();
    }
}
