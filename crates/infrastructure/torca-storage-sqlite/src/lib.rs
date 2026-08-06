//! SQLCipher-backed SQLite storage, durable delivery implementations and embedded SQL catalog.

mod backend;
mod catalog;
mod durable;
mod durable_sqlcipher;
mod migration;
mod repository;
mod sqlcipher;
mod storage;

pub use backend::{MemoryStorageBackend, StorageBackend, StorageBackendError};
pub use catalog::{SqlStatement, identity_sql, messaging_sql};
pub use durable::{
    DurableDeliveryError, DurableDeliveryStore, InMemoryDurableDeliveryStore, OutboxRecord,
    OutboxState,
};
pub use durable_sqlcipher::{SqlCipherDurableStore, SqlCipherDurableStoreOpenError};
pub use migration::{Migration, MigrationError, MigrationRunner, migrations};
pub use repository::{SqlCipherStore, SqlCipherStoreOpenError};
pub use sqlcipher::{DatabaseKey, SqlCipherBackend};
pub use storage::StorageKernel;
