//! Compatibility facade for the storage-owned transactional Read-state adapter.
//!
//! Operational SQL and SQLCipher ownership live in `torca-storage-sqlite`.

pub use torca_storage_sqlite::{ReadStateError, SqlCipherReadState};
