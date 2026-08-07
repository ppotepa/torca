//! Final SQLCipher-backed SQLite storage root for Torca 0.1.

#[path = "backend.rs"]
mod backend;
#[path = "catalog.rs"]
mod catalog;
#[path = "control_outbox.rs"]
mod control_outbox;
#[path = "durable.rs"]
mod durable;
#[path = "durable_sqlcipher.rs"]
mod durable_sqlcipher;
#[path = "inbound_sqlcipher.rs"]
mod inbound_sqlcipher;
#[path = "message_repository.rs"]
mod message_repository;
#[path = "migration_v3.rs"]
mod migration;
#[path = "read_state.rs"]
mod read_state;
#[path = "receipt_repository.rs"]
mod receipt_repository;
#[path = "repository.rs"]
mod repository;
#[path = "sqlcipher.rs"]
mod sqlcipher;
#[path = "storage.rs"]
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
pub use message_repository::{SqlCipherMessageStore, SqlCipherMessageStoreOpenError};
pub use migration::{Migration, MigrationError, MigrationRunner, migrations};
pub use read_state::{ReadStateError, SqlCipherReadState};
pub use receipt_repository::{SqlCipherReceiptStore, SqlCipherReceiptStoreOpenError};
pub use repository::{SqlCipherStore, SqlCipherStoreOpenError};
pub use sqlcipher::{DatabaseKey, SqlCipherBackend};
pub use storage::StorageKernel;
