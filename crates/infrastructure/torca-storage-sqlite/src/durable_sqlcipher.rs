use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{ErrorCode, OptionalExtension, params};
use torca_conversations::ConversationId;
use torca_foundation::{CommandId, OpaqueId, Timestamp};
use torca_messaging::{
    Message, MessageBody, MessageDirection, MessageId, MessageStatus, ReplyReference,
};

use crate::{
    DatabaseKey, DurableDeliveryError, DurableDeliveryStore, MigrationError, OutboxRecord,
    OutboxState, SqlCipherBackend, StorageBackend, StorageBackendError, StorageKernel,
    messaging_sql,
};

/// Failure while opening and migrating the concrete durable-delivery store.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SqlCipherDurableStoreOpenError {
    Backend(StorageBackendError),
    Migration(MigrationError),
}

impl core::fmt::Display for SqlCipherDurableStoreOpenError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for SqlCipherDurableStoreOpenError {}
impl From<StorageBackendError> for SqlCipherDurableStoreOpenError {
    fn from(value: StorageBackendError) -> Self {
        Self::Backend(value)
    }
}
impl From<MigrationError> for SqlCipherDurableStoreOpenError {
    fn from(value: MigrationError) -> Self {
        Self::Migration(value)
    }
}

/// SQLCipher implementation of the transactional outbox and inbound deduplication port.
pub struct SqlCipherDurableStore {
    backend: SqlCipherBackend,
}

impl SqlCipherDurableStore {
    /// Opens, keys and migrates a durable-delivery database.
    pub fn open(
        path: impl AsRef<Path>,
        key: &DatabaseKey,
    ) -> Result<Self, SqlCipherDurableStoreOpenError> {
        let backend = SqlCipherBackend::open(path, key)?;
        Self::bootstrap(backend)
    }

    /// Opens an encrypted in-memory store for integration tests.
    pub fn open_in_memory(key: &DatabaseKey) -> Result<Self, SqlCipherDurableStoreOpenError> {
        let backend = SqlCipherBackend::open_in_memory(key)?;
        Self::bootstrap(backend)
    }

    fn bootstrap(backend: SqlCipherBackend) -> Result<Self, SqlCipherDurableStoreOpenError> {
        let mut kernel = StorageKernel::new(backend);
        kernel.bootstrap()?;
        Ok(Self { backend: kernel.into_backend() })
    }

    fn load_message(&self, id: MessageId) -> Result<Message, DurableDeliveryError> {
        let bytes = id.to_opaque().into_bytes();
        let row = self
            .backend
            .connection()
            .query_row(messaging_sql::SELECT_MESSAGE.sql, params![bytes.as_slice()], |row| {
                Ok(MessageRow {
                    conversation_id: row.get(0)?,
                    direction: row.get(1)?,
                    status: row.get(2)?,
                    body: row.get(3)?,
                    reply_to: row.get(4)?,
                    created_at_ms: row.get(5)?,
                    updated_at_ms: row.get(6)?,
                })
            })
            .optional()
            .map_err(storage_error)?;
        row.ok_or(DurableDeliveryError::NotFound)?.into_message(id)
    }

    fn outbox_exists(&self, id: MessageId) -> Result<bool, DurableDeliveryError> {
        let bytes = id.to_opaque().into_bytes();
        self.backend
            .connection()
            .query_row(messaging_sql::EXISTS.sql, params![bytes.as_slice()], |row| {
                row.get::<_, bool>(0)
            })
            .map_err(storage_error)
    }

    fn transition_result(&self, changed: usize, id: MessageId) -> Result<(), DurableDeliveryError> {
        if changed == 1 {
            Ok(())
        } else if self.outbox_exists(id)? {
            Err(DurableDeliveryError::InvalidState)
        } else {
            Err(DurableDeliveryError::NotFound)
        }
    }
}

impl DurableDeliveryStore for SqlCipherDurableStore {
    fn queue_outbound(
        &mut self,
        message: Message,
        command_id: CommandId,
        next_attempt_at: Timestamp,
    ) -> Result<(), DurableDeliveryError> {
        if message.direction() != MessageDirection::Outbound
            || message.status() != MessageStatus::Queued
        {
            return Err(DurableDeliveryError::InvalidState);
        }

        let message_id = message.id().to_opaque().into_bytes();
        let conversation_id = message.conversation_id().to_opaque().into_bytes();
        let command_id = command_id.to_opaque().into_bytes();
        let reply_id = message.reply_to().map(|reply| reply.message_id.to_opaque().into_bytes());
        let reply = reply_id.as_ref().map(<[u8; 16]>::as_slice);

        self.backend.begin().map_err(backend_error)?;
        let result = (|| {
            self.backend.connection().execute(
                messaging_sql::INSERT_MESSAGE.sql,
                params![
                    message_id.as_slice(),
                    conversation_id.as_slice(),
                    encode_direction(message.direction()),
                    encode_status(message.status()),
                    message.body().as_str(),
                    reply,
                    message.created_at().to_unix_millis(),
                    message.updated_at().to_unix_millis(),
                ],
            )?;
            self.backend.connection().execute(
                messaging_sql::INSERT_OUTBOX.sql,
                params![
                    message_id.as_slice(),
                    command_id.as_slice(),
                    next_attempt_at.to_unix_millis(),
                ],
            )?;
            Ok::<(), rusqlite::Error>(())
        })();

        match result {
            Ok(()) => self.backend.commit().map_err(backend_error),
            Err(error) => {
                let _ = self.backend.rollback();
                if error.sqlite_error_code() == Some(ErrorCode::ConstraintViolation) {
                    Err(DurableDeliveryError::DuplicateMessage)
                } else {
                    Err(storage_error(error))
                }
            }
        }
    }

    fn claim_due(
        &mut self,
        now: Timestamp,
        limit: usize,
    ) -> Result<Vec<OutboxRecord>, DurableDeliveryError> {
        let limit = i64::try_from(limit)
            .map_err(|_| DurableDeliveryError::Storage("claim limit is too large".into()))?;
        let mut statement = self
            .backend
            .connection()
            .prepare(messaging_sql::CLAIM_DUE.sql)
            .map_err(storage_error)?;
        let rows = statement
            .query_map(params![now.to_unix_millis(), limit], |row| {
                Ok(ClaimRow {
                    message_id: row.get(0)?,
                    command_id: row.get(1)?,
                    attempts: row.get(2)?,
                    next_attempt_at_ms: row.get(3)?,
                    claimed_at_ms: row.get(4)?,
                })
            })
            .map_err(storage_error)?;
        let claims: Vec<ClaimRow> = rows.collect::<Result<_, _>>().map_err(storage_error)?;
        drop(statement);

        claims
            .into_iter()
            .map(|claim| {
                let message_id = MessageId::from_opaque(OpaqueId::from_bytes(fixed_16(
                    claim.message_id,
                    "message_id",
                )?));
                let command_id = CommandId::from_opaque(OpaqueId::from_bytes(fixed_16(
                    claim.command_id,
                    "command_id",
                )?));
                let attempts = u32::try_from(claim.attempts).map_err(|_| {
                    DurableDeliveryError::Storage("attempt count is invalid".into())
                })?;
                let next_attempt_at = timestamp(claim.next_attempt_at_ms, "next_attempt_at")?;
                let claimed_at =
                    claim.claimed_at_ms.map(|value| timestamp(value, "claimed_at")).transpose()?;
                Ok(OutboxRecord {
                    message: self.load_message(message_id)?,
                    command_id,
                    attempts,
                    next_attempt_at,
                    claimed_at,
                    state: OutboxState::Claimed,
                })
            })
            .collect()
    }

    fn reschedule(
        &mut self,
        message_id: MessageId,
        attempts: u32,
        next_attempt_at: Timestamp,
    ) -> Result<(), DurableDeliveryError> {
        let id = message_id.to_opaque().into_bytes();
        let changed = self
            .backend
            .connection()
            .execute(
                messaging_sql::RESCHEDULE.sql,
                params![id.as_slice(), i64::from(attempts), next_attempt_at.to_unix_millis()],
            )
            .map_err(storage_error)?;
        self.transition_result(changed, message_id)
    }

    fn complete(&mut self, message_id: MessageId) -> Result<(), DurableDeliveryError> {
        let id = message_id.to_opaque().into_bytes();
        let changed = self
            .backend
            .connection()
            .execute(messaging_sql::COMPLETE.sql, params![id.as_slice()])
            .map_err(storage_error)?;
        self.transition_result(changed, message_id)
    }

    fn dead_letter(&mut self, message_id: MessageId) -> Result<(), DurableDeliveryError> {
        let id = message_id.to_opaque().into_bytes();
        let changed = self
            .backend
            .connection()
            .execute(messaging_sql::DEAD_LETTER.sql, params![id.as_slice()])
            .map_err(storage_error)?;
        self.transition_result(changed, message_id)
    }

    fn recover_stale_claims(
        &mut self,
        claimed_before: Timestamp,
    ) -> Result<usize, DurableDeliveryError> {
        self.backend
            .connection()
            .execute(messaging_sql::RECOVER_STALE.sql, params![claimed_before.to_unix_millis()])
            .map_err(storage_error)
    }

    fn record_inbound(&mut self, envelope_id: OpaqueId) -> Result<bool, DurableDeliveryError> {
        let id = envelope_id.into_bytes();
        let accepted_at = system_timestamp()?;
        let changed = self
            .backend
            .connection()
            .execute(
                messaging_sql::INSERT_INBOUND_DEDUP.sql,
                params![id.as_slice(), accepted_at.to_unix_millis()],
            )
            .map_err(storage_error)?;
        Ok(changed == 1)
    }
}

struct ClaimRow {
    message_id: Vec<u8>,
    command_id: Vec<u8>,
    attempts: i64,
    next_attempt_at_ms: i64,
    claimed_at_ms: Option<i64>,
}

struct MessageRow {
    conversation_id: Vec<u8>,
    direction: i64,
    status: i64,
    body: String,
    reply_to: Option<Vec<u8>>,
    created_at_ms: i64,
    updated_at_ms: i64,
}

impl MessageRow {
    fn into_message(self, id: MessageId) -> Result<Message, DurableDeliveryError> {
        if self.direction != encode_direction(MessageDirection::Outbound) {
            return Err(DurableDeliveryError::Storage(
                "outbox references a non-outbound message".into(),
            ));
        }
        let status = match self.status {
            value if value == encode_status(MessageStatus::Queued) => MessageStatus::Queued,
            value if value == encode_status(MessageStatus::Sending) => MessageStatus::Sending,
            _ => {
                return Err(DurableDeliveryError::Storage(
                    "claimed outbox references a non-sendable message".into(),
                ));
            }
        };
        let conversation_id = ConversationId::from_opaque(OpaqueId::from_bytes(fixed_16(
            self.conversation_id,
            "conversation_id",
        )?));
        let body = MessageBody::new(self.body)
            .map_err(|_| DurableDeliveryError::Storage("stored message body is invalid".into()))?;
        let reply_to = self
            .reply_to
            .map(|value| {
                fixed_16(value, "reply_to_message_id").map(|bytes| ReplyReference {
                    message_id: MessageId::from_opaque(OpaqueId::from_bytes(bytes)),
                })
            })
            .transpose()?;
        let created_at = timestamp(self.created_at_ms, "created_at")?;
        let updated_at = timestamp(self.updated_at_ms, "updated_at")?;
        Message::from_persisted(
            id,
            conversation_id,
            body,
            reply_to,
            MessageDirection::Outbound,
            status,
            created_at,
            updated_at,
            None,
            None,
            None,
            Vec::new(),
        )
        .map_err(|_| DurableDeliveryError::Storage("stored message state is invalid".into()))
    }
}

const fn encode_direction(value: MessageDirection) -> i64 {
    match value {
        MessageDirection::Outbound => 0,
        MessageDirection::Inbound => 1,
    }
}
const fn encode_status(value: MessageStatus) -> i64 {
    match value {
        MessageStatus::Queued => 0,
        MessageStatus::Sending => 1,
        MessageStatus::Sent => 2,
        MessageStatus::Delivered => 3,
        MessageStatus::Read => 4,
        MessageStatus::Failed => 5,
        MessageStatus::Cancelled => 6,
    }
}

fn fixed_16(value: Vec<u8>, field: &str) -> Result<[u8; 16], DurableDeliveryError> {
    value
        .try_into()
        .map_err(|_| DurableDeliveryError::Storage(format!("{field} must contain 16 bytes")))
}
fn timestamp(value: i64, field: &str) -> Result<Timestamp, DurableDeliveryError> {
    Timestamp::from_unix_millis(value).map_err(|_| {
        DurableDeliveryError::Storage(format!("{field} is outside the supported range"))
    })
}
fn system_timestamp() -> Result<Timestamp, DurableDeliveryError> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| DurableDeliveryError::Storage("system clock is before Unix epoch".into()))?;
    let millis = i64::try_from(duration.as_millis()).map_err(|_| {
        DurableDeliveryError::Storage("system clock exceeds timestamp range".into())
    })?;
    timestamp(millis, "system time")
}
fn backend_error(error: StorageBackendError) -> DurableDeliveryError {
    DurableDeliveryError::Storage(error.0)
}
fn storage_error(error: rusqlite::Error) -> DurableDeliveryError {
    let code = error
        .sqlite_error_code()
        .map_or_else(|| "unknown".to_owned(), |value| format!("{value:?}"));
    DurableDeliveryError::Storage(format!("SQLite durable operation failed ({code})"))
}

#[cfg(test)]
mod tests {
    use torca_conversations::ConversationId;
    use torca_foundation::{CommandId, OpaqueId, Timestamp};
    use torca_messaging::{Message, MessageBody, MessageId};

    use crate::{DatabaseKey, DurableDeliveryStore, SqlCipherDurableStore};

    #[test]
    fn outbox_claim_reschedule_and_complete_round_trip() {
        let key = DatabaseKey::new([0x55; 32]);
        let mut store = SqlCipherDurableStore::open_in_memory(&key).expect("open store");
        let message = Message::outbound(
            MessageId::from_u128(1),
            ConversationId::from_u128(2),
            MessageBody::new("hello").expect("body"),
            None,
            Timestamp::UNIX_EPOCH,
        );
        store
            .queue_outbound(message, CommandId::from_u128(3), Timestamp::UNIX_EPOCH)
            .expect("queue");
        let claimed = store.claim_due(Timestamp::UNIX_EPOCH, 10).expect("claim");
        assert_eq!(claimed.len(), 1);
        let next = Timestamp::from_unix_millis(1).expect("timestamp");
        store.reschedule(MessageId::from_u128(1), 1, next).expect("reschedule");
        assert!(store.claim_due(Timestamp::UNIX_EPOCH, 10).expect("early claim").is_empty());
        assert_eq!(store.claim_due(next, 10).expect("claim again").len(), 1);
        store.complete(MessageId::from_u128(1)).expect("complete");
        assert!(store.claim_due(next, 10).expect("completed claim").is_empty());
    }

    #[test]
    fn inbound_deduplication_is_persistent_for_the_store() {
        let key = DatabaseKey::new([0x66; 32]);
        let mut store = SqlCipherDurableStore::open_in_memory(&key).expect("open store");
        let envelope = OpaqueId::from_u128(9);
        assert!(store.record_inbound(envelope).expect("first"));
        assert!(!store.record_inbound(envelope).expect("duplicate"));
    }
}
