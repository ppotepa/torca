use core::fmt;

/// Redaction-safe backend error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StorageBackendError(pub String);
impl fmt::Display for StorageBackendError { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(&self.0) } }
impl std::error::Error for StorageBackendError {}

/// Minimum backend required by the storage kernel.
pub trait StorageBackend {
    /// Returns the current schema version.
    fn schema_version(&self) -> Result<u32, StorageBackendError>;
    /// Starts a transaction.
    fn begin(&mut self) -> Result<(), StorageBackendError>;
    /// Executes a trusted compile-time SQL batch.
    fn execute_batch(&mut self, sql: &'static str) -> Result<(), StorageBackendError>;
    /// Records the schema version within the active transaction.
    fn set_schema_version(&mut self, version: u32) -> Result<(), StorageBackendError>;
    /// Commits the active transaction.
    fn commit(&mut self) -> Result<(), StorageBackendError>;
    /// Rolls back the active transaction.
    fn rollback(&mut self) -> Result<(), StorageBackendError>;
}

/// In-memory backend used to verify ordering and transaction behavior.
#[derive(Clone, Debug, Default)]
pub struct MemoryStorageBackend {
    version: u32,
    in_transaction: bool,
    applied_batches: Vec<&'static str>,
}

impl MemoryStorageBackend {
    /// Returns SQL batches applied in order.
    pub fn applied_batches(&self) -> &[&'static str] { &self.applied_batches }
}

impl StorageBackend for MemoryStorageBackend {
    fn schema_version(&self) -> Result<u32, StorageBackendError> { Ok(self.version) }
    fn begin(&mut self) -> Result<(), StorageBackendError> {
        if self.in_transaction { return Err(StorageBackendError("transaction already active".into())); }
        self.in_transaction = true;
        Ok(())
    }
    fn execute_batch(&mut self, sql: &'static str) -> Result<(), StorageBackendError> {
        if !self.in_transaction { return Err(StorageBackendError("no active transaction".into())); }
        self.applied_batches.push(sql);
        Ok(())
    }
    fn set_schema_version(&mut self, version: u32) -> Result<(), StorageBackendError> {
        if !self.in_transaction { return Err(StorageBackendError("no active transaction".into())); }
        self.version = version;
        Ok(())
    }
    fn commit(&mut self) -> Result<(), StorageBackendError> {
        if !self.in_transaction { return Err(StorageBackendError("no active transaction".into())); }
        self.in_transaction = false;
        Ok(())
    }
    fn rollback(&mut self) -> Result<(), StorageBackendError> { self.in_transaction = false; Ok(()) }
}
