use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use torca_client_engine::EngineHandle;
use torca_communication_adapters::{
    ProductionCommunicationInputs, ReadReceiptPolicy, build_production_communication,
};
use torca_connectivity::ConnectivityObserver;
use torca_crypto::{
    ManagedIdentityKeys, ManagedPeerSecrets, OwnedHandshakeSigner, RustCryptoProvider,
    RustPairingCrypto,
};
use torca_foundation::ErrorCode;
use torca_pairing_coordinator::{
    PairingApprovalPort, PairingCoordinator, PairingPeerSecretStore, PairingRuntime,
};
use torca_pairing_driver::{PairingWorkerDriver, RuntimePairingDriver};
use torca_platform::{PlatformServices, SecretNamespace};
use torca_rendezvous_client::{RendezvousClient, SharedTorRelayTransport};
use torca_runtime::{RelayProbe, RelayServiceInfo, RuntimeHandle, RuntimeOwner};
use torca_storage_sqlite::SqlCipherRelationshipAdmin;
use torca_tor::{OwnedTorDriver, SharedTorEndpoint};
use torca_tor::{PeerListener, TorBootstrapObserver};

use crate::composition::{NativeCompositionError, load_or_create_database_key};

// Keep interactive requests below the application command deadline. Long-lived
// recovery is handled by the pairing supervisor instead of one blocking call.
const NETWORK_TIMEOUT: Duration = Duration::from_secs(8);
const TOR_STARTUP_TIMEOUT: Duration = Duration::from_secs(15 * 60);
const COMPILED_RELAY_ENDPOINT: &str = match option_env!("TORCA_RELAY_ENDPOINT") {
    Some(value) => value,
    None => "",
};

struct TorRelayProbe {
    transport: SharedTorRelayTransport,
    info: Mutex<Option<RelayServiceInfo>>,
}

impl RelayProbe for TorRelayProbe {
    fn probe(&self) -> Result<(), ErrorCode> {
        self.transport
            .try_relay_info(Duration::from_secs(2))
            .map(|info| {
                if let Ok(mut current) = self.info.lock() {
                    *current = Some(RelayServiceInfo {
                        product_version: info.product_version,
                        build_id: info.build_id,
                        source_commit: info.source_commit,
                        protocol_version: info.protocol_version,
                    });
                }
            })
            .map_err(|error| {
                ErrorCode::new(match error.kind {
                    torca_rendezvous_client::RelayTransportFailureKind::Busy => {
                        "relay.connection_busy"
                    }
                    torca_rendezvous_client::RelayTransportFailureKind::Unavailable => {
                        "relay.connection_unavailable"
                    }
                    torca_rendezvous_client::RelayTransportFailureKind::Timeout => {
                        "relay.request_timeout"
                    }
                    torca_rendezvous_client::RelayTransportFailureKind::Disconnected => {
                        "relay.connection_disconnected"
                    }
                    torca_rendezvous_client::RelayTransportFailureKind::InvalidResponse => {
                        "relay.health_response_invalid"
                    }
                })
            })
    }

    fn service_info(&self) -> Option<RelayServiceInfo> {
        self.info.lock().ok().and_then(|value| value.clone())
    }
}
pub(crate) fn spawn_production_runtime(
    engine: EngineHandle,
    bootstrap_observer: TorBootstrapObserver,
    read_receipt_policy: ReadReceiptPolicy,
) -> Result<(RuntimeHandle, RuntimeOwner), NativeCompositionError> {
    let platform = crate::platform_selector::platform_services()?;
    spawn_runtime_for(platform.as_ref(), engine, bootstrap_observer, read_receipt_policy)
}

fn spawn_runtime_for(
    platform: &dyn PlatformServices,
    engine: EngineHandle,
    bootstrap_observer: TorBootstrapObserver,
    read_receipt_policy: ReadReceiptPolicy,
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
    let connectivity = ConnectivityObserver::default();
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
            connectivity: connectivity.clone(),
            read_receipt_policy,
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
    let relay_transport = SharedTorRelayTransport::new(tor_client.clone(), relay.0, relay.1);
    let relay_probe: Arc<dyn RelayProbe> =
        Arc::new(TorRelayProbe { transport: relay_transport.clone(), info: Mutex::new(None) });
    let pairing = build_pairing_driver(
        engine.clone(),
        endpoint,
        relay_transport,
        ManagedIdentityKeys::new(
            RustCryptoProvider,
            platform.open_secret_store(SecretNamespace::Identity),
        ),
        ManagedPeerSecrets::new(
            RustCryptoProvider,
            platform.open_secret_store(SecretNamespace::Runtime),
        ),
        connectivity.clone(),
    )?
    .with_contact_metadata(metadata);
    let pairing = PairingWorkerDriver::spawn(pairing)
        .map_err(|_| NativeCompositionError::new("spawn pairing supervisor failed"))?;
    Ok(RuntimeOwner::spawn_with_connectivity(
        engine,
        pairing,
        communication,
        tor,
        Some(relay_probe),
        connectivity,
    ))
}

fn bind_peer_listener() -> Result<PeerListener, NativeCompositionError> {
    PeerListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
        .map_err(|_| NativeCompositionError::new("bind local peer listener failed"))
}

fn build_pairing_driver<A, S>(
    engine: EngineHandle,
    endpoint: SharedTorEndpoint,
    relay_transport: SharedTorRelayTransport,
    approval: A,
    peer_secrets: S,
    connectivity: ConnectivityObserver,
) -> Result<
    RuntimePairingDriver<RendezvousClient<SharedTorRelayTransport>, RustPairingCrypto, A, S>,
    NativeCompositionError,
>
where
    A: PairingApprovalPort + Send + 'static,
    S: PairingPeerSecretStore + Send + 'static,
{
    let rendezvous =
        RendezvousClient::new(relay_transport, NETWORK_TIMEOUT).with_connectivity(connectivity);
    let coordinator = PairingCoordinator::new(rendezvous, RustPairingCrypto::new());
    let mut runtime = PairingRuntime::new(coordinator, engine.clone(), approval, peer_secrets);
    runtime
        .restore_active_sessions()
        .map_err(|_| NativeCompositionError::new("restore active pairing sessions failed"))?;
    Ok(RuntimePairingDriver::new(runtime, engine, endpoint))
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

pub(crate) fn compiled_relay_endpoint() -> Result<(String, u16), NativeCompositionError> {
    parse_relay_endpoint(COMPILED_RELAY_ENDPOINT)
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
