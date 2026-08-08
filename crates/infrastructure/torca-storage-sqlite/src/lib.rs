//! SQLCipher-backed SQLite storage root.

mod backend;
mod catalog;
mod control_outbox;
mod durable;
mod durable_sqlcipher;
mod inbound_sqlcipher;
mod message_repository;
#[path = "migration_v3.rs"]
mod migration;
mod read_state;
mod receipt_repository;
mod relationship_admin;
mod repository;
mod security_projection;
mod sqlcipher;
mod storage;

pub use backend::{MemoryStorageBackend, StorageBackend, StorageBackendError};
pub use catalog::{
    SqlStatement, contact_sql, conversation_sql, identity_sql, messaging_sql,
    peer_credential_sql, receipt_sql,
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
pub use sqlcipher::{DatabaseKey, SqlCipherBackend};
pub use storage::StorageKernel;
