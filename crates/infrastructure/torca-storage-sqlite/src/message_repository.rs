use std::collections::BTreeMap;
use std::path::Path;

use rusqlite::{OptionalExtension, params};
use torca_conversations::ConversationId;
use torca_foundation::{OpaqueId, Timestamp};
use torca_messaging::{
    DeliveryAttempt, Message, MessageBody, MessageDirection, MessageError, MessageId,
    MessageReaction, MessageRepository, MessageStatus, ReplyReference,
};

use crate::{
    DatabaseKey, MigrationError, SqlCipherBackend, StorageBackendError, StorageKernel,
    messaging_sql,
};

const PAGE_FOR_CONVERSATION_SQL: &str =
    include_str!("../sql/queries/message_page_for_conversation.sql");
const SEARCH_FOR_CONVERSATION_SQL: &str =
    include_str!("../sql/queries/message_search_for_conversation.sql");
const CONVERSATION_SUMMARIES_SQL: &str =
    include_str!("../sql/queries/conversation_message_summaries.sql");
const MAX_PAGE_SIZE: usize = 200;

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

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConversationMessagePage {
    pub messages: Vec<Message>,
    pub reactions: Vec<MessageReaction>,
    pub has_more: bool,
}

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConversationMessageSummary {
    pub conversation_id: ConversationId,
    pub unread_count: u32,
    pub last_activity_at: Timestamp,
    pub last_message: Option<Message>,
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

    /// Loads one bounded page ordered chronologically for presentation.
    /// `before` is an exclusive `(created_at, message_id)` cursor from the oldest loaded message.
    pub fn page_for_conversation(
        &self,
        conversation_id: ConversationId,
        before: Option<(Timestamp, MessageId)>,
        limit: usize,
    ) -> Result<ConversationMessagePage, MessageError> {
        let limit = limit.clamp(1, MAX_PAGE_SIZE);
        let fetch_limit =
            i64::try_from(limit.saturating_add(1)).map_err(|_| MessageError::RepositoryFailure)?;
        let conversation = conversation_id.to_opaque().into_bytes();
        let before_ms = before.map(|(at, _)| at.to_unix_millis());
        let before_id_bytes = before.map(|(_, id)| id.to_opaque().into_bytes());
        let before_id = before_id_bytes.as_ref().map(<[u8; 16]>::as_slice);
        let mut statement = self
            .backend
            .connection()
            .prepare(PAGE_FOR_CONVERSATION_SQL)
            .map_err(|_| MessageError::RepositoryFailure)?;
        let rows = statement
            .query_map(params![conversation.as_slice(), before_ms, before_id, fetch_limit], |row| {
                Ok(MessageRow {
                    message_id: row.get(0)?,
                    conversation_id: conversation.to_vec(),
                    direction: row.get(1)?,
                    status: row.get(2)?,
                    body: row.get(3)?,
                    reply_to: row.get(4)?,
                    created_at_ms: row.get(5)?,
                    updated_at_ms: row.get(6)?,
                    attempt_count: row.get(7)?,
                    sent_at_ms: row.get(8)?,
                    delivered_at_ms: row.get(9)?,
                    read_at_ms: row.get(10)?,
                })
            })
            .map_err(|_| MessageError::RepositoryFailure)?;
        let mut messages = rows
            .map(|row| row.map_err(|_| MessageError::RepositoryFailure)?.into_message())
            .collect::<Result<Vec<_>, _>>()?;
        let has_more = messages.len() > limit;
        if has_more {
            messages.truncate(limit);
        }
        messages.reverse();
        let reactions = self.reactions_for_conversation(conversation_id)?;
        Ok(ConversationMessagePage { messages, reactions, has_more })
    }

    /// Performs a literal case-insensitive substring search inside one conversation.
    pub fn search_conversation(
        &self,
        conversation_id: ConversationId,
        query: &str,
        limit: usize,
    ) -> Result<Vec<Message>, MessageError> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(Vec::new());
        }
        let limit = i64::try_from(limit.clamp(1, MAX_PAGE_SIZE))
            .map_err(|_| MessageError::RepositoryFailure)?;
        let conversation = conversation_id.to_opaque().into_bytes();
        let mut statement = self
            .backend
            .connection()
            .prepare(SEARCH_FOR_CONVERSATION_SQL)
            .map_err(|_| MessageError::RepositoryFailure)?;
        let rows = statement
            .query_map(params![conversation.as_slice(), query, limit], |row| {
                Ok(MessageRow {
                    message_id: row.get(0)?,
                    conversation_id: conversation.to_vec(),
                    direction: row.get(1)?,
                    status: row.get(2)?,
                    body: row.get(3)?,
                    reply_to: row.get(4)?,
                    created_at_ms: row.get(5)?,
                    updated_at_ms: row.get(6)?,
                    attempt_count: row.get(7)?,
                    sent_at_ms: row.get(8)?,
                    delivered_at_ms: row.get(9)?,
                    read_at_ms: row.get(10)?,
                })
            })
            .map_err(|_| MessageError::RepositoryFailure)?;
        rows.map(|row| row.map_err(|_| MessageError::RepositoryFailure)?.into_message()).collect()
    }

    /// Produces one storage-owned summary row per conversation without loading the full history.
    pub fn conversation_summaries(
        &self,
    ) -> Result<BTreeMap<ConversationId, ConversationMessageSummary>, MessageError> {
        let mut statement = self
            .backend
            .connection()
            .prepare(CONVERSATION_SUMMARIES_SQL)
            .map_err(|_| MessageError::RepositoryFailure)?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, Option<Vec<u8>>>(3)?,
                    row.get::<_, Option<i64>>(4)?,
                    row.get::<_, Option<i64>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<Vec<u8>>>(7)?,
                    row.get::<_, Option<i64>>(8)?,
                    row.get::<_, Option<i64>>(9)?,
                    row.get::<_, Option<i64>>(10)?,
                    row.get::<_, Option<i64>>(11)?,
                    row.get::<_, Option<i64>>(12)?,
                    row.get::<_, Option<i64>>(13)?,
                ))
            })
            .map_err(|_| MessageError::RepositoryFailure)?;

        let mut summaries = BTreeMap::new();
        for row in rows {
            let (
                conversation,
                unread_count,
                last_activity_at_ms,
                message_id,
                direction,
                status,
                body,
                reply_to,
                created_at_ms,
                updated_at_ms,
                attempt_count,
                sent_at_ms,
                delivered_at_ms,
                read_at_ms,
            ) = row.map_err(|_| MessageError::RepositoryFailure)?;
            let conversation_id =
                ConversationId::from_opaque(OpaqueId::from_bytes(fixed_16(conversation.clone())?));
            let last_message = match message_id {
                Some(message_id) => Some(
                    MessageRow {
                        message_id,
                        conversation_id: conversation,
                        direction: direction.ok_or(MessageError::RepositoryFailure)?,
                        status: status.ok_or(MessageError::RepositoryFailure)?,
                        body: body.ok_or(MessageError::RepositoryFailure)?,
                        reply_to,
                        created_at_ms: created_at_ms.ok_or(MessageError::RepositoryFailure)?,
                        updated_at_ms: updated_at_ms.ok_or(MessageError::RepositoryFailure)?,
                        attempt_count: attempt_count.ok_or(MessageError::RepositoryFailure)?,
                        sent_at_ms,
                        delivered_at_ms,
                        read_at_ms,
                    }
                    .into_message()?,
                ),
                None => None,
            };
            let unread_count =
                u32::try_from(unread_count).map_err(|_| MessageError::RepositoryFailure)?;
            let last_activity_at = Timestamp::from_unix_millis(last_activity_at_ms)
                .map_err(|_| MessageError::RepositoryFailure)?;
            summaries.insert(
                conversation_id,
                ConversationMessageSummary {
                    conversation_id,
                    unread_count,
                    last_activity_at,
                    last_message,
                },
            );
        }
        Ok(summaries)
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
                        sent_at_ms: row.get(8)?,
                        delivered_at_ms: row.get(9)?,
                        read_at_ms: row.get(10)?,
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
                    sent_at_ms: row.get(8)?,
                    delivered_at_ms: row.get(9)?,
                    read_at_ms: row.get(10)?,
                })
            })
            .map_err(|_| MessageError::RepositoryFailure)?;
        rows.map(|row| row.map_err(|_| MessageError::RepositoryFailure)?.into_message()).collect()
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
                    sent_at_ms: row.get(9)?,
                    delivered_at_ms: row.get(10)?,
                    read_at_ms: row.get(11)?,
                })
            })
            .map_err(|_| MessageError::RepositoryFailure)?;
        rows.map(|row| row.map_err(|_| MessageError::RepositoryFailure)?.into_message()).collect()
    }

    fn upsert_reaction(&mut self, reaction: MessageReaction) -> Result<(), MessageError> {
        let message_id = reaction.message_id().to_opaque().into_bytes();
        let conversation_id = reaction.conversation_id().to_opaque().into_bytes();
        let actor_id = reaction.actor_id().into_bytes();
        self.backend
            .connection()
            .execute(
                messaging_sql::UPSERT_REACTION.sql,
                params![
                    message_id.as_slice(),
                    conversation_id.as_slice(),
                    actor_id.as_slice(),
                    reaction.emoji(),
                    i64::from(reaction.active()),
                    reaction.updated_at().to_unix_millis(),
                ],
            )
            .map_err(|_| MessageError::RepositoryFailure)?;
        Ok(())
    }

    fn reactions_for_conversation(
        &self,
        conversation_id: ConversationId,
    ) -> Result<Vec<MessageReaction>, MessageError> {
        let conversation_id = conversation_id.to_opaque().into_bytes();
        let mut statement = self
            .backend
            .connection()
            .prepare(messaging_sql::REACTIONS_FOR_CONVERSATION.sql)
            .map_err(|_| MessageError::RepositoryFailure)?;
        let rows = statement
            .query_map(params![conversation_id.as_slice()], |row| {
                let message_id: Vec<u8> = row.get(0)?;
                let conversation_id: Vec<u8> = row.get(1)?;
                let actor_id: Vec<u8> = row.get(2)?;
                let emoji: String = row.get(3)?;
                let active: i64 = row.get(4)?;
                let updated_at_ms: i64 = row.get(5)?;
                Ok((message_id, conversation_id, actor_id, emoji, active, updated_at_ms))
            })
            .map_err(|_| MessageError::RepositoryFailure)?;
        rows.map(|row| {
            let (message_id, conversation_id, actor_id, emoji, active, updated_at_ms) =
                row.map_err(|_| MessageError::RepositoryFailure)?;
            let message_id = MessageId::from_opaque(OpaqueId::from_bytes(
                message_id.try_into().map_err(|_| MessageError::RepositoryFailure)?,
            ));
            let conversation_id = ConversationId::from_opaque(OpaqueId::from_bytes(
                conversation_id.try_into().map_err(|_| MessageError::RepositoryFailure)?,
            ));
            let actor_id = OpaqueId::from_bytes(
                actor_id.try_into().map_err(|_| MessageError::RepositoryFailure)?,
            );
            MessageReaction::new(
                message_id,
                conversation_id,
                actor_id,
                emoji,
                active != 0,
                Timestamp::from_unix_millis(updated_at_ms)
                    .map_err(|_| MessageError::RepositoryFailure)?,
            )
            .map_err(|_| MessageError::RepositoryFailure)
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
    sent_at_ms: Option<i64>,
    delivered_at_ms: Option<i64>,
    read_at_ms: Option<i64>,
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
        let sent_at = optional_timestamp(self.sent_at_ms)?;
        let delivered_at = optional_timestamp(self.delivered_at_ms)?;
        let read_at = optional_timestamp(self.read_at_ms)?;
        Message::from_persisted(
            id,
            conversation_id,
            body,
            reply_to,
            direction,
            status,
            created_at,
            updated_at,
            sent_at,
            delivered_at,
            read_at,
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
                message.sent_at().map(Timestamp::to_unix_millis),
                message.delivered_at().map(Timestamp::to_unix_millis),
                message.read_at().map(Timestamp::to_unix_millis),
            ],
        )
        .map_err(|_| MessageError::RepositoryFailure)?;
    Ok(())
}

fn optional_timestamp(value: Option<i64>) -> Result<Option<Timestamp>, MessageError> {
    value
        .map(|milliseconds| {
            Timestamp::from_unix_millis(milliseconds).map_err(|_| MessageError::RepositoryFailure)
        })
        .transpose()
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
        MessageStatus::Deleted => 7,
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
        7 => Ok(MessageStatus::Deleted),
        _ => Err(MessageError::RepositoryFailure),
    }
}

#[cfg(test)]
mod tests {
    use crate::{DatabaseKey, SqlCipherMessageStore};
    use torca_conversations::ConversationId;
    use torca_foundation::Timestamp;
    use torca_messaging::{Message, MessageBody, MessageId, MessageRepository};

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
