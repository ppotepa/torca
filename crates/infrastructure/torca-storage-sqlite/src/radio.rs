use std::path::Path;

use rusqlite::params;
use torca_contacts::ContactId;
use torca_foundation::{OpaqueId, Timestamp};
use torca_radio::{RadioEventActor, RadioPreference, RadioTimelineEvent, RadioTimelineEventKind};
use torca_radio_coordinator::{RadioApplicationError, RadioStateStore, RadioTimelineRecord};

use crate::{DatabaseKey, MigrationError, SqlCipherBackend, StorageBackendError, StorageKernel};

const LOAD_PREFERENCES_SQL: &str = include_str!("../sql/queries/radio_preferences.sql");
const UPSERT_PREFERENCE_SQL: &str = include_str!("../sql/commands/radio_preference_upsert.sql");
const INSERT_EVENT_SQL: &str = include_str!("../sql/commands/conversation_event_insert.sql");
const EVENT_EXISTS_SQL: &str = include_str!("../sql/queries/conversation_event_exists.sql");
const LOAD_RECENT_EVENTS_SQL: &str = include_str!("../sql/queries/radio_recent_events.sql");
#[cfg(test)]
const INSERT_TEST_CONTACT_SQL: &str = include_str!("../sql/commands/test_radio_contact_insert.sql");
#[cfg(test)]
const INSERT_TEST_CONVERSATION_SQL: &str =
    include_str!("../sql/commands/test_radio_conversation_insert.sql");

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RadioStorageOpenError {
    Backend(StorageBackendError),
    Migration(MigrationError),
}

impl core::fmt::Display for RadioStorageOpenError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for RadioStorageOpenError {}

impl From<StorageBackendError> for RadioStorageOpenError {
    fn from(value: StorageBackendError) -> Self {
        Self::Backend(value)
    }
}

impl From<MigrationError> for RadioStorageOpenError {
    fn from(value: MigrationError) -> Self {
        Self::Migration(value)
    }
}

/// SQLCipher implementation of the atomic Radio Mode persistence boundary.
pub struct SqlCipherRadioStore {
    backend: SqlCipherBackend,
}

impl SqlCipherRadioStore {
    pub fn open(path: impl AsRef<Path>, key: &DatabaseKey) -> Result<Self, RadioStorageOpenError> {
        let backend = SqlCipherBackend::open(path, key)?;
        Self::bootstrap(backend)
    }

    pub fn open_in_memory(key: &DatabaseKey) -> Result<Self, RadioStorageOpenError> {
        let backend = SqlCipherBackend::open_in_memory(key)?;
        Self::bootstrap(backend)
    }

    fn bootstrap(backend: SqlCipherBackend) -> Result<Self, RadioStorageOpenError> {
        let mut kernel = StorageKernel::new(backend);
        kernel.bootstrap()?;
        Ok(Self { backend: kernel.into_backend() })
    }

    #[cfg(test)]
    fn seed_relationship(
        &self,
        contact_id: ContactId,
        conversation_id: OpaqueId,
    ) -> Result<(), rusqlite::Error> {
        let contact = contact_id.to_opaque().into_bytes();
        let conversation = conversation_id.into_bytes();
        self.backend.connection().execute(
            INSERT_TEST_CONTACT_SQL,
            params![
                contact.as_slice(),
                contact.as_slice(),
                [3_u8; 16].as_slice(),
                [4_u8; 32].as_slice(),
                contact.as_slice()
            ],
        )?;
        self.backend.connection().execute(
            INSERT_TEST_CONVERSATION_SQL,
            params![conversation.as_slice(), contact.as_slice()],
        )?;
        Ok(())
    }
}

impl RadioStateStore for SqlCipherRadioStore {
    fn load_preferences(&self) -> Result<Vec<RadioPreference>, RadioApplicationError> {
        let mut statement = self
            .backend
            .connection()
            .prepare(LOAD_PREFERENCES_SQL)
            .map_err(|_| RadioApplicationError::Persistence)?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })
            .map_err(|_| RadioApplicationError::Persistence)?;
        rows.map(|row| {
            let (contact, enabled, revision, changed_at_ms) =
                row.map_err(|_| RadioApplicationError::Persistence)?;
            let contact: [u8; 16] =
                contact.try_into().map_err(|_| RadioApplicationError::Persistence)?;
            let revision =
                u64::try_from(revision).map_err(|_| RadioApplicationError::Persistence)?;
            let changed_at = Timestamp::from_unix_millis(changed_at_ms)
                .map_err(|_| RadioApplicationError::Persistence)?;
            Ok(RadioPreference {
                contact_id: ContactId::from_opaque(OpaqueId::from_bytes(contact)),
                enabled: enabled != 0,
                revision,
                changed_at,
            })
        })
        .collect()
    }

    fn load_recent_events(
        &self,
        limit: usize,
    ) -> Result<Vec<RadioTimelineRecord>, RadioApplicationError> {
        let limit = i64::try_from(limit).map_err(|_| RadioApplicationError::Persistence)?;
        let mut statement = self
            .backend
            .connection()
            .prepare(LOAD_RECENT_EVENTS_SQL)
            .map_err(|_| RadioApplicationError::Persistence)?;
        let rows = statement
            .query_map([limit], |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, Vec<u8>>(4)?,
                    row.get::<_, i64>(5)?,
                ))
            })
            .map_err(|_| RadioApplicationError::Persistence)?;
        rows.map(|row| {
            let (event_id, contact_id, kind, actor, correlation_id, occurred_at_ms) =
                row.map_err(|_| RadioApplicationError::Persistence)?;
            Ok(RadioTimelineRecord {
                event_id: decode_id(event_id)?,
                contact_id: ContactId::from_opaque(decode_id(contact_id)?),
                correlation_id: decode_id(correlation_id)?,
                event: RadioTimelineEvent {
                    kind: decode_kind(kind)?,
                    actor: decode_actor(actor)?,
                    occurred_at: Timestamp::from_unix_millis(occurred_at_ms)
                        .map_err(|_| RadioApplicationError::Persistence)?,
                },
            })
        })
        .collect()
    }

    fn commit(
        &mut self,
        preferences: &[RadioPreference],
        events: &[RadioTimelineRecord],
    ) -> Result<(), RadioApplicationError> {
        let transaction = self
            .backend
            .connection()
            .unchecked_transaction()
            .map_err(|_| RadioApplicationError::Persistence)?;

        // Disable the previous relationship before enabling its replacement,
        // so the partial unique index remains valid throughout the transaction.
        for preference in preferences
            .iter()
            .filter(|value| !value.enabled)
            .chain(preferences.iter().filter(|value| value.enabled))
        {
            let contact = preference.contact_id.to_opaque().into_bytes();
            let revision = i64::try_from(preference.revision)
                .map_err(|_| RadioApplicationError::Persistence)?;
            transaction
                .execute(
                    UPSERT_PREFERENCE_SQL,
                    params![
                        contact.as_slice(),
                        i64::from(preference.enabled),
                        revision,
                        preference.changed_at.to_unix_millis(),
                    ],
                )
                .map_err(|_| RadioApplicationError::Persistence)?;
        }

        for record in events {
            let event_id = record.event_id.into_bytes();
            let contact = record.contact_id.to_opaque().into_bytes();
            let correlation = record.correlation_id.into_bytes();
            let inserted = transaction
                .execute(
                    INSERT_EVENT_SQL,
                    params![
                        event_id.as_slice(),
                        contact.as_slice(),
                        encode_kind(record.event.kind),
                        encode_actor(record.event.actor),
                        correlation.as_slice(),
                        record.event.occurred_at.to_unix_millis(),
                    ],
                )
                .map_err(|_| RadioApplicationError::Persistence)?;
            if inserted == 0 {
                // INSERT OR IGNORE is idempotent for a known event id. An
                // absent relationship is distinguished below by checking the
                // conversation only when the event did not already exist.
                let exists = transaction
                    .query_row(EVENT_EXISTS_SQL, [event_id.as_slice()], |row| row.get::<_, i64>(0))
                    .map_err(|_| RadioApplicationError::Persistence)?;
                if exists == 0 {
                    return Err(RadioApplicationError::Persistence);
                }
            }
        }

        transaction.commit().map_err(|_| RadioApplicationError::Persistence)
    }
}

const fn encode_kind(value: RadioTimelineEventKind) -> i64 {
    match value {
        RadioTimelineEventKind::Enabled => 1,
        RadioTimelineEventKind::Disabled => 2,
        RadioTimelineEventKind::Ready => 3,
        RadioTimelineEventKind::Interrupted => 4,
        RadioTimelineEventKind::Restored => 5,
    }
}

const fn encode_actor(value: RadioEventActor) -> i64 {
    match value {
        RadioEventActor::Local => 1,
        RadioEventActor::Remote => 2,
        RadioEventActor::System => 3,
    }
}

fn decode_id(value: Vec<u8>) -> Result<OpaqueId, RadioApplicationError> {
    let value: [u8; 16] = value.try_into().map_err(|_| RadioApplicationError::Persistence)?;
    Ok(OpaqueId::from_bytes(value))
}

const fn decode_kind(value: i64) -> Result<RadioTimelineEventKind, RadioApplicationError> {
    match value {
        1 => Ok(RadioTimelineEventKind::Enabled),
        2 => Ok(RadioTimelineEventKind::Disabled),
        3 => Ok(RadioTimelineEventKind::Ready),
        4 => Ok(RadioTimelineEventKind::Interrupted),
        5 => Ok(RadioTimelineEventKind::Restored),
        _ => Err(RadioApplicationError::Persistence),
    }
}

const fn decode_actor(value: i64) -> Result<RadioEventActor, RadioApplicationError> {
    match value {
        1 => Ok(RadioEventActor::Local),
        2 => Ok(RadioEventActor::Remote),
        3 => Ok(RadioEventActor::System),
        _ => Err(RadioApplicationError::Persistence),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use torca_radio::{RadioEventActor, RadioTimelineEvent, RadioTimelineEventKind};

    fn at(value: i64) -> Timestamp {
        Timestamp::from_unix_millis(value).expect("timestamp")
    }

    #[test]
    fn preference_and_timeline_event_commit_atomically() {
        let key = DatabaseKey::new([0x41; 32]);
        let mut store = SqlCipherRadioStore::open_in_memory(&key).expect("store");
        let contact = ContactId::from_u128(1);
        store.seed_relationship(contact, OpaqueId::from_u128(2)).expect("relationship");
        let preference =
            RadioPreference { contact_id: contact, enabled: true, revision: 1, changed_at: at(10) };
        let event = RadioTimelineRecord {
            event_id: OpaqueId::from_u128(3),
            contact_id: contact,
            correlation_id: OpaqueId::from_u128(4),
            event: RadioTimelineEvent {
                kind: RadioTimelineEventKind::Enabled,
                actor: RadioEventActor::Local,
                occurred_at: at(10),
            },
        };

        store.commit(&[preference], &[event]).expect("commit");
        assert_eq!(store.load_preferences().expect("load"), vec![preference]);
        assert_eq!(store.load_recent_events(10).expect("events"), vec![event]);
        store.commit(&[], &[event]).expect("idempotent event retry");
    }

    #[test]
    fn unique_index_rejects_two_active_contacts() {
        let key = DatabaseKey::new([0x42; 32]);
        let mut store = SqlCipherRadioStore::open_in_memory(&key).expect("store");
        let first = ContactId::from_u128(1);
        let second = ContactId::from_u128(2);
        store.seed_relationship(first, OpaqueId::from_u128(11)).expect("first");
        store.seed_relationship(second, OpaqueId::from_u128(12)).expect("second");
        let preferences = [first, second].map(|contact_id| RadioPreference {
            contact_id,
            enabled: true,
            revision: 1,
            changed_at: at(10),
        });
        assert_eq!(store.commit(&preferences, &[]), Err(RadioApplicationError::Persistence));
        assert!(store.load_preferences().expect("rolled back").is_empty());
    }
}
