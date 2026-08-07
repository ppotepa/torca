use core::fmt;
use std::net::SocketAddr;
use std::path::Path;
use std::time::Duration;

use torca_attachment_sqlite::SqlCipherAttachmentStore;
use torca_attachment_transfer::AttachmentTransfer;
use torca_client_engine::EngineHandle;
use torca_communication_driver::TorcaCommunicationDriver;
use torca_control_delivery::{ControlDeliveryWorker, ControlOutbox};
use torca_crypto::{ManagedPeerSecrets, ProtectedSecretStore, RustCryptoProvider};
use torca_delivery::DeliveryWorker;
use torca_file_storage::FileBlobStore;
use torca_foundation::OpaqueId;
use torca_messaging::RetryPolicy;
use torca_peer_link::PeerLink;
use torca_peer_protocol::HandshakeSigner;
use torca_peer_shared::SharedPeerLink;
use torca_read_state::SqlCipherReadState;
use torca_storage_sqlite::{
    DatabaseKey, SqlCipherDurableStore, SqlCipherInboundStore, SqlCipherMessageStore,
    SqlCipherStore,
};
use torca_transport_tor::PeerListener;

use crate::{
    AttachmentAdapter, InboundTextReceiptAdapter, PeerLinkAdapter, ReadStateAdapter,
    ReceiptPeerTransport, SharedControlWorker, SharedPeerCrypto, TextPeerTransport,
    TextWorkerAdapter,
};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
const ACK_TIMEOUT: Duration = Duration::from_secs(30);
const RETRY_MAX_ATTEMPTS: u32 = 12;
const RETRY_BASE: Duration = Duration::from_secs(1);
const RETRY_MAX: Duration = Duration::from_secs(60);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommunicationBuildError {
    Storage,
    Attachment,
    Cache,
}
impl fmt::Display for CommunicationBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for CommunicationBuildError {}

/// Platform-specific secure dependencies passed into the common communication composition.
pub struct ProductionCommunicationInputs<K, P, AP> {
    pub signer: K,
    pub peer_secret_store: P,
    pub attachment_secret_store: AP,
    pub listener: PeerListener,
    pub socks_address: SocketAddr,
    pub local_identity_id: OpaqueId,
}

#[allow(clippy::too_many_lines)]
pub fn build_production_communication<K, P, AP>(
    engine: EngineHandle,
    database_path: &Path,
    database_key: &DatabaseKey,
    cache_root: &Path,
    staging_root: &Path,
    inputs: ProductionCommunicationInputs<K, P, AP>,
) -> Result<TorcaCommunicationDriver, CommunicationBuildError>
where
    K: HandshakeSigner + Send + 'static,
    P: ProtectedSecretStore + Send + 'static,
    AP: ProtectedSecretStore + Send + 'static,
{
    let peer_relationships = SqlCipherStore::open(database_path, database_key)
        .map_err(|_| CommunicationBuildError::Storage)?;
    let link = SharedPeerLink::new(PeerLink::new(
        inputs.listener,
        peer_relationships,
        inputs.signer,
        inputs.local_identity_id,
        inputs.socks_address,
        CONNECT_TIMEOUT,
    ));

    let shared_crypto = SharedPeerCrypto::new(ManagedPeerSecrets::new(
        RustCryptoProvider,
        inputs.peer_secret_store,
    ));

    let text_relationships = SqlCipherStore::open(database_path, database_key)
        .map_err(|_| CommunicationBuildError::Storage)?;
    let text_transport = TextPeerTransport::new(
        text_relationships,
        link.clone(),
        shared_crypto.clone(),
        inputs.local_identity_id,
        ACK_TIMEOUT,
    );
    let text_store = SqlCipherDurableStore::open(database_path, database_key)
        .map_err(|_| CommunicationBuildError::Storage)?;
    let text = TextWorkerAdapter::new(DeliveryWorker::new(
        text_store,
        text_transport,
        RetryPolicy {
            max_attempts: RETRY_MAX_ATTEMPTS,
            base_delay: RETRY_BASE,
            max_delay: RETRY_MAX,
        },
    ));

    let control_relationships = SqlCipherStore::open(database_path, database_key)
        .map_err(|_| CommunicationBuildError::Storage)?;
    let control_transport = ReceiptPeerTransport::new(
        control_relationships,
        link.clone(),
        shared_crypto.clone(),
        inputs.local_identity_id,
        ACK_TIMEOUT,
    );
    let control_outbox = ControlOutbox::open(database_path, database_key)
        .map_err(|_| CommunicationBuildError::Storage)?;
    let control = SharedControlWorker::new(ControlDeliveryWorker::new(
        control_outbox,
        control_transport,
    ));

    let inbound_relationships = SqlCipherStore::open(database_path, database_key)
        .map_err(|_| CommunicationBuildError::Storage)?;
    let inbound_store = SqlCipherInboundStore::open(database_path, database_key)
        .map_err(|_| CommunicationBuildError::Storage)?;
    let inbound = InboundTextReceiptAdapter::new(
        inbound_relationships,
        link.clone(),
        shared_crypto,
        inbound_store,
        control.clone(),
        engine.clone(),
        inputs.local_identity_id,
    );

    let attachment_relationships = SqlCipherStore::open(database_path, database_key)
        .map_err(|_| CommunicationBuildError::Storage)?;
    let attachment_messages = SqlCipherMessageStore::open(database_path, database_key)
        .map_err(|_| CommunicationBuildError::Storage)?;
    let attachment_metadata = SqlCipherAttachmentStore::open(database_path, database_key)
        .map_err(|_| CommunicationBuildError::Attachment)?;
    let attachment_cache = FileBlobStore::open(cache_root)
        .map_err(|_| CommunicationBuildError::Cache)?;
    let attachment_transfer = AttachmentTransfer::new(
        attachment_relationships,
        attachment_messages,
        link.clone(),
        ManagedPeerSecrets::new(RustCryptoProvider, inputs.attachment_secret_store),
        attachment_metadata,
        attachment_cache,
        staging_root,
        inputs.local_identity_id,
        ACK_TIMEOUT,
    )
    .map_err(|_| CommunicationBuildError::Attachment)?;
    let attachments = AttachmentAdapter::new(attachment_transfer);

    let read_state = SqlCipherReadState::open(database_path, database_key)
        .map_err(|_| CommunicationBuildError::Storage)?;

    Ok(TorcaCommunicationDriver::new(
        engine,
        Box::new(PeerLinkAdapter::new(link)),
        Box::new(text),
        Box::new(control),
        Box::new(inbound),
        Box::new(attachments),
        Box::new(ReadStateAdapter::new(read_state)),
    ))
}
