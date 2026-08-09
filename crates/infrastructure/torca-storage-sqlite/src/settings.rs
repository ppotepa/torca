use core::fmt;
use std::path::Path;

use rusqlite::OptionalExtension;

use crate::{DatabaseKey, SqlCipherBackend, SqlCipherStoreOpenError};

/// Durable runtime-owned settings stored in the encrypted application database.
pub struct SqlCipherSettingsStore {
    backend: SqlCipherBackend,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SettingsError {
    Open(SqlCipherStoreOpenError),
    Query,
    Write,
}

impl fmt::Display for SettingsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for SettingsError {}

impl SqlCipherSettingsStore {
    pub fn open(path: impl AsRef<Path>, key: &DatabaseKey) -> Result<Self, SettingsError> {
        let backend = SqlCipherBackend::open(path, key)
            .map_err(|error| SettingsError::Open(SqlCipherStoreOpenError::Backend(error)))?;
        let mut kernel = crate::StorageKernel::new(backend);
        kernel
            .bootstrap()
            .map_err(|error| SettingsError::Open(SqlCipherStoreOpenError::Migration(error)))?;
        Ok(Self { backend: kernel.into_backend() })
    }

    #[cfg(test)]
    fn open_in_memory(key: &DatabaseKey) -> Result<Self, SettingsError> {
        let backend = SqlCipherBackend::open_in_memory(key)
            .map_err(|error| SettingsError::Open(SqlCipherStoreOpenError::Backend(error)))?;
        let mut kernel = crate::StorageKernel::new(backend);
        kernel
            .bootstrap()
            .map_err(|error| SettingsError::Open(SqlCipherStoreOpenError::Migration(error)))?;
        Ok(Self { backend: kernel.into_backend() })
    }

    pub fn notifications_enabled(&self) -> Result<bool, SettingsError> {
        let value = self
            .backend
            .connection()
            .query_row(
                "SELECT bool_value FROM runtime_settings WHERE setting_key = 'notifications_enabled'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(|_| SettingsError::Query)?
            .unwrap_or(1);
        Ok(value != 0)
    }

    pub fn set_notifications_enabled(
        &self,
        enabled: bool,
        updated_at_ms: i64,
    ) -> Result<(), SettingsError> {
        self.backend
            .connection()
            .execute(
                "INSERT INTO runtime_settings(setting_key, bool_value, updated_at_ms) VALUES ('notifications_enabled', ?1, ?2) ON CONFLICT(setting_key) DO UPDATE SET bool_value = excluded.bool_value, updated_at_ms = excluded.updated_at_ms",
                rusqlite::params![i64::from(enabled), updated_at_ms],
            )
            .map_err(|_| SettingsError::Write)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::SqlCipherSettingsStore;
    use crate::DatabaseKey;

    #[test]
    fn notification_setting_is_durable_for_the_connection() {
        let key = DatabaseKey::new([0x17; 32]);
        let store = SqlCipherSettingsStore::open_in_memory(&key).expect("settings store");
        assert!(store.notifications_enabled().expect("read default"));
        store.set_notifications_enabled(false, 42).expect("write setting");
        assert!(!store.notifications_enabled().expect("read updated"));
    }
}
