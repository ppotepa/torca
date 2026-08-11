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

const MIGRATIONS: [Migration; 5] = [
    Migration {
        version: 1,
        name: "baseline",
        sql: include_str!("../sql/migrations/0001_baseline.sql"),
    },
    Migration {
        version: 2,
        name: "pairing_display_name",
        sql: include_str!("../sql/migrations/0002_pairing_display_name.sql"),
    },
    Migration {
        version: 3,
        name: "message_lifecycle",
        sql: include_str!("../sql/migrations/0003_message_lifecycle.sql"),
    },
    Migration {
        version: 4,
        name: "pending_operations",
        sql: include_str!("../sql/migrations/0004_pending_operations.sql"),
    },
    Migration {
        version: 5,
        name: "runtime_privacy",
        sql: include_str!("../sql/migrations/0005_runtime_privacy.sql"),
    },
];

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
