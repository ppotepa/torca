use core::fmt;

use crate::{StorageBackend, StorageBackendError};

/// One monotonic embedded schema migration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Migration { pub version: u32, pub name: &'static str, pub sql: &'static str }

/// Canonical migration list.
pub const fn migrations() -> &'static [Migration] { &MIGRATIONS }

const MIGRATIONS: [Migration; 2] = [
    Migration { version: 1, name: "foundation", sql: include_str!("../sql/migrations/0001_foundation.sql") },
    Migration { version: 2, name: "identity", sql: include_str!("../sql/migrations/0002_identity.sql") },
];

/// Migration failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MigrationError {
    /// Embedded migrations are not strictly ordered.
    InvalidOrder,
    /// Database schema is newer than this binary.
    DatabaseTooNew { database: u32, supported: u32 },
    /// Backend operation failed.
    Backend(StorageBackendError),
}
impl fmt::Display for MigrationError { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{self:?}") } }
impl std::error::Error for MigrationError {}
impl From<StorageBackendError> for MigrationError { fn from(value: StorageBackendError) -> Self { Self::Backend(value) } }

/// Applies embedded migrations transactionally and in order.
pub struct MigrationRunner;
impl MigrationRunner {
    /// Migrates a backend to the latest known version.
    pub fn migrate<B: StorageBackend>(backend: &mut B) -> Result<u32, MigrationError> {
        let latest = MIGRATIONS.last().map_or(0, |migration| migration.version);
        for pair in MIGRATIONS.windows(2) {
            if pair[0].version >= pair[1].version { return Err(MigrationError::InvalidOrder); }
        }
        let current = backend.schema_version()?;
        if current > latest { return Err(MigrationError::DatabaseTooNew { database: current, supported: latest }); }
        for migration in MIGRATIONS.iter().filter(|migration| migration.version > current) {
            backend.begin()?;
            if let Err(error) = backend.execute_batch(migration.sql).and_then(|()| backend.set_schema_version(migration.version)).and_then(|()| backend.commit()) {
                let _ = backend.rollback();
                return Err(MigrationError::Backend(error));
            }
        }
        Ok(latest)
    }
}
