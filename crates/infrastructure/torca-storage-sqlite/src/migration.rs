use crate::{StorageBackend, StorageBackendError};
use core::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Migration {
    pub version: u32,
    pub name: &'static str,
    pub sql: &'static str,
}

pub const fn migrations() -> &'static [Migration] {
    &MIGRATIONS
}

const MIGRATIONS: [Migration; 1] = [Migration {
    version: 1,
    name: "baseline",
    sql: include_str!("../sql/migrations/0001_baseline.sql"),
}];

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MigrationError {
    InvalidOrder,
    DatabaseTooNew { database: u32, supported: u32 },
    Backend(StorageBackendError),
}
impl fmt::Display for MigrationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for MigrationError {}
impl From<StorageBackendError> for MigrationError {
    fn from(value: StorageBackendError) -> Self {
        Self::Backend(value)
    }
}

pub struct MigrationRunner;
impl MigrationRunner {
    pub fn migrate<B: StorageBackend>(backend: &mut B) -> Result<u32, MigrationError> {
        let latest = MIGRATIONS.last().map_or(0, |migration| migration.version);
        for pair in MIGRATIONS.windows(2) {
            if pair[0].version >= pair[1].version {
                return Err(MigrationError::InvalidOrder);
            }
        }
        let current = backend.schema_version()?;
        if current > latest {
            return Err(MigrationError::DatabaseTooNew { database: current, supported: latest });
        }
        for migration in MIGRATIONS.iter().filter(|migration| migration.version > current) {
            backend.begin()?;
            if let Err(error) = backend
                .execute_batch(migration.sql)
                .and_then(|()| backend.set_schema_version(migration.version))
                .and_then(|()| backend.commit())
            {
                let _ = backend.rollback();
                return Err(MigrationError::Backend(error));
            }
        }
        Ok(latest)
    }
}
