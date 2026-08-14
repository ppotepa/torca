//! SQLCipher-backed SQLite storage root.

/// Installed-data epoch. Changing this value requires an explicit deploy reset.
pub const STORAGE_EPOCH: u16 = 2;
/// Baseline schema version for this application generation.
pub const SCHEMA_VERSION: u32 = 1;

mod backend;
mod catalog;
mod control_outbox;
mod durable;
mod durable_sqlcipher;
mod inbound_sqlcipher;
mod message_repository;
mod migration;
mod pairing_repository;
mod pending_operations;
mod radio;
mod read_state;
mod receipt_repository;
mod relationship_admin;
mod repository;
mod security_projection;
mod settings;
mod sqlcipher;
mod storage;

pub use backend::{MemoryStorageBackend, StorageBackend, StorageBackendError};
pub use catalog::{
    SqlStatement, contact_sql, conversation_sql, identity_sql, messaging_sql, peer_credential_sql,
    receipt_sql,
};
pub use control_outbox::SqlCipherControlOutbox;
pub use durable::{
    DurableDeliveryError, DurableDeliveryStore, InMemoryDurableDeliveryStore, OutboxRecord,
    OutboxState,
};
pub use durable_sqlcipher::{SqlCipherDurableStore, SqlCipherDurableStoreOpenError};
pub use inbound_sqlcipher::{SqlCipherInboundStore, SqlCipherInboundStoreOpenError};
pub use message_repository::{
    ConversationMessagePage, ConversationMessageSummary, SqlCipherMessageStore,
    SqlCipherMessageStoreOpenError,
};
pub use migration::{Migration, MigrationError, MigrationRunner, migrations};
pub use pairing_repository::SqlCipherPairingRepository;
pub use pending_operations::{PendingOperationStorageError, SqlCipherPendingOperationStore};
pub use radio::{RadioStorageOpenError, SqlCipherRadioStore};
pub use read_state::{ReadStateError, SqlCipherReadState};
pub use receipt_repository::{SqlCipherReceiptStore, SqlCipherReceiptStoreOpenError};
pub use relationship_admin::{
    RelationshipAdminError, RelationshipCleanup, SqlCipherRelationshipAdmin,
};
pub use repository::{SqlCipherStore, SqlCipherStoreOpenError};
pub use security_projection::{
    ContactSecuritySnapshot, ContactSecurityState, SecurityProjectionError,
    SqlCipherSecurityProjection,
};
pub use settings::{SettingsError, SqlCipherSettingsStore};
pub use sqlcipher::{DatabaseKey, SqlCipherBackend};
pub use storage::StorageKernel;
pub use torca_client_engine::AvatarGenomeRecord;
