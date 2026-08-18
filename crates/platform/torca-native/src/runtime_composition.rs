use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use torca_client_engine::EngineHandle;
use torca_communication_adapters::{
    ProductionCommunicationInputs, ProductionCommunicationOutput, ReadReceiptPolicy,
    build_production_communication,
};
use torca_connectivity::ConnectivityObserver;
use torca_crypto::{
    ManagedIdentityKeys, ManagedPeerSecrets, OwnedHandshakeSigner, RustCryptoProvider,
    RustPairingCrypto,
};
use torca_pairing_coordinator::{
    PairingApprovalPort, PairingCoordinator, PairingPeerSecretStore, PairingRuntime,
};
use torca_pairing_driver::{PairingWorkerDriver, RuntimePairingDriver};
use torca_platform::{PlatformServices, SecretNamespace};
use torca_radio_coordinator::SharedRadioCoordinator;
use torca_rendezvous_client::{RendezvousClient, SharedTorRelayTransport};
use torca_runtime::{RuntimeHandle, RuntimeOwner};
use torca_tor::{OwnedTorDriver, PeerListener, SharedTorEndpoint, TorBootstrapObserver};

use crate::composition::{NativeCompositionError, load_or_create_database_key};
pub(crate) use crate::relay_endpoint::compiled_relay_endpoint;
use crate::relay_probe::build_relay_probe;

// Keep interactive requests below the application command deadline. Long-lived
// recovery is handled by the pairing supervisor instead of one blocking call.
const NETWORK_TIMEOUT: Duration = Duration::from_secs(8);
// Onion circuits routinely need several seconds even after directory
// bootstrap. Use the same bounded request budget as foreground pairing; the
// durable relay transport still serializes the reconnect lane.
const RELAY_HEALTH_TIMEOUT: Duration = NETWORK_TIMEOUT;
const TOR_STARTUP_TIMEOUT: Duration = Duration::from_secs(15 * 60);

pub(crate) fn spawn_production_runtime(
    engine: EngineHandle,
    bootstrap_observer: TorBootstrapObserver,
    read_receipt_policy: ReadReceiptPolicy,
) -> Result<(RuntimeHandle, RuntimeOwner, SharedRadioCoordinator), NativeCompositionError> {
    let platform = crate::platform_selector::platform_services()?;
    spawn_runtime_for(platform.as_ref(), engine, bootstrap_observer, read_receipt_policy)
}

fn spawn_runtime_for(
    platform: &dyn PlatformServices,
    engine: EngineHandle,
    bootstrap_observer: TorBootstrapObserver,
    read_receipt_policy: ReadReceiptPolicy,
) -> Result<(RuntimeHandle, RuntimeOwner, SharedRadioCoordinator), NativeCompositionError> {
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
    let ProductionCommunicationOutput { driver: communication, radio } =
        build_production_communication(
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

    let relay = platform
        .relay_endpoint()
        .map_err(NativeCompositionError::new)
        .map(|endpoint| (endpoint.host, endpoint.port))?;
    let relay_transport = SharedTorRelayTransport::new(tor_client.clone(), relay.0, relay.1);
    let relay_probe = build_relay_probe(relay_transport.clone(), RELAY_HEALTH_TIMEOUT);
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
    )?;
    let pairing = PairingWorkerDriver::spawn(pairing)
        .map_err(|_| NativeCompositionError::new("spawn pairing supervisor failed"))?;
    let (handle, owner) = RuntimeOwner::spawn_with_connectivity(
        engine,
        pairing,
        communication,
        tor,
        Some(relay_probe),
        connectivity,
    );
    Ok((handle, owner, radio))
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

fn current_timestamp() -> Result<torca_foundation::Timestamp, NativeCompositionError> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| NativeCompositionError::new("system clock is before Unix epoch"))?;
    let millis = i64::try_from(duration.as_millis())
        .map_err(|_| NativeCompositionError::new("system timestamp is out of range"))?;
    torca_foundation::Timestamp::from_unix_millis(millis)
        .map_err(|_| NativeCompositionError::new("system timestamp is invalid"))
}
