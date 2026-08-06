//! SQLCipher-compatible SQLite storage kernel, durable delivery contracts and SQL catalog.

mod backend;
mod catalog;
mod durable;
mod migration;
mod storage;

pub use backend::{MemoryStorageBackend, StorageBackend, StorageBackendError};
pub use catalog::{SqlStatement, identity_sql, messaging_sql};
pub use durable::{
    DurableDeliveryError, DurableDeliveryStore, InMemoryDurableDeliveryStore, OutboxRecord,
    OutboxState,
};
pub use migration::{Migration, MigrationError, MigrationRunner, migrations};
pub use storage::StorageKernel;
