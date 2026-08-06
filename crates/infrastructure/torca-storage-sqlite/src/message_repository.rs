use std::path::Path;

use rusqlite::{OptionalExtension, params};
use torca_conversations::ConversationId;
use torca_foundation::{OpaqueId, Timestamp};
use torca_messaging::{
    DeliveryAttempt, Message, MessageBody, MessageDirection, MessageError, MessageId,
    MessageRepository, MessageStatus, ReplyReference,
};

use crate::{
    DatabaseKey, MigrationError, SqlCipherBackend, StorageBackendError, StorageKernel,
    messaging_sql,
};

/// Failure while opening the concrete message repository.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SqlCipherMessageStoreOpenError {
    Backend(StorageBackendError),
    Migration(MigrationError),
}
impl core::fmt::Display for SqlCipherMessageStoreOpenError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for SqlCipherMessageStoreOpenError {}
impl From<StorageBackendError> for SqlCipherMessageStoreOpenError {
    fn from(value: StorageBackendError) -> Self {
        Self::Backend(value)
    }
}
impl From<MigrationError> for SqlCipherMessageStoreOpenError {
    fn from(value: MigrationError) -> Self {
        Self::Migration(value)
    }
}

/// SQLCipher-backed domain message repository.
pub struct SqlCipherMessageStore {
    backend: SqlCipherBackend,
}

impl SqlCipherMessageStore {
    pub fn open(
        path: impl AsRef<Path>,
        key: &DatabaseKey,
    ) -> Result<Self, SqlCipherMessageStoreOpenError> {
        let backend = SqlCipherBackend::open(path, key)?;
        Self::bootstrap(backend)
    }

    pub fn open_in_memory(key: &DatabaseKey) -> Result<Self, SqlCipherMessageStoreOpenError> {
        let backend = SqlCipherBackend::open_in_memory(key)?;
        Self::bootstrap(backend)
    }

    fn bootstrap(backend: SqlCipherBackend) -> Result<Self, SqlCipherMessageStoreOpenError> {
        let mut kernel = StorageKernel::new(backend);
        kernel.bootstrap()?;
        Ok(Self { backend: kernel.into_backend() })
    }
}

impl MessageRepository for SqlCipherMessageStore {
    fn insert(&mut self, message: Message) -> Result<(), MessageError> {
        if self.get(message.id())?.is_some() {
            return Err(MessageError::AlreadyExists);
        }
        execute_message(&self.backend, messaging_sql::INSERT_DOMAIN_MESSAGE.sql, &message)?;
        Ok(())
    }

    fn get(&self, id: MessageId) -> Result<Option<Message>, MessageError> {
        let id_bytes = id.to_opaque().into_bytes();
        let row = self
            .backend
            .connection()
            .query_row(
                messaging_sql::SELECT_DOMAIN_MESSAGE.sql,
                params![id_bytes.as_slice()],
                |row| {
                    Ok(MessageRow {
                        message_id: id_bytes.to_vec(),
                        conversation_id: row.get(0)?,
                        direction: row.get(1)?,
                        status: row.get(2)?,
                        body: row.get(3)?,
                        reply_to: row.get(4)?,
                        created_at_ms: row.get(5)?,
                        updated_at_ms: row.get(6)?,
                        attempt_count: row.get(7)?,
                    })
                },
            )
            .optional()
            .map_err(|_| MessageError::RepositoryFailure)?;
        row.map(MessageRow::into_message).transpose()
    }

    fn update(&mut self, message: Message) -> Result<(), MessageError> {
        if self.get(message.id())?.is_none() {
            return Err(MessageError::NotFound);
        }
        execute_message(&self.backend, messaging_sql::UPDATE_DOMAIN_MESSAGE.sql, &message)?;
        Ok(())
    }

    fn for_conversation(
        &self,
        conversation_id: ConversationId,
    ) -> Result<Vec<Message>, MessageError> {
        let conversation_bytes = conversation_id.to_opaque().into_bytes();
        let mut statement = self
            .backend
            .connection()
            .prepare(messaging_sql::SELECT_DOMAIN_FOR_CONVERSATION.sql)
            .map_err(|_| MessageError::RepositoryFailure)?;
        let rows = statement
            .query_map(params![conversation_bytes.as_slice()], |row| {
                Ok(MessageRow {
                    message_id: row.get(0)?,
                    conversation_id: conversation_bytes.to_vec(),
                    direction: row.get(1)?,
                    status: row.get(2)?,
                    body: row.get(3)?,
                    reply_to: row.get(4)?,
                    created_at_ms: row.get(5)?,
                    updated_at_ms: row.get(6)?,
                    attempt_count: row.get(7)?,
                })
            })
            .map_err(|_| MessageError::RepositoryFailure)?;
        rows.map(|row| {
            row.map_err(|_| MessageError::RepositoryFailure)?
                .into_message()
        })
        .collect()
    }

    fn list(&self) -> Result<Vec<Message>, MessageError> {
        let mut statement = self
            .backend
            .connection()
            .prepare(messaging_sql::LIST_DOMAIN_MESSAGES.sql)
            .map_err(|_| MessageError::RepositoryFailure)?;
        let rows = statement
            .query_map([], |row| {
                Ok(MessageRow {
                    message_id: row.get(0)?,
                    conversation_id: row.get(1)?,
                    direction: row.get(2)?,
                    status: row.get(3)?,
                    body: row.get(4)?,
                    reply_to: row.get(5)?,
                    created_at_ms: row.get(6)?,
                    updated_at_ms: row.get(7)?,
                    attempt_count: row.get(8)?,
                })
            })
            .map_err(|_| MessageError::RepositoryFailure)?;
        rows.map(|row| {
            row.map_err(|_| MessageError::RepositoryFailure)?
                .into_message()
        })
        .collect()
    }
}

struct MessageRow {
    message_id: Vec<u8>,
    conversation_id: Vec<u8>,
    direction: i64,
    status: i64,
    body: String,
    reply_to: Option<Vec<u8>>,
    created_at_ms: i64,
    updated_at_ms: i64,
    attempt_count: i64,
}

impl MessageRow {
    fn into_message(self) -> Result<Message, MessageError> {
        let id = MessageId::from_opaque(OpaqueId::from_bytes(fixed_16(self.message_id)?));
        let conversation_id =
            ConversationId::from_opaque(OpaqueId::from_bytes(fixed_16(self.conversation_id)?));
        let direction = decode_direction(self.direction)?;
        let status = decode_status(self.status)?;
        let body = MessageBody::new(self.body).map_err(|_| MessageError::RepositoryFailure)?;
        let reply_to = self
            .reply_to
            .map(|value| {
                fixed_16(value).map(|bytes| ReplyReference {
                    message_id: MessageId::from_opaque(OpaqueId::from_bytes(bytes)),
                })
            })
            .transpose()?;
        let created_at = Timestamp::from_unix_millis(self.created_at_ms)
            .map_err(|_| MessageError::RepositoryFailure)?;
        let updated_at = Timestamp::from_unix_millis(self.updated_at_ms)
            .map_err(|_| MessageError::RepositoryFailure)?;
        let attempt_count =
            u32::try_from(self.attempt_count).map_err(|_| MessageError::RepositoryFailure)?;
        let attempts = (1..=attempt_count)
            .map(|number| DeliveryAttempt { number, at: updated_at, error_code: None })
            .collect();
        Message::from_persisted(
            id,
            conversation_id,
            body,
            reply_to,
            direction,
            status,
            created_at,
            updated_at,
            attempts,
        )
        .map_err(|_| MessageError::RepositoryFailure)
    }
}

fn execute_message(
    backend: &SqlCipherBackend,
    sql: &str,
    message: &Message,
) -> Result<(), MessageError> {
    let id = message.id().to_opaque().into_bytes();
    let conversation_id = message.conversation_id().to_opaque().into_bytes();
    let reply_id = message.reply_to().map(|reply| reply.message_id.to_opaque().into_bytes());
    let reply = reply_id.as_ref().map(<[u8; 16]>::as_slice);
    let attempt_count =
        i64::try_from(message.attempts().len()).map_err(|_| MessageError::RepositoryFailure)?;
    backend
        .connection()
        .execute(
            sql,
            params![
                id.as_slice(),
                conversation_id.as_slice(),
                encode_direction(message.direction()),
                encode_status(message.status()),
                message.body().as_str(),
                reply,
                message.created_at().to_unix_millis(),
                message.updated_at().to_unix_millis(),
                attempt_count,
            ],
        )
        .map_err(|_| MessageError::RepositoryFailure)?;
    Ok(())
}

fn fixed_16(value: Vec<u8>) -> Result<[u8; 16], MessageError> {
    value.try_into().map_err(|_| MessageError::RepositoryFailure)
}

const fn encode_direction(value: MessageDirection) -> i64 {
    match value {
        MessageDirection::Outbound => 0,
        MessageDirection::Inbound => 1,
    }
}
fn decode_direction(value: i64) -> Result<MessageDirection, MessageError> {
    match value {
        0 => Ok(MessageDirection::Outbound),
        1 => Ok(MessageDirection::Inbound),
        _ => Err(MessageError::RepositoryFailure),
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
fn decode_status(value: i64) -> Result<MessageStatus, MessageError> {
    match value {
        0 => Ok(MessageStatus::Queued),
        1 => Ok(MessageStatus::Sending),
        2 => Ok(MessageStatus::Sent),
        3 => Ok(MessageStatus::Delivered),
        4 => Ok(MessageStatus::Read),
        5 => Ok(MessageStatus::Failed),
        6 => Ok(MessageStatus::Cancelled),
        _ => Err(MessageError::RepositoryFailure),
    }
}

#[cfg(test)]
mod tests {
    use torca_conversations::ConversationId;
    use torca_foundation::Timestamp;
    use torca_messaging::{Message, MessageBody, MessageId, MessageRepository};

    use crate::{DatabaseKey, SqlCipherMessageStore};

    #[test]
    fn message_round_trips_through_sqlcipher() {
        let key = DatabaseKey::new([0x26; 32]);
        let mut store = SqlCipherMessageStore::open_in_memory(&key).expect("open store");
        let message = Message::outbound(
            MessageId::from_u128(31),
            ConversationId::from_u128(32),
            MessageBody::new("hello").expect("body"),
            None,
            Timestamp::UNIX_EPOCH,
        );
        store.insert(message.clone()).expect("insert");
        assert_eq!(store.get(message.id()).expect("get"), Some(message));
    }
}
