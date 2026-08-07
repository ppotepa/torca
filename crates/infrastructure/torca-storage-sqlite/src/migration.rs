use crate::{StorageBackend, StorageBackendError};
use core::fmt;

/// One monotonic embedded schema migration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Migration {
    pub version: u32,
    pub name: &'static str,
    pub sql: &'static str,
}
/// Canonical migration list.
pub const fn migrations() -> &'static [Migration] {
    &MIGRATIONS
}
const MIGRATIONS: [Migration; 8] = [
    Migration {
        version: 1,
        name: "foundation",
        sql: include_str!("../sql/migrations/0001_foundation.sql"),
    },
    Migration {
        version: 2,
        name: "identity",
        sql: include_str!("../sql/migrations/0002_identity.sql"),
    },
    Migration {
        version: 3,
        name: "messaging",
        sql: include_str!("../sql/migrations/0003_messaging.sql"),
    },
    Migration {
        version: 4,
        name: "contacts_conversations",
        sql: include_str!("../sql/migrations/0004_contacts_conversations.sql"),
    },
    Migration {
        version: 5,
        name: "message_attempt_count",
        sql: include_str!("../sql/migrations/0005_message_attempt_count.sql"),
    },
    Migration {
        version: 6,
        name: "outbound_message_outbox_invariant",
        sql: include_str!("../sql/migrations/0006_outbound_message_outbox_invariant.sql"),
    },
    Migration {
        version: 7,
        name: "stale_delivery_requeue",
        sql: include_str!("../sql/migrations/0007_stale_delivery_requeue.sql"),
    },
    Migration {
        version: 8,
        name: "delivery_message_state_lifecycle",
        sql: include_str!("../sql/migrations/0008_delivery_message_state_lifecycle.sql"),
    },
];
/// Migration failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MigrationError {
    InvalidOrder,
    DatabaseTooNew { database: u32, supported: u32 },
    Backend(StorageBackendError),
}
impl fmt::Display for MigrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for MigrationError {}
impl From<StorageBackendError> for MigrationError {
    fn from(value: StorageBackendError) -> Self {
        Self::Backend(value)
    }
}
/// Applies migrations transactionally and in order.
pub struct MigrationRunner;
impl MigrationRunner {
    /// Migrates to latest version.
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
