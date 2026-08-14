use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use torca_client_engine::EngineHandle;
use torca_communication_adapters::{
    ProductionCommunicationInputs, ReadReceiptPolicy, build_production_communication,
};
use torca_connectivity::ConnectivityObserver;
use torca_crypto::Ed25519HandshakeSigner;
use torca_foundation::{OpaqueId, Timestamp};
use torca_identity::{IdentityId, IdentityRepository};
use torca_pairing_driver::RelayPairingDriver;
use torca_radio_coordinator::SharedRadioCoordinator;
use torca_rendezvous_client::{
    DEFAULT_RELAY_MAX_PAYLOAD, OnionRelayService, RelayClientConfig, RelayService,
};
use torca_runtime::{RuntimeHandle, RuntimeOwner};
use torca_storage_sqlite::{DatabaseKey, SqlCipherStore};
use torca_tor::{
    OwnedTorDriver, SharedTorEndpoint, TOR_PEER_VIRTUAL_PORT, TorBootstrapObserver, TorService,
};

use crate::composition::NativeCompositionError;
use crate::platform_selector::{PlatformSecretStores, secret_stores};
use crate::{composition, runtime_event_wake};

const TOR_STARTUP_TIMEOUT: Duration = Duration::from_secs(120);
const RELAY_TIMEOUT: Duration = Duration::from_secs(45);
const RELAY_PROBE_TIMEOUT: Duration = Duration::from_secs(15);
const PAIRING_ATTEMPTS: u32 = 4;

pub(crate) fn spawn_production_runtime(
    engine: EngineHandle,
    bootstrap_observer: Option<TorBootstrapObserver>,
    connectivity: ConnectivityObserver,
    read_receipt_policy: ReadReceiptPolicy,
) -> Result<(RuntimeHandle, RuntimeOwner, SharedRadioCoordinator), NativeCompositionError> {
    let database_path = composition::database_path()?;
    let database_key = composition::load_database_key()?;
    let state_root = composition::runtime_root()?.join("tor");
    let cache_root = composition::runtime_root()?.join("attachments");
    let staging_root = composition::runtime_root()?.join("attachment-staging");
    let relay_endpoint = composition::relay_endpoint()?;
    let PlatformSecretStores {
        identity: identity_secret_store,
        peer: peer_secret_store,
        attachment: attachment_secret_store,
        export: export_secret_store,
        relationship: relationship_secret_store,
    } = secret_stores()?;

    let identity_store = SqlCipherStore::open(&database_path, &database_key)
        .map_err(|_| NativeCompositionError::Storage)?;
    let identity = identity_store
        .get()
        .map_err(|_| NativeCompositionError::Storage)?
        .ok_or(NativeCompositionError::Identity)?;
    let identity_id = identity.id().to_opaque();
    let signer = Ed25519HandshakeSigner::new(identity.key().key_id(), identity_secret_store);

    let peer_listener = TorService::bind_peer_listener(TOR_PEER_VIRTUAL_PORT)
        .map_err(|_| NativeCompositionError::Tor)?;
    let peer_target = peer_listener.local_addr().map_err(|_| NativeCompositionError::Tor)?;
    let endpoint = SharedTorEndpoint::default();
    let now = current_timestamp()?;
    let tor = OwnedTorDriver::bootstrap_observed(
        &state_root,
        peer_target,
        endpoint.clone(),
        TOR_STARTUP_TIMEOUT,
        now,
        bootstrap_observer,
    )
    .map_err(|_| NativeCompositionError::Tor)?;
    let tor_client = tor.client_handle().ok_or(NativeCompositionError::Tor)?;

    let relay = Arc::new(OnionRelayService::new(
        tor_client.clone(),
        RelayClientConfig {
            endpoint: relay_endpoint,
            timeout: RELAY_TIMEOUT,
            max_payload: DEFAULT_RELAY_MAX_PAYLOAD,
        },
    ));
    let pairing_store = SqlCipherStore::open(&database_path, &database_key)
        .map_err(|_| NativeCompositionError::Storage)?;
    let pairing = RelayPairingDriver::new(
        pairing_store,
        relay.clone(),
        endpoint,
        PAIRING_ATTEMPTS,
    );
    let relay_probe = Arc::new(RelayProbeAdapter::new(relay, RELAY_PROBE_TIMEOUT));

    let runtime_event_waker: Arc<dyn Fn() + Send + Sync> =
        Arc::new(runtime_event_wake::signal);
    let communication = build_production_communication(
        engine.clone(),
        &database_path,
        &database_key,
        &cache_root,
        &staging_root,
        ProductionCommunicationInputs {
            signer,
            peer_secret_store,
            attachment_secret_store,
            export_secret_store,
            relationship_secret_store,
            listener: peer_listener,
            tor_client,
            local_identity_id: identity_id,
            connectivity: connectivity.clone(),
            read_receipt_policy,
            runtime_event_waker,
        },
    )?;
    let radio = communication.radio.clone();
    let (handle, owner) = RuntimeOwner::spawn_with_connectivity(
        engine,
        pairing,
        communication.driver,
        tor,
        Some(relay_probe),
        connectivity,
    );
    Ok((handle, owner, radio))
}

struct RelayProbeAdapter {
    service: Arc<OnionRelayService>,
    timeout: Duration,
}
impl RelayProbeAdapter {
    const fn new(service: Arc<OnionRelayService>, timeout: Duration) -> Self {
        Self { service, timeout }
    }
}
impl torca_runtime::RelayProbe for RelayProbeAdapter {
    fn probe(&self) -> Result<(), torca_runtime::RuntimeDriverError> {
        self.service.ping(self.timeout).map_err(|_| torca_runtime::RuntimeDriverError::Communication)
    }

    fn service_info(&self) -> Option<torca_runtime::RelayServiceInfo> {
        self.service
            .service_info(self.timeout)
            .ok()
            .map(|info| torca_runtime::RelayServiceInfo {
                product_version: info.product_version,
                build_id: info.build_id,
                source_commit: info.source_commit,
                protocol_version: info.protocol_version,
            })
    }
}

fn current_timestamp() -> Result<Timestamp, NativeCompositionError> {
    let elapsed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| NativeCompositionError::Clock)?;
    let millis = i64::try_from(elapsed.as_millis()).map_err(|_| NativeCompositionError::Clock)?;
    Timestamp::from_unix_millis(millis).map_err(|_| NativeCompositionError::Clock)
}

#[allow(dead_code)]
fn _typed_identity(id: OpaqueId) -> IdentityId {
    IdentityId::from_opaque(id)
}
