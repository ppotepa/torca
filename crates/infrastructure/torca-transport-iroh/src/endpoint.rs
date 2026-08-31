use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use iroh::endpoint::Endpoint;
use iroh::{EndpointAddr, SecretKey, TransportAddr, Watcher};
use tokio::runtime::Runtime;
use tokio::time::{Duration, timeout};
use torca_crypto::{ProtectedSecretStore, ProtectedSecretStoreError};
use torca_identity::KeyId;

use super::profile::{
    COMPILED_IROH_DISABLE_DISCOVERY, COMPILED_IROH_DISABLE_RELAY, COMPILED_IROH_LOCAL_ONLY,
    IrohServiceConfig, configured_flag,
};
use super::{ALPN, IrohEndpointProfile, PAIRING_ALPN, RADIO_ALPN, build_provider_runtime};

/// Endpoint creation must never turn provider commissioning into an
/// unbounded warm-up. `Endpoint::bind` is expected to bind local sockets; any
/// relay/discovery work is provider-owned and continues after this returns.
const ENDPOINT_BIND_TIMEOUT: Duration = Duration::from_secs(15);

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
    pub(super) route_generation: Arc<AtomicU64>,
    pub(super) route_stale: Arc<AtomicBool>,
    runtime: Arc<Runtime>,
    secret: SecretKey,
    profile: IrohEndpointProfile,
    local_only: bool,
    relay_urls: Vec<iroh::RelayUrl>,
}

impl IrohEndpointSlot {
    pub(super) fn new(
        endpoint: Endpoint,
        runtime: Arc<Runtime>,
        secret: SecretKey,
        profile: IrohEndpointProfile,
        local_only: bool,
    ) -> ProviderEndpointSlot {
        let relay_urls = pairing_relay_urls(profile);
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
            relay_urls,
        })
    }

    /// Creates a slot for tests which already provide an endpoint. The
    /// endpoint's own secret key is retained so dormancy/reactivation has the
    /// same identity as production composition.
    pub(super) fn static_endpoint(
        endpoint: Endpoint,
        runtime: Arc<Runtime>,
    ) -> ProviderEndpointSlot {
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

    /// Shared route-staleness marker for transports created by this slot.
    /// Keeping the atomic behind the slot lets a network callback invalidate
    /// a transport that is already queued for a dial without exposing any
    /// Iroh-specific state to the generic peer link.
    pub(super) fn route_stale_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.route_stale)
    }

    /// Returns the immutable deployment profile used when binding this slot.
    /// Policy/diagnostics use this instead of guessing battery cost from an
    /// endpoint address or from the provider name.
    pub fn profile(&self) -> IrohEndpointProfile {
        self.profile
    }

    pub(super) fn is_terminated(&self) -> bool {
        self.terminated.load(Ordering::Acquire)
    }

    pub(super) async fn wait_current(&self) -> Option<Endpoint> {
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

    pub(super) fn address(&self) -> Option<EndpointAddr> {
        self.current().map(|endpoint| endpoint.addr())
    }

    /// Returns an address suitable for a new pairing invitation.
    ///
    /// `Endpoint::addr()` is intentionally optimistic and can contain only
    /// local interface addresses until Iroh has selected a home relay. A QR
    /// invitation is immutable, so include the relay candidates configured
    /// for this endpoint immediately instead of blocking on `Endpoint::online`
    /// (which waits for a successful WAN handshake and may pend indefinitely
    /// on a mobile network transition).
    pub fn address_for_pairing(&self) -> Option<EndpointAddr> {
        self.current().map(|endpoint| {
            let mut address = endpoint.addr();
            let relay_statuses = endpoint.home_relay_status().get();
            address = address.with_addrs(
                relay_statuses.into_iter().map(|status| TransportAddr::Relay(status.url().clone())),
            );
            address.with_addrs(self.relay_urls.iter().cloned().map(TransportAddr::Relay))
        })
    }

    pub(super) fn deactivate(&self) {
        let endpoint = self.endpoint.lock().ok().and_then(|mut slot| slot.take());
        if let Some(endpoint) = endpoint {
            self.route_stale.store(true, Ordering::Release);
            self.generation.fetch_add(1, Ordering::AcqRel);
            self.route_generation.fetch_add(1, Ordering::AcqRel);
            self.runtime.block_on(endpoint.close());
        }
        self.notify.notify_waiters();
    }

    pub(super) fn terminate(&self) {
        self.terminated.store(true, Ordering::Release);
        self.deactivate();
    }

    pub(super) fn activate(&self) -> Result<(), IrohIdentityError> {
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

fn pairing_relay_urls(profile: IrohEndpointProfile) -> Vec<iroh::RelayUrl> {
    if !profile.supports_incoming_reachability()
        || configured_flag(COMPILED_IROH_DISABLE_RELAY, "TORCA_IROH_DISABLE_RELAY")
        || configured_flag(COMPILED_IROH_DISABLE_DISCOVERY, "TORCA_IROH_DISABLE_DISCOVERY")
    {
        return Vec::new();
    }

    let service_config = IrohServiceConfig::from_environment(profile).unwrap_or_default();
    if !service_config.relay_urls.is_empty() {
        return service_config.relay_urls;
    }
    if service_config.is_custom() {
        // A custom discovery-only deployment deliberately disables the relay
        // worker in `bind_endpoint_from_secret`.
        return Vec::new();
    }

    iroh::endpoint::default_relay_mode().relay_map().urls()
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

pub(super) fn bind_endpoint_from_secret(
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
    match runtime.block_on(async { timeout(ENDPOINT_BIND_TIMEOUT, builder.bind()).await }) {
        Ok(Ok(endpoint)) => Ok(endpoint),
        Ok(Err(error)) => Err(IrohIdentityError::Bind(error.to_string())),
        Err(_) => Err(IrohIdentityError::Bind(format!(
            "endpoint bind timed out after {} seconds",
            ENDPOINT_BIND_TIMEOUT.as_secs()
        ))),
    }
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
