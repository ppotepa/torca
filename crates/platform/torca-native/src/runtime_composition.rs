use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use torca_client_engine::EngineHandle;
use torca_communication_adapters::{ProductionCommunicationInputs, build_production_communication};
use torca_crypto::{
    ManagedIdentityKeys, ManagedPeerSecrets, OwnedHandshakeSigner, RustCryptoProvider,
    RustPairingCrypto,
};
use torca_pairing_coordinator::{
    PairingApprovalPort, PairingCoordinator, PairingPeerSecretStore, PairingRuntime,
};
use torca_pairing_driver::RuntimePairingDriver;
use torca_platform::{PlatformServices, RelayEndpoint, SecretNamespace};
use torca_rendezvous_client::{RendezvousClient, TorRelayTransport};
use torca_runtime::{
    OwnedTorDriver, RelayProbe, RuntimeDriverError, RuntimeHandle, RuntimeOwner, SharedTorEndpoint,
};
use torca_storage_sqlite::SqlCipherRelationshipAdmin;
use torca_tor::{PeerListener, TorBootstrapObserver, TorService};

use crate::composition::{NativeCompositionError, load_or_create_database_key};

const NETWORK_TIMEOUT: Duration = Duration::from_secs(30);
const TOR_STARTUP_TIMEOUT: Duration = Duration::from_secs(15 * 60);
const COMPILED_RELAY_ENDPOINT: &str = match option_env!("TORCA_RELAY_ENDPOINT") {
    Some(value) => value,
    None => "",
};

struct TorRelayProbe {
    tor: Arc<TorService>,
    host: String,
    port: u16,
}

impl RelayProbe for TorRelayProbe {
    fn probe(&self) -> Result<(), RuntimeDriverError> {
        let stream = self
            .tor
            .connect_onion_with_timeout(&self.host, self.port, Duration::from_secs(15))
            .map_err(|_| RuntimeDriverError::Communication)?;
        stream.peer_addr().map_err(|_| RuntimeDriverError::Communication)?;
        Ok(())
    }
}
pub(crate) fn spawn_production_runtime(
    engine: EngineHandle,
    bootstrap_observer: TorBootstrapObserver,
) -> Result<(RuntimeHandle, RuntimeOwner), NativeCompositionError> {
    #[cfg(windows)]
    {
        use crate::app_paths::windows_app_root;
        use torca_platform_windows::WindowsPlatformServices;
        let root = windows_app_root()?;
        let relay = parse_relay_endpoint(COMPILED_RELAY_ENDPOINT)?;
        let platform = WindowsPlatformServices::new(
            root.join("data"),
            root.join("cache"),
            root.join("logs"),
            RelayEndpoint { host: relay.0, port: relay.1 },
        );
        return spawn_runtime_for(&platform, engine, bootstrap_observer);
    }
    #[cfg(target_os = "android")]
    {
        use crate::composition::android::{database_path, log_root_path};
        use torca_platform_android::AndroidPlatformServices;
        let database = database_path()
            .map_err(|_| NativeCompositionError::new("resolve Android database path failed"))?;
        let data = database.parent().map_or_else(|| database.clone(), std::path::Path::to_path_buf);
        let relay = parse_relay_endpoint(COMPILED_RELAY_ENDPOINT)?;
        let platform = AndroidPlatformServices::new(
            data.clone(),
            data.join("cache"),
            log_root_path().unwrap_or_else(|_| data.join("logs")),
            RelayEndpoint { host: relay.0, port: relay.1 },
        )
        .with_secret_store_factory(|namespace| {
            let name = match namespace {
                torca_platform::SecretNamespace::Identity => "identity",
                torca_platform::SecretNamespace::Storage => "database",
                torca_platform::SecretNamespace::Runtime => "peer",
            };
            Box::new(crate::composition::android::AndroidProtectedSecretStore::new(name))
        });
        return spawn_runtime_for(&platform, engine, bootstrap_observer);
    }
    #[cfg(not(any(windows, target_os = "android")))]
    {
        let _ = engine;
        let _ = bootstrap_observer;
        Err(NativeCompositionError::new(
            "production network runtime is not implemented for this platform",
        ))
    }
}

fn spawn_runtime_for<P: PlatformServices>(
    platform: &P,
    engine: EngineHandle,
    bootstrap_observer: TorBootstrapObserver,
) -> Result<(RuntimeHandle, RuntimeOwner), NativeCompositionError> {
    let paths = platform.app_paths();
    let database_path = paths.data.join("torca.db");
    let mut database_store = platform.open_secret_store(SecretNamespace::Storage);
    let database_key = load_or_create_database_key(
        database_store.as_mut(),
        crate::composition::DATABASE_KEY_HANDLE,
        RustCryptoProvider,
    )?;
    let identity = engine_identity(&engine)?;
    let key_id = identity.public().key().key_id();
    let identity_id = identity.public().identity_id().to_opaque();

    let listener = bind_peer_listener()?;
    let endpoint = SharedTorEndpoint::default();
    let tor = OwnedTorDriver::bootstrap_observed(
        paths.data.join("tor"),
        listener.local_addr(),
        endpoint.clone(),
        TOR_STARTUP_TIMEOUT,
        current_timestamp()?,
        Some(bootstrap_observer),
    )
    .map_err(|(error, diagnostic)| {
        // Preserve the redacted, actionable Arti diagnostic.  Previously the
        // native boundary discarded it and Windows only reported the opaque
        // `start Tor runtime failed`, making a failed bootstrap impossible to
        // distinguish from a disconnected worker.
        NativeCompositionError::new(format!(
            "start Tor runtime failed: {error}; diagnostic: {diagnostic}"
        ))
    })?;
    let tor_client = tor
        .client_handle()
        .ok_or_else(|| NativeCompositionError::new("Arti Tor client is unavailable"))?;
    let signer = OwnedHandshakeSigner::new(
        ManagedIdentityKeys::new(
            RustCryptoProvider,
            platform.open_secret_store(SecretNamespace::Identity),
        ),
        key_id,
    );
    let communication = build_production_communication(
        engine.clone(),
        &database_path,
        &database_key,
        &paths.cache.join("attachments"),
        &paths.data.join("attachments").join("staging"),
        ProductionCommunicationInputs {
            signer,
            peer_secret_store: platform.open_secret_store(SecretNamespace::Runtime),
            attachment_secret_store: platform.open_secret_store(SecretNamespace::Runtime),
            export_secret_store: platform.open_secret_store(SecretNamespace::Runtime),
            relationship_secret_store: platform.open_secret_store(SecretNamespace::Runtime),
            listener,
            tor_client: tor_client.clone(),
            local_identity_id: identity_id,
        },
    )
    .map_err(|_| NativeCompositionError::new("compose communication runtime failed"))?;
    let metadata = SqlCipherRelationshipAdmin::open(&database_path, &database_key)
        .map_err(|_| NativeCompositionError::new("open pairing contact metadata store failed"))?;
    let relay = platform
        .relay_endpoint()
        .map_err(NativeCompositionError::new)
        .map(|endpoint| (endpoint.host, endpoint.port))?;
    // Relay health is deliberately asynchronous. A relay outage must put the
    // relay step into degraded state without preventing Tor/onion/profile from
    // becoming available.
    let relay_probe: Arc<dyn RelayProbe> =
        Arc::new(TorRelayProbe { tor: tor_client.clone(), host: relay.0.clone(), port: relay.1 });
    let pairing = build_pairing_driver(
        engine.clone(),
        endpoint,
        tor_client,
        relay,
        ManagedIdentityKeys::new(
            RustCryptoProvider,
            platform.open_secret_store(SecretNamespace::Identity),
        ),
        ManagedPeerSecrets::new(
            RustCryptoProvider,
            platform.open_secret_store(SecretNamespace::Runtime),
        ),
    )
    .with_contact_metadata(metadata);
    Ok(RuntimeOwner::spawn_with_relay_probe(engine, pairing, communication, tor, Some(relay_probe)))
}

fn bind_peer_listener() -> Result<PeerListener, NativeCompositionError> {
    PeerListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
        .map_err(|_| NativeCompositionError::new("bind local peer listener failed"))
}

fn build_pairing_driver<A, S>(
    engine: EngineHandle,
    endpoint: SharedTorEndpoint,
    tor_client: Arc<TorService>,
    relay: (String, u16),
    approval: A,
    peer_secrets: S,
) -> RuntimePairingDriver<RendezvousClient<TorRelayTransport>, RustPairingCrypto, A, S>
where
    A: PairingApprovalPort + Send + 'static,
    S: PairingPeerSecretStore + Send + 'static,
{
    let transport = TorRelayTransport::new(tor_client, relay.0, relay.1);
    let rendezvous = RendezvousClient::new(transport, NETWORK_TIMEOUT);
    let coordinator = PairingCoordinator::new(rendezvous, RustPairingCrypto::new());
    let runtime = PairingRuntime::new(coordinator, engine.clone(), approval, peer_secrets);
    RuntimePairingDriver::new(runtime, engine, endpoint)
}

fn engine_identity(
    engine: &EngineHandle,
) -> Result<torca_identity::Identity, NativeCompositionError> {
    engine
        .overview_snapshot()
        .map_err(|_| NativeCompositionError::new("load local identity failed"))?
        .identity
        .ok_or_else(|| NativeCompositionError::new("local identity is not initialized"))
}

fn parse_relay_endpoint(value: &str) -> Result<(String, u16), NativeCompositionError> {
    let (host, port) = value
        .rsplit_once(':')
        .ok_or_else(|| NativeCompositionError::new("relay endpoint must be host.onion:port"))?;
    let label = host.strip_suffix(".onion").ok_or_else(|| {
        NativeCompositionError::new("relay endpoint must use a v3 onion hostname")
    })?;
    if label.len() != 56 || !label.bytes().all(|byte| matches!(byte, b'a'..=b'z' | b'2'..=b'7')) {
        return Err(NativeCompositionError::new(
            "relay endpoint contains an invalid v3 onion hostname",
        ));
    }
    let port = port
        .parse::<u16>()
        .map_err(|_| NativeCompositionError::new("relay endpoint contains an invalid port"))?;
    if port == 0 {
        return Err(NativeCompositionError::new("relay endpoint port must be non-zero"));
    }
    Ok((host.to_owned(), port))
}

fn current_timestamp() -> Result<torca_foundation::Timestamp, NativeCompositionError> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| NativeCompositionError::new("system clock is before Unix epoch"))?;
    let millis = i64::try_from(duration.as_millis())
        .map_err(|_| NativeCompositionError::new("system timestamp is out of range"))?;
    torca_foundation::Timestamp::from_unix_millis(millis)
        .map_err(|_| NativeCompositionError::new("system timestamp is invalid"))
}
