use crate::{MigrationError, MigrationRunner, StorageBackend};

/// Storage composition root around an injected SQLite-compatible backend.
pub struct StorageKernel<B> { backend: B }

impl<B: StorageBackend> StorageKernel<B> {
    /// Creates a kernel.
    pub const fn new(backend: B) -> Self { Self { backend } }
    /// Applies required PRAGMA configuration and embedded migrations.
    pub fn bootstrap(&mut self) -> Result<u32, MigrationError> {
        self.backend.begin()?;
        let pragmas = include_str!("../sql/bootstrap.sql");
        if let Err(error) = self.backend.execute_batch(pragmas).and_then(|()| self.backend.commit()) {
            let _ = self.backend.rollback();
            return Err(MigrationError::Backend(error));
        }
        MigrationRunner::migrate(&mut self.backend)
    }
    /// Returns the backend.
    pub const fn backend(&self) -> &B { &self.backend }
    /// Consumes the kernel.
    pub fn into_backend(self) -> B { self.backend }
}

#[cfg(test)]
mod tests {
    use crate::{MemoryStorageBackend, StorageKernel};

    #[test]
    fn bootstrap_applies_pragmas_and_ordered_migrations() {
        let mut kernel = StorageKernel::new(MemoryStorageBackend::default());
        assert_eq!(kernel.bootstrap().expect("bootstrap succeeds"), 2);
        assert_eq!(kernel.backend().applied_batches().len(), 3);
    }
}
