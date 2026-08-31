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

const MIGRATIONS: [Migration; 20] = [
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
    Migration {
        version: 6,
        name: "attachment_failure_code",
        sql: include_str!("../sql/migrations/0006_attachment_failure_code.sql"),
    },
    Migration {
        version: 7,
        name: "radio_mode",
        sql: include_str!("../sql/migrations/0007_radio_mode.sql"),
    },
    Migration {
        version: 8,
        name: "message_reactions",
        sql: include_str!("../sql/migrations/0008_message_reactions.sql"),
    },
    Migration {
        version: 9,
        name: "avatar_genomes",
        sql: include_str!("../sql/migrations/0009_avatar_genomes.sql"),
    },
    Migration {
        version: 10,
        name: "pairing_avatar",
        sql: include_str!("../sql/migrations/0010_pairing_avatar.sql"),
    },
    Migration {
        version: 11,
        name: "contact_avatar_genomes",
        sql: include_str!("../sql/migrations/0011_contact_avatar_genomes.sql"),
    },
    Migration {
        version: 12,
        name: "battery_preferences",
        sql: include_str!("../sql/migrations/0012_battery_preferences.sql"),
    },
    Migration {
        version: 13,
        name: "contact_availability",
        sql: include_str!("../sql/migrations/0013_contact_availability.sql"),
    },
    Migration {
        version: 14,
        name: "contact_transport_endpoints",
        sql: include_str!("../sql/migrations/0014_contact_transport_endpoints.sql"),
    },
    Migration {
        version: 15,
        name: "pairing_transport_endpoints",
        sql: include_str!("../sql/migrations/0015_pairing_transport_endpoints.sql"),
    },
    Migration {
        version: 16,
        name: "notification_outbox",
        sql: include_str!("../sql/migrations/0016_notification_outbox.sql"),
    },
    Migration {
        version: 17,
        name: "iroh_only_routes",
        sql: include_str!("../sql/migrations/0017_iroh_only_routes.sql"),
    },
    Migration {
        version: 18,
        name: "storage_epoch",
        sql: include_str!("../sql/migrations/0018_storage_epoch.sql"),
    },
    Migration {
        version: 19,
        name: "profile_country",
        sql: include_str!("../sql/migrations/0019_profile_country.sql"),
    },
    Migration {
        version: 20,
        name: "contact_country",
        sql: include_str!("../sql/migrations/0020_contact_country.sql"),
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
