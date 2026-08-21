use core::fmt;
use std::path::Path;

use rusqlite::OptionalExtension;
use torca_battery::{
    BackgroundSyncCadence, BatteryPreferences, RequestedBatteryMode, VisualActivityPolicy,
};
use torca_contacts::ContactId;
use torca_runtime_policy::{ContactAvailabilityMode, MeteredTransferPolicy};

const BATTERY_PREFERENCES_SQL: &str =
    include_str!("../sql/queries/runtime_battery_preferences.sql");
const BATTERY_PREFERENCES_UPDATE_SQL: &str =
    include_str!("../sql/commands/runtime_battery_preferences_update.sql");
const CONTACT_AVAILABILITY_SQL: &str = include_str!("../sql/queries/contact_availability.sql");
const CONTACT_AVAILABILITY_UPSERT_SQL: &str =
    include_str!("../sql/commands/contact_availability_upsert.sql");
#[cfg(test)]
const TEST_CONTACT_INSERT_SQL: &str = include_str!("../sql/commands/test_contact_insert.sql");

use crate::{DatabaseKey, SqlCipherBackend, SqlCipherStoreOpenError};

const BOOL_SETTING_SQL: &str = include_str!("../sql/queries/runtime_bool_setting.sql");
const SETTING_UPDATED_AT_SQL: &str = include_str!("../sql/queries/runtime_setting_updated_at.sql");
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

    pub fn contact_availability(
        &self,
        contact_id: ContactId,
    ) -> Result<ContactAvailabilityMode, SettingsError> {
        let id = contact_id.to_opaque().into_bytes();
        self.backend
            .connection()
            .query_row(CONTACT_AVAILABILITY_SQL, [id.as_slice()], |row| row.get::<_, String>(0))
            .optional()
            .map_err(|_| SettingsError::Query)?
            .map_or(Ok(ContactAvailabilityMode::Adaptive), |value| {
                Ok(ContactAvailabilityMode::from_wire(&value))
            })
    }

    pub fn set_contact_availability(
        &self,
        contact_id: ContactId,
        mode: ContactAvailabilityMode,
        updated_at_ms: i64,
    ) -> Result<(), SettingsError> {
        let id = contact_id.to_opaque().into_bytes();
        self.backend
            .connection()
            .execute(
                CONTACT_AVAILABILITY_UPSERT_SQL,
                rusqlite::params![id.as_slice(), mode.wire(), updated_at_ms],
            )
            .map_err(|_| SettingsError::Write)?;
        Ok(())
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

    pub fn battery_preferences(&self) -> Result<BatteryPreferences, SettingsError> {
        self.backend
            .connection()
            .query_row(BATTERY_PREFERENCES_SQL, [], |row| {
                Ok(BatteryPreferences {
                    mode: parse_mode(&row.get::<_, String>(0)?),
                    background_sync: parse_sync(&row.get::<_, String>(1)?),
                    allow_delayed_background_delivery: row.get::<_, i64>(2)? != 0,
                    metered_transfers: parse_metered(&row.get::<_, String>(3)?),
                    visual_activity: parse_visual(&row.get::<_, String>(4)?),
                })
            })
            .map_err(|_| SettingsError::Query)
    }

    pub fn set_battery_preferences(
        &self,
        preferences: BatteryPreferences,
        updated_at_ms: i64,
    ) -> Result<(), SettingsError> {
        self.backend
            .connection()
            .execute(
                BATTERY_PREFERENCES_UPDATE_SQL,
                rusqlite::params![
                    mode_value(preferences.mode),
                    sync_value(preferences.background_sync),
                    i64::from(preferences.allow_delayed_background_delivery),
                    metered_value(preferences.metered_transfers),
                    visual_value(preferences.visual_activity),
                    updated_at_ms,
                ],
            )
            .map_err(|_| SettingsError::Write)?;
        Ok(())
    }

    /// Returns the local acknowledgement boundary for the Contacts navigation badge.
    pub fn new_contacts_acknowledged_at_ms(&self) -> Result<Option<i64>, SettingsError> {
        self.backend
            .connection()
            .query_row(SETTING_UPDATED_AT_SQL, [NEW_CONTACTS_ACKNOWLEDGED], |row| {
                row.get::<_, i64>(0)
            })
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

fn parse_mode(value: &str) -> RequestedBatteryMode {
    match value {
        "always_available" => RequestedBatteryMode::AlwaysAvailable,
        "balanced" => RequestedBatteryMode::Balanced,
        "battery_saver" => RequestedBatteryMode::BatterySaver,
        _ => RequestedBatteryMode::Automatic,
    }
}

fn mode_value(value: RequestedBatteryMode) -> &'static str {
    match value {
        RequestedBatteryMode::Automatic => "automatic",
        RequestedBatteryMode::AlwaysAvailable => "always_available",
        RequestedBatteryMode::Balanced => "balanced",
        RequestedBatteryMode::BatterySaver => "battery_saver",
    }
}

fn parse_sync(_value: &str) -> BackgroundSyncCadence {
    // Old persisted cadence values are intentionally read as the BATTERY1
    // compatibility value. They cannot reactivate a periodic background wake.
    BackgroundSyncCadence::OnOpen
}

fn sync_value(_value: BackgroundSyncCadence) -> &'static str {
    "on_open"
}

fn parse_metered(value: &str) -> MeteredTransferPolicy {
    match value {
        "allow_all" => MeteredTransferPolicy::AllowAll,
        "pause_all" => MeteredTransferPolicy::PauseAll,
        _ => MeteredTransferPolicy::PauseLarge,
    }
}

fn metered_value(value: MeteredTransferPolicy) -> &'static str {
    match value {
        MeteredTransferPolicy::AllowAll => "allow_all",
        MeteredTransferPolicy::PauseLarge => "pause_large",
        MeteredTransferPolicy::PauseAll => "pause_all",
    }
}

fn parse_visual(value: &str) -> VisualActivityPolicy {
    match value {
        "full" => VisualActivityPolicy::Full,
        "focused_only" => VisualActivityPolicy::FocusedOnly,
        "static" => VisualActivityPolicy::Static,
        _ => VisualActivityPolicy::FollowSystem,
    }
}

fn visual_value(value: VisualActivityPolicy) -> &'static str {
    match value {
        VisualActivityPolicy::Full => "full",
        VisualActivityPolicy::FocusedOnly => "focused_only",
        VisualActivityPolicy::Static => "static",
        VisualActivityPolicy::FollowSystem => "follow_system",
    }
}

#[cfg(test)]
mod tests {
    use super::SqlCipherSettingsStore;
    use crate::DatabaseKey;
    use torca_contacts::ContactId;
    use torca_foundation::OpaqueId;
    use torca_runtime_policy::{ContactAvailabilityMode, MeteredTransferPolicy};

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

    #[test]
    fn battery_preferences_are_durable_for_the_connection() {
        use torca_battery::{
            BackgroundSyncCadence, BatteryPreferences, RequestedBatteryMode, VisualActivityPolicy,
        };
        let key = DatabaseKey::new([0x21; 32]);
        let store = SqlCipherSettingsStore::open_in_memory(&key).expect("settings store");
        let preferences = BatteryPreferences {
            mode: RequestedBatteryMode::BatterySaver,
            background_sync: BackgroundSyncCadence::OnOpen,
            allow_delayed_background_delivery: true,
            metered_transfers: MeteredTransferPolicy::PauseAll,
            visual_activity: VisualActivityPolicy::FocusedOnly,
        };
        store.set_battery_preferences(preferences, 42).expect("write battery settings");
        assert_eq!(store.battery_preferences().expect("read battery settings"), preferences);
    }

    #[test]
    fn contact_availability_defaults_to_adaptive_and_persists_instant() {
        let key = DatabaseKey::new([0x22; 32]);
        let store = SqlCipherSettingsStore::open_in_memory(&key).expect("settings store");
        let contact = ContactId::from_opaque(OpaqueId::from_u128(77));
        assert_eq!(
            store.contact_availability(contact).expect("default availability"),
            ContactAvailabilityMode::Adaptive
        );
        let id = contact.to_opaque().into_bytes();
        store
            .backend
            .connection()
            .execute(
                super::TEST_CONTACT_INSERT_SQL,
                rusqlite::params![
                    id.as_slice(),
                    [1_u8; 16].as_slice(),
                    [2_u8; 16].as_slice(),
                    [3_u8; 32].as_slice(),
                    [4_u8; 16].as_slice()
                ],
            )
            .expect("contact fixture");
        store
            .set_contact_availability(contact, ContactAvailabilityMode::Instant, 42)
            .expect("persist instant");
        assert_eq!(
            store.contact_availability(contact).expect("stored availability"),
            ContactAvailabilityMode::Instant
        );
    }
}
