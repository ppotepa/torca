use core::fmt;
use std::path::Path;

use rusqlite::OptionalExtension;

use crate::{DatabaseKey, SqlCipherBackend, SqlCipherStoreOpenError};

const BOOL_SETTING_SQL: &str = include_str!("../sql/queries/runtime_bool_setting.sql");
const SETTING_UPDATED_AT_SQL: &str =
    include_str!("../sql/queries/runtime_setting_updated_at.sql");
const UPSERT_BOOL_SETTING_SQL: &str =
    include_str!("../sql/commands/runtime_bool_setting_upsert.sql");
const NOTIFICATIONS_ENABLED: &str = "notifications_enabled";
const READ_RECEIPTS_ENABLED: &str = "read_receipts_enabled";
const NEW_CONTACTS_ACKNOWLEDGED: &str = "new_contacts_acknowledged";

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
        self.bool_setting(NOTIFICATIONS_ENABLED, true)
    }

    pub fn set_notifications_enabled(
        &self,
        enabled: bool,
        updated_at_ms: i64,
    ) -> Result<(), SettingsError> {
        self.set_bool_setting(NOTIFICATIONS_ENABLED, enabled, updated_at_ms)
    }

    pub fn read_receipts_enabled(&self) -> Result<bool, SettingsError> {
        self.bool_setting(READ_RECEIPTS_ENABLED, true)
    }

    pub fn set_read_receipts_enabled(
        &self,
        enabled: bool,
        updated_at_ms: i64,
    ) -> Result<(), SettingsError> {
        self.set_bool_setting(READ_RECEIPTS_ENABLED, enabled, updated_at_ms)
    }

    /// Returns the local acknowledgement boundary for the Contacts navigation badge.
    pub fn new_contacts_acknowledged_at_ms(&self) -> Result<Option<i64>, SettingsError> {
        self.backend
            .connection()
            .query_row(
                SETTING_UPDATED_AT_SQL,
                [NEW_CONTACTS_ACKNOWLEDGED],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(|_| SettingsError::Query)
    }

    /// Marks every contact visible at `updated_at_ms` as acknowledged on this device.
    pub fn acknowledge_new_contacts(&self, updated_at_ms: i64) -> Result<(), SettingsError> {
        self.set_bool_setting(NEW_CONTACTS_ACKNOWLEDGED, true, updated_at_ms)
    }

    fn bool_setting(&self, key: &str, default: bool) -> Result<bool, SettingsError> {
        let value = self
            .backend
            .connection()
            .query_row(BOOL_SETTING_SQL, [key], |row| row.get::<_, i64>(0))
            .optional()
            .map_err(|_| SettingsError::Query)?
            .unwrap_or(i64::from(default));
        Ok(value != 0)
    }

    fn set_bool_setting(
        &self,
        key: &str,
        enabled: bool,
        updated_at_ms: i64,
    ) -> Result<(), SettingsError> {
        self.backend
            .connection()
            .execute(
                UPSERT_BOOL_SETTING_SQL,
                rusqlite::params![key, i64::from(enabled), updated_at_ms],
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

    #[test]
    fn read_receipt_setting_is_durable_for_the_connection() {
        let key = DatabaseKey::new([0x19; 32]);
        let store = SqlCipherSettingsStore::open_in_memory(&key).expect("settings store");
        assert!(store.read_receipts_enabled().expect("read default"));
        store.set_read_receipts_enabled(false, 42).expect("write setting");
        assert!(!store.read_receipts_enabled().expect("read updated"));
    }

    #[test]
    fn new_contacts_acknowledgement_is_durable_for_the_connection() {
        let key = DatabaseKey::new([0x18; 32]);
        let store = SqlCipherSettingsStore::open_in_memory(&key).expect("settings store");
        assert_eq!(store.new_contacts_acknowledged_at_ms().expect("read setting"), None);
        store.acknowledge_new_contacts(42).expect("write setting");
        assert_eq!(store.new_contacts_acknowledged_at_ms().expect("read setting"), Some(42));
    }
}
