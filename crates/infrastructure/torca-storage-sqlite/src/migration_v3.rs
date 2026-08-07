use crate::{StorageBackend, StorageBackendError};
use core::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Migration {
    pub version: u32,
    pub name: &'static str,
    pub sql: &'static str,
}

pub const fn migrations() -> &'static [Migration] { &MIGRATIONS }

const MIGRATIONS: [Migration; 17] = [
    Migration { version: 1, name: "foundation", sql: include_str!("../sql/migrations/0001_foundation.sql") },
    Migration { version: 2, name: "identity", sql: include_str!("../sql/migrations/0002_identity.sql") },
    Migration { version: 3, name: "messaging", sql: include_str!("../sql/migrations/0003_messaging.sql") },
    Migration { version: 4, name: "contacts_conversations", sql: include_str!("../sql/migrations/0004_contacts_conversations.sql") },
    Migration { version: 5, name: "message_attempt_count", sql: include_str!("../sql/migrations/0005_message_attempt_count.sql") },
    Migration { version: 6, name: "outbound_message_outbox_invariant", sql: include_str!("../sql/migrations/0006_outbound_message_outbox_invariant.sql") },
    Migration { version: 7, name: "stale_delivery_requeue", sql: include_str!("../sql/migrations/0007_stale_delivery_requeue.sql") },
    Migration { version: 8, name: "delivery_message_state_lifecycle", sql: include_str!("../sql/migrations/0008_delivery_message_state_lifecycle.sql") },
    Migration { version: 9, name: "peer_credentials", sql: include_str!("../sql/migrations/0009_peer_credentials.sql") },
    Migration { version: 10, name: "receipt_message_lifecycle", sql: include_str!("../sql/migrations/0010_receipt_message_lifecycle.sql") },
    Migration { version: 11, name: "control_outbox", sql: include_str!("../sql/migrations/0011_control_outbox.sql") },
    Migration { version: 12, name: "attachments", sql: include_str!("../sql/migrations/0012_attachments.sql") },
    Migration { version: 13, name: "delivery_attempt_sync", sql: include_str!("../sql/migrations/0013_delivery_attempt_sync.sql") },
    Migration { version: 14, name: "failed_message_dead_letters_outbox", sql: include_str!("../sql/migrations/0014_failed_message_dead_letters_outbox.sql") },
    Migration { version: 15, name: "retry_message_requeues_outbox", sql: include_str!("../sql/migrations/0015_retry_message_requeues_outbox.sql") },
    Migration { version: 16, name: "contact_metadata", sql: include_str!("../sql/migrations/0016_contact_metadata.sql") },
    Migration { version: 17, name: "unique_remote_identity", sql: include_str!("../sql/migrations/0017_unique_remote_identity.sql") },
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MigrationError {
    InvalidOrder,
    DatabaseTooNew { database: u32, supported: u32 },
    Backend(StorageBackendError),
}
impl fmt::Display for MigrationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result { write!(formatter, "{self:?}") }
}
impl std::error::Error for MigrationError {}
impl From<StorageBackendError> for MigrationError { fn from(value: StorageBackendError) -> Self { Self::Backend(value) } }

pub struct MigrationRunner;
impl MigrationRunner {
    pub fn migrate<B: StorageBackend>(backend: &mut B) -> Result<u32, MigrationError> {
        let latest = MIGRATIONS.last().map_or(0, |migration| migration.version);
        for pair in MIGRATIONS.windows(2) {
            if pair[0].version >= pair[1].version { return Err(MigrationError::InvalidOrder); }
        }
        let current = backend.schema_version()?;
        if current > latest { return Err(MigrationError::DatabaseTooNew { database: current, supported: latest }); }
        for migration in MIGRATIONS.iter().filter(|migration| migration.version > current) {
            backend.begin()?;
            if let Err(error) = backend.execute_batch(migration.sql)
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
