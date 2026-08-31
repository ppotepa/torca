use core::fmt;
use std::path::Path;

use rusqlite::OptionalExtension;

use crate::{
    DatabaseKey, MigrationError, STORAGE_EPOCH, SqlCipherBackend, StorageBackendError,
    StorageKernel,
};

const METADATA_TABLE: &str = "torca_storage_metadata";

/// Failure while validating installed data before schema migrations run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StorageCompatibilityError {
    Backend(StorageBackendError),
    Migration(MigrationError),
    IncompatibleEpoch { found: u16, expected: u16 },
    InvalidEpochMetadata,
}

impl fmt::Display for StorageCompatibilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IncompatibleEpoch { found, expected } => write!(
                formatter,
                "INCOMPATIBLE_STORAGE_EPOCH: found={found} expected={expected} reset_required=true"
            ),
            Self::InvalidEpochMetadata => write!(
                formatter,
                "INCOMPATIBLE_STORAGE_EPOCH: invalid epoch metadata expected={} reset_required=true",
                STORAGE_EPOCH
            ),
            Self::Backend(error) => {
                write!(formatter, "storage compatibility check failed: {error}")
            }
            Self::Migration(error) => write!(formatter, "storage migration failed: {error}"),
        }
    }
}

impl std::error::Error for StorageCompatibilityError {}

impl From<StorageBackendError> for StorageCompatibilityError {
    fn from(value: StorageBackendError) -> Self {
        Self::Backend(value)
    }
}

impl From<MigrationError> for StorageCompatibilityError {
    fn from(value: MigrationError) -> Self {
        Self::Migration(value)
    }
}

/// Validates the installed-data epoch before applying migrations and stamps
/// compatible databases that predate explicit epoch metadata.
pub fn prepare_database(
    path: impl AsRef<Path>,
    key: &DatabaseKey,
) -> Result<(), StorageCompatibilityError> {
    let backend = SqlCipherBackend::open(path, key)?;
    prepare_backend(backend)
}

fn prepare_backend(backend: SqlCipherBackend) -> Result<(), StorageCompatibilityError> {
    validate_epoch(&backend)?;
    let mut kernel = StorageKernel::new(backend);
    kernel.bootstrap()?;
    let backend = kernel.into_backend();
    match stored_epoch(&backend)? {
        Some(epoch) if epoch == STORAGE_EPOCH => Ok(()),
        Some(epoch) => Err(StorageCompatibilityError::IncompatibleEpoch {
            found: epoch,
            expected: STORAGE_EPOCH,
        }),
        None => Err(StorageCompatibilityError::InvalidEpochMetadata),
    }
}

fn validate_epoch(backend: &SqlCipherBackend) -> Result<(), StorageCompatibilityError> {
    if let Some(epoch) = stored_epoch(backend)? {
        if epoch != STORAGE_EPOCH {
            return Err(StorageCompatibilityError::IncompatibleEpoch {
                found: epoch,
                expected: STORAGE_EPOCH,
            });
        }
    }

    // Epoch 2 contacts required this column. Detect it before migration 17/18
    // can make the legacy profile look current while leaving INSERT broken.
    if table_has_column(backend, "contacts", "onion_address")? {
        return Err(StorageCompatibilityError::IncompatibleEpoch {
            found: 2,
            expected: STORAGE_EPOCH,
        });
    }
    Ok(())
}

fn stored_epoch(backend: &SqlCipherBackend) -> Result<Option<u16>, StorageCompatibilityError> {
    if !table_exists(backend, METADATA_TABLE)? {
        return Ok(None);
    }
    let value = backend
        .connection()
        .query_row(include_str!("../sql/queries/storage_epoch_select.sql"), [], |row| {
            row.get::<_, i64>(0)
        })
        .optional()
        .map_err(|error| StorageBackendError(format!("storage epoch query failed: {error}")))?;
    value
        .map(|value| {
            u16::try_from(value).map_err(|_| StorageCompatibilityError::InvalidEpochMetadata)
        })
        .transpose()
}

fn table_exists(
    backend: &SqlCipherBackend,
    table: &str,
) -> Result<bool, StorageCompatibilityError> {
    backend
        .connection()
        .query_row(include_str!("../sql/queries/storage_table_exists.sql"), [table], |_| Ok(()))
        .optional()
        .map(|value| value.is_some())
        .map_err(|error| {
            StorageBackendError(format!("storage catalog query failed: {error}")).into()
        })
}

fn table_has_column(
    backend: &SqlCipherBackend,
    table: &str,
    column: &str,
) -> Result<bool, StorageCompatibilityError> {
    if !table_exists(backend, table)? {
        return Ok(false);
    }
    let sql = format!("PRAGMA table_info({table})");
    let mut statement = backend
        .connection()
        .prepare(&sql)
        .map_err(|error| StorageBackendError(format!("storage schema query failed: {error}")))?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| StorageBackendError(format!("storage schema query failed: {error}")))?;
    for value in columns {
        if value
            .map_err(|error| StorageBackendError(format!("storage schema query failed: {error}")))?
            == column
        {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn backend() -> SqlCipherBackend {
        SqlCipherBackend::open_in_memory(&DatabaseKey::new([0x42; 32])).expect("open SQLCipher")
    }

    #[test]
    fn clean_database_is_stamped_with_current_epoch() {
        let backend = backend();
        prepare_backend(backend).expect("prepare clean database");
    }

    #[test]
    fn legacy_onion_schema_is_rejected_before_migration() {
        let backend = backend();
        backend
            .connection()
            .execute_batch(
                "CREATE TABLE contacts (contact_id BLOB PRIMARY KEY, onion_address TEXT NOT NULL);",
            )
            .expect("create legacy contacts");
        assert_eq!(
            validate_epoch(&backend),
            Err(StorageCompatibilityError::IncompatibleEpoch { found: 2, expected: STORAGE_EPOCH })
        );
        assert!(table_has_column(&backend, "contacts", "onion_address").expect("inspect schema"));
    }

    #[test]
    fn explicit_mismatched_epoch_is_rejected() {
        let backend = backend();
        backend
            .connection()
            .execute_batch(
                "CREATE TABLE torca_storage_metadata (singleton INTEGER PRIMARY KEY, storage_epoch INTEGER NOT NULL);\
                 INSERT INTO torca_storage_metadata(singleton, storage_epoch) VALUES (1, 2);",
            )
            .expect("create epoch metadata");
        assert_eq!(
            validate_epoch(&backend),
            Err(StorageCompatibilityError::IncompatibleEpoch { found: 2, expected: STORAGE_EPOCH })
        );
    }

    #[test]
    fn explicit_current_epoch_is_accepted() {
        let backend = backend();
        backend
            .connection()
            .execute_batch(include_str!("../sql/migrations/0018_storage_epoch.sql"))
            .expect("create current epoch metadata");
        assert_eq!(validate_epoch(&backend), Ok(()));
    }

    #[test]
    fn future_epoch_is_rejected_without_downgrade() {
        let backend = backend();
        backend
            .connection()
            .execute_batch(
                "CREATE TABLE torca_storage_metadata (singleton INTEGER PRIMARY KEY, storage_epoch INTEGER NOT NULL);\
                 INSERT INTO torca_storage_metadata(singleton, storage_epoch) VALUES (1, 4);",
            )
            .expect("create future epoch metadata");
        assert_eq!(
            validate_epoch(&backend),
            Err(StorageCompatibilityError::IncompatibleEpoch { found: 4, expected: STORAGE_EPOCH })
        );
    }
}
