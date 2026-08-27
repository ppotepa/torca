use std::path::Path;

use rusqlite::params;

use crate::{DatabaseKey, MigrationError, SqlCipherBackend, StorageBackendError, StorageKernel};

const APPEND_SQL: &str = include_str!("../sql/commands/notification_append.sql");
const ACKNOWLEDGE_THROUGH_SQL: &str =
    include_str!("../sql/commands/notification_acknowledge_through.sql");
const CURSOR_BY_EVENT_SQL: &str = include_str!("../sql/queries/notification_cursor_by_event.sql");
const READ_AFTER_SQL: &str = include_str!("../sql/queries/notification_read_after.sql");
const MAX_CURSOR_SQL: &str = include_str!("../sql/queries/notification_max_cursor.sql");

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NotificationStorageError {
    Backend(StorageBackendError),
    Migration(MigrationError),
}

impl core::fmt::Display for NotificationStorageError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for NotificationStorageError {}
impl From<StorageBackendError> for NotificationStorageError {
    fn from(e: StorageBackendError) -> Self {
        Self::Backend(e)
    }
}
impl From<MigrationError> for NotificationStorageError {
    fn from(e: MigrationError) -> Self {
        Self::Migration(e)
    }
}

/// Durable cursor-addressed notification outbox. Payloads are opaque JSON so
/// storage does not depend on the bridge contract crate.
pub struct SqlCipherNotificationStore {
    backend: SqlCipherBackend,
}

impl SqlCipherNotificationStore {
    pub fn open(
        path: impl AsRef<Path>,
        key: &DatabaseKey,
    ) -> Result<Self, NotificationStorageError> {
        Self::bootstrap(SqlCipherBackend::open(path, key)?)
    }
    pub fn open_in_memory(key: &DatabaseKey) -> Result<Self, NotificationStorageError> {
        Self::bootstrap(SqlCipherBackend::open_in_memory(key)?)
    }
    fn bootstrap(backend: SqlCipherBackend) -> Result<Self, NotificationStorageError> {
        let mut kernel = StorageKernel::new(backend);
        kernel.bootstrap()?;
        Ok(Self { backend: kernel.into_backend() })
    }
    pub fn append(
        &mut self,
        event_id: &str,
        payload: &str,
        created_at_ms: i64,
    ) -> Result<u64, NotificationStorageError> {
        self.backend
            .connection()
            .execute(APPEND_SQL, params![event_id, payload, created_at_ms])
            .map_err(sql_error)?;
        self.backend
            .connection()
            .query_row(CURSOR_BY_EVENT_SQL, params![event_id], |row| row.get::<_, i64>(0))
            .map(|v| u64::try_from(v.max(0)).unwrap_or(0))
            .map_err(sql_error)
    }
    pub fn read_after(
        &self,
        after_cursor: u64,
        limit: usize,
    ) -> Result<Vec<(u64, String)>, NotificationStorageError> {
        let mut stmt = self.backend.connection().prepare(READ_AFTER_SQL).map_err(sql_error)?;
        let rows = stmt
            .query_map(
                params![
                    i64::try_from(after_cursor).unwrap_or(i64::MAX),
                    i64::try_from(limit).unwrap_or(i64::MAX)
                ],
                |row| {
                    Ok((
                        u64::try_from(row.get::<_, i64>(0)?.max(0)).unwrap_or(0),
                        row.get::<_, String>(1)?,
                    ))
                },
            )
            .map_err(sql_error)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(sql_error)
    }
    pub fn max_cursor(&self) -> Result<u64, NotificationStorageError> {
        self.backend
            .connection()
            .query_row(MAX_CURSOR_SQL, [], |row| row.get::<_, i64>(0))
            .map(|v| u64::try_from(v.max(0)).unwrap_or(0))
            .map_err(sql_error)
    }
    pub fn acknowledge_through(&mut self, cursor: u64) -> Result<(), NotificationStorageError> {
        self.backend
            .connection()
            .execute(ACKNOWLEDGE_THROUGH_SQL, params![i64::try_from(cursor).unwrap_or(i64::MAX)])
            .map(|_| ())
            .map_err(sql_error)
    }
}

fn sql_error(error: rusqlite::Error) -> NotificationStorageError {
    NotificationStorageError::Backend(StorageBackendError(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_is_durable_and_event_ids_are_idempotent() {
        let key = DatabaseKey::new([7; 32]);
        let mut store = SqlCipherNotificationStore::open_in_memory(&key).unwrap();
        let first = store.append("event-1", r#"{"kind":"contact_added"}"#, 10).unwrap();
        let same = store.append("event-1", r#"{"kind":"contact_added"}"#, 11).unwrap();
        assert_eq!(first, same);
        let second = store.append("event-2", r#"{"kind":"message_received"}"#, 12).unwrap();
        assert!(second > first);
        assert_eq!(store.read_after(first, 10).unwrap().len(), 1);
        assert_eq!(store.max_cursor().unwrap(), second);
        store.acknowledge_through(first).unwrap();
        assert_eq!(store.read_after(0, 10).unwrap().len(), 1);
    }
}
