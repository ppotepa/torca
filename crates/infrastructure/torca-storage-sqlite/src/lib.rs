//! SQLCipher-compatible SQLite storage kernel and compile-time SQL catalog.
//!
//! Batch 05 establishes migrations, transaction boundaries and repository mapping contracts.
//! A concrete SQLite driver is intentionally injected through [`StorageBackend`].

mod backend;
mod catalog;
mod migration;
mod storage;

pub use backend::{MemoryStorageBackend, StorageBackend, StorageBackendError};
pub use catalog::{SqlStatement, identity_sql};
pub use migration::{Migration, MigrationError, MigrationRunner, migrations};
pub use storage::StorageKernel;
