//! Iroh/QUIC implementation of the provider-neutral Torca transport.
//!
//! The adapter receives a configured Iroh endpoint and remote address from
//! the provider composition layer. Signalling and endpoint identity exchange
//! stay outside this crate; peer protocol and E2EE remain unchanged.

use std::collections::VecDeque;
use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use iroh::endpoint::{Connection, Endpoint, RecvStream, SendStream};
use iroh::{EndpointAddr, SecretKey};
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
    PeerTransport, PeerTransportError, PeerTransportFactory, ProviderCommissioning, ProviderRoute,
    ProviderRouteState, ProviderRuntimeDiagnostics, ProviderTransport, TransportCapabilities,
    TransportFactoryError, TransportKind, TransportPath,
};

mod pairing;
pub use pairing::IrohPairingService;

const ALPN: &[u8] = b"torca/peer/1";
const NETWORK_CHANGE_TIMEOUT: Duration = Duration::from_secs(30);
/// ALPN used only for the short-lived provider-owned pairing service.
pub const PAIRING_ALPN: &[u8] = b"torca/pairing/1";
/// Reserved provider-owned ALPN for optional Radio media streams.
pub const RADIO_ALPN: &[u8] = b"torca/radio/1";

/// Deployment-time Iroh endpoint policy. This is intentionally provider-local:
/// the generic runtime asks for availability/dormancy, while the endpoint
/// builder decides which discovery and relay services are appropriate for the
/// selected deployment profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IrohEndpointProfile {
    /// Publicly reachable baseline using the N0 relay and address lookup.
    AlwaysReachable,
    /// Keep direct addressing but do not publish/resolve through address
    /// lookup or maintain a relay connection. A peer must receive a complete
    /// endpoint address out of band.
    DirectOnly,
    /// No relay and no address lookup. Intended for local tests or explicit
    /// battery experiments; it cannot provide background reachability.
    LocalOnly,
}

impl IrohEndpointProfile {
    /// Parses the provider-local profile used by deployment and diagnostics.
    /// Unknown values deliberately fall back to the interoperable profile.
    pub fn from_wire(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "direct" | "direct-only" => Self::DirectOnly,
            "local" | "local-only" => Self::LocalOnly,
            _ => Self::AlwaysReachable,
        }
    }

    pub const fn wire_value(self) -> &'static str {
        match self {
            Self::AlwaysReachable => "always",
            Self::DirectOnly => "direct",
            Self::LocalOnly => "local",
        }
    }

    /// Relative energy class for scheduling and diagnostics. This is not a
    /// physical battery measurement: direct/local avoid discovery and relay
    /// maintenance, while always keeps those services available.
    pub const fn energy_class(self) -> EnergyClass {
        match self {
            Self::DirectOnly | Self::LocalOnly => EnergyClass::Low,
            Self::AlwaysReachable => EnergyClass::Medium,
        }
    }

    /// Whether this profile has an incoming-reachability service to monitor.
    /// Direct/local profiles intentionally have no relay or address lookup;
    /// treating their permanently-empty `home_relay_status` as a pending
    /// online probe would create an endless retry task and false warm-up UI.
    pub const fn supports_incoming_reachability(self) -> bool {
        matches!(self, Self::AlwaysReachable)
    }

    fn from_environment() -> Self {
        // Deployment passes the profile to Cargo, so packaged clients carry
        // an immutable profile that matches their artifact manifest. Only an
        // unconfigured development binary may use the process environment at
        // runtime; otherwise a stale host variable could silently change the
        // battery/reachability policy after artifact verification.
        option_env!("TORCA_IROH_PROFILE")
            .map(str::to_owned)
            .or_else(|| std::env::var("TORCA_IROH_PROFILE").ok())
            .map(|value| Self::from_wire(&value))
            .unwrap_or(Self::AlwaysReachable)
    }

    fn apply(self, builder: iroh::endpoint::Builder) -> iroh::endpoint::Builder {
        match self {
            Self::AlwaysReachable => builder,
            // Direct-only is an explicit low-power/offline-discovery profile:
            // disable both the address-lookup publisher and the relay map so
            // Iroh does not keep a home-relay task alive in the background.
            // The endpoint address must then be exchanged out of band.
            Self::DirectOnly => {
                builder.clear_address_lookup().relay_mode(iroh::RelayMode::Disabled)
            }
            Self::LocalOnly => builder.clear_address_lookup().relay_mode(iroh::RelayMode::Disabled),
        }
    }
}

/// Provider-owned service configuration for the relay-backed Iroh profile.
///
/// The direct and local profiles deliberately ignore these services: adding a
/// relay or public lookup endpoint to those profiles would silently defeat
/// their low-power/offline contract. Values are read once while the endpoint
/// is created, never from the application or FFI boundary.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct IrohServiceConfig {
    relay_urls: Vec<iroh::RelayUrl>,
    pkarr_url: Option<url::Url>,
}

const COMPILED_IROH_RELAY_URLS: Option<&str> = option_env!("TORCA_IROH_RELAY_URLS");
const COMPILED_IROH_PKARR_URL: Option<&str> = option_env!("TORCA_IROH_PKARR_URL");
const COMPILED_IROH_DISABLE_RELAY: Option<&str> = option_env!("TORCA_IROH_DISABLE_RELAY");
const COMPILED_IROH_DISABLE_DISCOVERY: Option<&str> = option_env!("TORCA_IROH_DISABLE_DISCOVERY");
const COMPILED_IROH_LOCAL_ONLY: Option<&str> = option_env!("TORCA_IROH_LOCAL_ONLY");
const COMPILED_IROH_RUNTIME_THREADS: Option<&str> = option_env!("TORCA_IROH_RUNTIME_THREADS");

fn configured_flag(compiled: Option<&str>, key: &str) -> bool {
    compiled
        .map(|value| {
            matches!(value.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on")
        })
        .or_else(|| {
            std::env::var(key).ok().map(|value| {
                matches!(value.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on")
            })
        })
        .unwrap_or(false)
}

impl IrohServiceConfig {
    fn from_environment(profile: IrohEndpointProfile) -> Result<Self, IrohIdentityError> {
        if !profile.supports_incoming_reachability() {
            return Ok(Self::default());
        }

        let relay_value = COMPILED_IROH_RELAY_URLS
            .map(str::to_owned)
            .or_else(|| std::env::var("TORCA_IROH_RELAY_URLS").ok());
        let pkarr_value = COMPILED_IROH_PKARR_URL
            .map(str::to_owned)
            .or_else(|| std::env::var("TORCA_IROH_PKARR_URL").ok());
        Self::from_values(
            profile,
            relay_value.as_deref(),
            pkarr_value.as_deref().filter(|value| !value.trim().is_empty()),
        )
    }

    fn from_values(
        profile: IrohEndpointProfile,
        relay_value: Option<&str>,
        pkarr_value: Option<&str>,
    ) -> Result<Self, IrohIdentityError> {
        if !profile.supports_incoming_reachability() {
            return Ok(Self::default());
        }

        let relay_urls = relay_value
            .into_iter()
            .flat_map(|value| {
                value.split(',').map(str::trim).map(str::to_owned).collect::<Vec<_>>()
            })
            .filter(|value| !value.is_empty())
            .map(|value| {
                value.parse::<iroh::RelayUrl>().map_err(|error| {
                    IrohIdentityError::Bind(format!(
                        "invalid TORCA_IROH_RELAY_URLS entry '{value}': {error}"
                    ))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let pkarr_url = pkarr_value
            .map(str::to_owned)
            .map(|value| {
                value.parse::<url::Url>().map_err(|error| {
                    IrohIdentityError::Bind(format!(
                        "invalid TORCA_IROH_PKARR_URL '{value}': {error}"
                    ))
                })
            })
            .transpose()?;
        Ok(Self { relay_urls, pkarr_url })
    }

    fn is_custom(&self) -> bool {
        !self.relay_urls.is_empty() || self.pkarr_url.is_some()
    }
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

/// Provider-owned endpoint handle used only by the native composition layer
/// to lazily encode current route/bootstrap metadata. It never crosses the
/// application or FFI boundary.
pub type ProviderEndpoint = Endpoint;

/// Shared, replaceable endpoint owned by the Iroh provider. Factories only
/// borrow a clone for the duration of a dial; they never own the lifecycle.
/// This allows the runtime to close an idle endpoint and recreate it later
/// without changing the stable endpoint identity.
pub type ProviderEndpointSlot = Arc<IrohEndpointSlot>;

pub struct IrohEndpointSlot {
    endpoint: Mutex<Option<Endpoint>>,
    notify: Arc<tokio::sync::Notify>,
    terminated: AtomicBool,
    generation: AtomicU64,
    route_generation: Arc<AtomicU64>,
    route_stale: Arc<AtomicBool>,
    runtime: Arc<Runtime>,
    secret: SecretKey,
    profile: IrohEndpointProfile,
    local_only: bool,
}

impl IrohEndpointSlot {
    fn new(
        endpoint: Endpoint,
        runtime: Arc<Runtime>,
        secret: SecretKey,
        profile: IrohEndpointProfile,
        local_only: bool,
    ) -> ProviderEndpointSlot {
        Arc::new(Self {
            endpoint: Mutex::new(Some(endpoint)),
            notify: Arc::new(tokio::sync::Notify::new()),
            terminated: AtomicBool::new(false),
            generation: AtomicU64::new(0),
            route_generation: Arc::new(AtomicU64::new(0)),
            route_stale: Arc::new(AtomicBool::new(false)),
            runtime,
            secret,
            profile,
            local_only,
        })
    }

    /// Creates a slot for tests which already provide an endpoint. The
    /// endpoint's own secret key is retained so dormancy/reactivation has the
    /// same identity as production composition.
    fn static_endpoint(endpoint: Endpoint, runtime: Arc<Runtime>) -> ProviderEndpointSlot {
        let secret = endpoint.secret_key().clone();
        Self::new(endpoint, runtime, secret, IrohEndpointProfile::AlwaysReachable, false)
    }

    pub fn current(&self) -> Option<Endpoint> {
        self.endpoint.lock().ok().and_then(|slot| slot.clone())
    }

    /// Monotonically increasing endpoint generation. A generation changes
    /// whenever the provider closes or recreates its endpoint; consumers can
    /// use it to reject work that captured a stale endpoint before dormancy
    /// or a provider restart.
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Returns the generation of the currently advertised endpoint address.
    /// Iroh may change that address without replacing the endpoint (for
    /// example after Wi-Fi/LTE migration or relay selection), so callers must
    /// not use the endpoint generation as a route freshness signal. A network
    /// transition increments this generation immediately and marks the route
    /// stale; callers must check `route_is_fresh` before advertising it.
    pub fn route_generation(&self) -> u64 {
        self.route_generation.load(Ordering::Acquire)
    }

    /// Returns whether the current endpoint address is safe to advertise.
    /// Network migration invalidates the old address before Iroh has finished
    /// selecting the replacement route.
    pub fn route_is_fresh(&self) -> bool {
        !self.route_stale.load(Ordering::Acquire)
    }

    /// Shared route-freshness marker for transports created by this slot.
    /// Keeping the atomic behind the slot lets a network callback invalidate
    /// a transport that is already queued for a dial without exposing any
    /// Iroh-specific state to the generic peer link.
    fn route_fresh_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.route_stale)
    }

    /// Returns the immutable deployment profile used when binding this slot.
    /// Policy/diagnostics use this instead of guessing battery cost from an
    /// endpoint address or from the provider name.
    pub fn profile(&self) -> IrohEndpointProfile {
        self.profile
    }

    fn is_terminated(&self) -> bool {
        self.terminated.load(Ordering::Acquire)
    }

    async fn wait_current(&self) -> Option<Endpoint> {
        loop {
            if let Some(endpoint) = self.current() {
                return Some(endpoint);
            }
            if self.terminated.load(Ordering::Acquire) {
                return None;
            }
            self.notify.notified().await;
        }
    }

    fn address(&self) -> Option<EndpointAddr> {
        self.current().map(|endpoint| endpoint.addr())
    }

    fn deactivate(&self) {
        let endpoint = self.endpoint.lock().ok().and_then(|mut slot| slot.take());
        if let Some(endpoint) = endpoint {
            self.route_stale.store(true, Ordering::Release);
            self.generation.fetch_add(1, Ordering::AcqRel);
            self.route_generation.fetch_add(1, Ordering::AcqRel);
            self.runtime.block_on(endpoint.close());
        }
        self.notify.notify_waiters();
    }

    fn terminate(&self) {
        self.terminated.store(true, Ordering::Release);
        self.deactivate();
    }

    fn activate(&self) -> Result<(), IrohIdentityError> {
        if self.terminated.load(Ordering::Acquire) {
            return Err(IrohIdentityError::Bind("endpoint slot is terminated".to_owned()));
        }
        if self.current().is_some() {
            return Ok(());
        }
        let endpoint = bind_endpoint_from_secret(
            &self.runtime,
            self.secret.clone(),
            self.profile,
            self.local_only,
        )?;
        if let Ok(mut slot) = self.endpoint.lock() {
            *slot = Some(endpoint);
            self.route_stale.store(false, Ordering::Release);
            self.generation.fetch_add(1, Ordering::AcqRel);
            self.route_generation.fetch_add(1, Ordering::AcqRel);
        }
        self.notify.notify_waiters();
        Ok(())
    }
}

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
    let profile = IrohEndpointProfile::from_environment();
    let local_only = configured_flag(COMPILED_IROH_LOCAL_ONLY, "TORCA_IROH_LOCAL_ONLY")
        || matches!(profile, IrohEndpointProfile::LocalOnly);
    bind_endpoint_from_secret(runtime, secret, profile, local_only)
}

fn bind_endpoint_from_secret(
    runtime: &Runtime,
    secret: SecretKey,
    profile: IrohEndpointProfile,
    local_only: bool,
) -> Result<Endpoint, IrohIdentityError> {
    let service_config = IrohServiceConfig::from_environment(profile)?;
    // Packaged artifacts use the values embedded by this crate's build script;
    // an unconfigured development build may still use process environment.
    // This prevents a host shell from silently changing an already verified
    // artifact's routing/energy policy after deployment.
    let disable_relay = configured_flag(COMPILED_IROH_DISABLE_RELAY, "TORCA_IROH_DISABLE_RELAY");
    let disable_discovery =
        configured_flag(COMPILED_IROH_DISABLE_DISCOVERY, "TORCA_IROH_DISABLE_DISCOVERY");
    // Minimal is important for custom deployments: starting from N0 would
    // silently retain public N0 lookup services when only one custom service
    // was configured. Direct/local also start from Minimal so an offline
    // profile never constructs unused discovery/relay workers.
    let use_minimal = !matches!(profile, IrohEndpointProfile::AlwaysReachable)
        || service_config.is_custom()
        || disable_relay
        || disable_discovery;
    let base = if use_minimal {
        Endpoint::builder(iroh::endpoint::presets::Minimal)
    } else {
        Endpoint::builder(iroh::endpoint::presets::N0)
    };
    let mut builder = profile.apply(base).secret_key(secret).alpns(vec![
        ALPN.to_vec(),
        PAIRING_ALPN.to_vec(),
        RADIO_ALPN.to_vec(),
    ]);

    if profile.supports_incoming_reachability() && !disable_relay {
        if !service_config.relay_urls.is_empty() {
            builder = builder.relay_mode(iroh::RelayMode::custom(service_config.relay_urls));
        } else if use_minimal {
            // A custom discovery-only deployment still needs an explicit
            // relay decision. Keeping this disabled avoids an accidental
            // fallback to public N0 relays.
            builder = builder.relay_mode(iroh::RelayMode::Disabled);
        }
    } else if disable_relay {
        builder = builder.relay_mode(iroh::RelayMode::Disabled);
    }

    if profile.supports_incoming_reachability()
        && !disable_discovery
        && let Some(pkarr_url) = service_config.pkarr_url
    {
        builder = builder
            .address_lookup(iroh::address_lookup::PkarrPublisher::builder(pkarr_url.clone()))
            .address_lookup(iroh::address_lookup::PkarrResolver::builder(pkarr_url));
    } else if disable_discovery || (profile.supports_incoming_reachability() && use_minimal) {
        builder = builder.clear_address_lookup();
    }
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
        if route.provider != TransportKind::Iroh.wire_value() {
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

    fn transport_error(kind: RelayTransportFailureKind, sent: bool) -> RelayTransportError {
        RelayTransportError { kind, request_was_sent: sent }
    }
}

impl PairingServiceTransport for IrohPairingServiceTransport {
    fn invalidate(&mut self) {
        if let Some(connection) = self.connection.take() {
            connection.close(0_u32.into(), b"pairing reset");
        }
    }

    fn reconnect(&mut self) -> Result<(), RelayTransportError> {
        self.invalidate();
        // The pairing descriptor is an opaque snapshot of the provider route.
        // Never dial it while this endpoint is between network generations;
        // report a retryable unavailable result to the rendezvous client.
        if !self.endpoint.route_is_fresh() {
            return Err(Self::transport_error(RelayTransportFailureKind::Unavailable, false));
        }
        let endpoint = self
            .endpoint
            .current()
            .ok_or_else(|| Self::transport_error(RelayTransportFailureKind::Unavailable, false))?;
        let connection = self
            .runtime
            .block_on(endpoint.connect(self.remote.clone(), PAIRING_ALPN))
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
        let profile = IrohEndpointProfile::from_environment();
        let local_only = configured_flag(COMPILED_IROH_LOCAL_ONLY, "TORCA_IROH_LOCAL_ONLY")
            || matches!(profile, IrohEndpointProfile::LocalOnly);
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
/// infrastructure is available for incoming reachability.  This is purposely
/// not translated into Tor/onion vocabulary.
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
        let probe_cancel = Arc::clone(probe_cancel);
        let route_generation = Arc::clone(route_generation);
        let generation = Arc::clone(generation);
        let wake = Arc::clone(wake);
        let probe_attempts = Arc::clone(probe_attempts);
        let probe_failures = Arc::clone(probe_failures);
        runtime.spawn(async move {
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
            // A network event and a demand transition can arrive together.
            // Only one report may own the endpoint at a time; the later wake
            // observes the result through the atomics and does not duplicate
            // the cellular/relay work.
            if probe_in_flight.swap(true, Ordering::AcqRel) {
                return;
            }
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
    fn provider(&self) -> TransportKind {
        TransportKind::Iroh
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
    route_fresh: Option<Arc<AtomicBool>>,
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
        route_fresh: Option<Arc<AtomicBool>>,
    ) -> Self {
        Self {
            endpoint,
            remote: Some(remote),
            runtime,
            route_fresh,
            profile,
            connection: None,
            sender: None,
            inbound: Arc::new((Mutex::new(VecDeque::new()), Condvar::new())),
            wake: Arc::new(Mutex::new(None)),
            stream_alive: Arc::new(AtomicBool::new(false)),
            connected: false,
            path: TransportPath::Unknown,
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
            route_fresh: None,
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
            route_fresh: None,
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
                TransportPath::IrohRelay
            } else if path.is_ip() {
                TransportPath::IrohDirect
            } else {
                TransportPath::Unknown
            }
        })
        .unwrap_or_else(|| {
            // A path may not be published in the first snapshot immediately
            // after a handshake. Keep the profile as a conservative fallback
            // until the next reconnect/observation.
            if profile_supports_relay(profile) {
                TransportPath::IrohRelay
            } else {
                TransportPath::IrohDirect
            }
        })
}

impl PeerTransport for IrohTransport {
    fn connect(&mut self) -> Result<(), PeerTransportError> {
        if self.connected && self.stream_alive.load(Ordering::Acquire) {
            return Ok(());
        }
        if self.route_fresh.as_ref().is_some_and(|fresh| !fresh.load(Ordering::Acquire)) {
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
        if self.route_fresh.as_ref().is_some_and(|fresh| !fresh.load(Ordering::Acquire)) {
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
    fn kind(&self) -> TransportKind {
        TransportKind::Iroh
    }

    fn path(&self) -> TransportPath {
        // Iroh may migrate an established QUIC connection from a relay to a
        // direct path (or back) without rebuilding the stream. Observe the
        // selected path on every capability read so diagnostics and policy do
        // not report the deployment profile as the actual route.
        self.connection
            .as_ref()
            .map(|connection| selected_path(connection, self.profile))
            .unwrap_or(self.path)
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
    fn kind(&self) -> TransportKind {
        TransportKind::Iroh
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
            .provider_endpoint(TransportKind::Iroh.wire_value())
            .ok_or(TransportFactoryError::ContactNotFound)?;
        let local_endpoint = self.endpoint.current().ok_or(TransportFactoryError::Listener)?;
        let remote = decode_endpoint_addr(endpoint).map_err(|_| TransportFactoryError::Protocol)?;
        let mut transport = IrohTransport::new_with_profile(
            local_endpoint,
            remote,
            Arc::clone(&self.runtime),
            self.endpoint.profile(),
            Some(self.endpoint.route_fresh_flag()),
        );
        if let Some(waker) = self.wake.lock().ok().and_then(|slot| slot.clone()) {
            transport.set_waker(waker);
        }
        Ok(Box::new(transport))
    }

    fn local_route(&self) -> Option<ProviderRoute> {
        if !self.endpoint.route_is_fresh() {
            return None;
        }
        let address = self.endpoint.address()?;
        let endpoint = encode_endpoint_addr(&address).ok()?;
        ProviderRoute::new(TransportKind::Iroh, self.endpoint.route_generation(), endpoint)
    }

    fn local_route_is_fresh(&self) -> bool {
        self.endpoint.route_is_fresh()
    }

    fn local_route_state(&self) -> ProviderRouteState {
        if !self.endpoint.route_is_fresh() {
            ProviderRouteState::Stale
        } else if self.endpoint.address().is_some() {
            ProviderRouteState::Fresh
        } else {
            ProviderRouteState::Unavailable
        }
    }

    fn preserves_sessions_on_network_change(&self) -> bool {
        // QUIC/Iroh can migrate a live connection to the new network path;
        // closing every session here would create a reconnect storm and lose
        // the only authenticated channel on which to advertise the new route.
        true
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
    use std::collections::VecDeque;
    use std::io::{Read, Write};
    use std::sync::atomic::Ordering;
    use std::sync::{Arc, Condvar, Mutex};
    use std::time::Duration;

    use iroh::endpoint::presets;
    use tokio::runtime::Runtime;
    use torca_crypto::{InMemoryProtectedSecretStore, ProtectedSecretStore};
    use torca_foundation::Timestamp;
    use torca_radio_adapters::{RadioMediaConnector, RadioMediaRoute};
    use torca_runtime::{CommunicationLifecycle, CommunicationState, IncomingReachabilityState};
    use torca_transport_api::{
        CommissioningStage, CommissioningState, EnergyClass, PeerTransport, ProviderRouteState,
        TransportKind,
    };

    use super::{
        ALPN, IROH_ENDPOINT_SECRET_HANDLE, IrohComposition, IrohEndpointProfile, IrohEndpointSlot,
        IrohIncomingRouter, IrohLifecycle, IrohRadioMediaSystemFactory, IrohServiceConfig,
        IrohTransport, RADIO_ALPN, bind_endpoint, decode_endpoint_addr, encode_endpoint_addr,
        load_or_create_endpoint_secret,
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

        assert_eq!(lifecycle.provider(), TransportKind::Iroh);
        assert_eq!(lifecycle.state(), CommunicationState::Ready);
        assert_eq!(lifecycle.incoming_reachability_state(), IncomingReachabilityState::Unknown);
        assert_eq!(lifecycle.runtime_diagnostics().route_state, Some(ProviderRouteState::Fresh));
        assert_eq!(lifecycle.runtime_diagnostics().energy_class, Some(EnergyClass::Medium));
        assert_eq!(lifecycle.runtime_diagnostics().reachability_demanded, Some(false));
        assert_eq!(lifecycle.runtime_diagnostics().online_probe_attempts, Some(0));
        let commissioning = lifecycle.commissioning();
        assert_eq!(commissioning.provider, TransportKind::Iroh);
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
        let route_fresh = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mut transport = IrohTransport::new_with_profile(
            endpoint.clone(),
            endpoint.addr(),
            Arc::clone(&runtime),
            IrohEndpointProfile::DirectOnly,
            Some(route_fresh),
        );
        let error = transport.connect().expect_err("stale route must not dial");
        assert!(error.0.contains("route is stale"));
        runtime.block_on(endpoint.close());
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
            provider: TransportKind::Iroh.wire_value().to_owned(),
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
