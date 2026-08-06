use core::fmt;

/// Redaction-safe backend error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StorageBackendError(pub String);
impl fmt::Display for StorageBackendError { fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result { formatter.write_str(&self.0) } }
impl std::error::Error for StorageBackendError {}

/// Minimum backend required by the storage kernel.
pub trait StorageBackend {
    /// Returns current schema version.
    fn schema_version(&self) -> Result<u32, StorageBackendError>;
    /// Executes trusted connection-scoped configuration outside a transaction.
    fn execute_connection_batch(&mut self, sql: &'static str) -> Result<(), StorageBackendError>;
    /// Starts a transaction.
    fn begin(&mut self) -> Result<(), StorageBackendError>;
    /// Executes a trusted migration batch in the active transaction.
    fn execute_batch(&mut self, sql: &'static str) -> Result<(), StorageBackendError>;
    /// Stages schema version in the active transaction.
    fn set_schema_version(&mut self, version: u32) -> Result<(), StorageBackendError>;
    /// Commits the active transaction.
    fn commit(&mut self) -> Result<(), StorageBackendError>;
    /// Rolls back the active transaction.
    fn rollback(&mut self) -> Result<(), StorageBackendError>;
}

/// In-memory backend that models commit and rollback rather than mutating committed state early.
#[derive(Clone, Debug, Default)]
pub struct MemoryStorageBackend { version: u32, in_transaction: bool, applied_batches: Vec<&'static str>, pending_batches: Vec<&'static str>, pending_version: Option<u32> }
impl MemoryStorageBackend { /// Returns committed SQL batches in order.
    pub fn applied_batches(&self) -> &[&'static str] { &self.applied_batches } }
impl StorageBackend for MemoryStorageBackend {
    fn schema_version(&self) -> Result<u32, StorageBackendError> { Ok(self.version) }
    fn execute_connection_batch(&mut self, sql: &'static str) -> Result<(), StorageBackendError> { if self.in_transaction { return Err(StorageBackendError("connection batch cannot run in transaction".into())); } self.applied_batches.push(sql); Ok(()) }
    fn begin(&mut self) -> Result<(), StorageBackendError> { if self.in_transaction { return Err(StorageBackendError("transaction already active".into())); } self.in_transaction = true; self.pending_batches.clear(); self.pending_version = None; Ok(()) }
    fn execute_batch(&mut self, sql: &'static str) -> Result<(), StorageBackendError> { if !self.in_transaction { return Err(StorageBackendError("no active transaction".into())); } self.pending_batches.push(sql); Ok(()) }
    fn set_schema_version(&mut self, version: u32) -> Result<(), StorageBackendError> { if !self.in_transaction { return Err(StorageBackendError("no active transaction".into())); } self.pending_version = Some(version); Ok(()) }
    fn commit(&mut self) -> Result<(), StorageBackendError> { if !self.in_transaction { return Err(StorageBackendError("no active transaction".into())); } self.applied_batches.append(&mut self.pending_batches); if let Some(version) = self.pending_version.take() { self.version = version; } self.in_transaction = false; Ok(()) }
    fn rollback(&mut self) -> Result<(), StorageBackendError> { self.pending_batches.clear(); self.pending_version = None; self.in_transaction = false; Ok(()) }
}
