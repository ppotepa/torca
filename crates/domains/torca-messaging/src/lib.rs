//! Text-message lifecycle and retry semantics.

use core::fmt;
use std::collections::BTreeMap;
use std::time::Duration;

use torca_conversations::ConversationId;
use torca_foundation::{ErrorCode, OpaqueId, Timestamp};

/// Stable message identifier.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MessageId(OpaqueId);

impl MessageId {
    /// Creates an identifier from an opaque value.
    pub const fn from_opaque(value: OpaqueId) -> Self {
        Self(value)
    }
    /// Creates a deterministic identifier from an integer.
    pub const fn from_u128(value: u128) -> Self {
        Self(OpaqueId::from_u128(value))
    }
    /// Returns the opaque value.
    pub const fn to_opaque(self) -> OpaqueId {
        self.0
    }
}

impl fmt::Display for MessageId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, formatter)
    }
}

/// Validated text body.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MessageBody(String);

impl MessageBody {
    /// Maximum encoded body size.
    pub const MAX_BYTES: usize = 16 * 1024;
    /// Product limit for one user-visible text message.
    ///
    /// The byte limit above remains a protocol safety boundary. This smaller
    /// character limit keeps messages readable on both mobile and desktop and
    /// prevents accidental paste/flood payloads before they reach delivery.
    pub const MAX_CHARACTERS: usize = 1_000;

    /// Validates a message body.
    pub fn new(value: impl Into<String>) -> Result<Self, MessageError> {
        let value = value.into();
        if value.is_empty() {
            return Err(MessageError::EmptyBody);
        }
        if value.len() > Self::MAX_BYTES || value.chars().count() > Self::MAX_CHARACTERS {
            return Err(MessageError::BodyTooLarge { actual: value.len() });
        }
        if value.contains('\0') {
            return Err(MessageError::InvalidBody);
        }
        Ok(Self(value))
    }

    /// Returns the body text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Message direction relative to the local installation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageDirection {
    Outbound,
    Inbound,
}
/// Domain delivery state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageStatus {
    Queued,
    Sending,
    Sent,
    Delivered,
    Read,
    Failed,
    Cancelled,
}
/// Optional reply relationship.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReplyReference {
    pub message_id: MessageId,
}

/// One idempotent reaction state for a message and actor.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MessageReaction {
    message_id: MessageId,
    conversation_id: ConversationId,
    actor_id: OpaqueId,
    emoji: String,
    active: bool,
    updated_at: Timestamp,
}
impl MessageReaction {
    pub fn deterministic_id(message_id: MessageId, actor_id: OpaqueId, emoji: &str) -> OpaqueId {
        let mut bytes = message_id.to_opaque().into_bytes();
        for (index, value) in actor_id.as_bytes().iter().enumerate() {
            bytes[index] ^= *value;
        }
        for (index, value) in emoji.as_bytes().iter().enumerate() {
            bytes[index % bytes.len()] ^= *value;
        }
        let id = OpaqueId::from_bytes(bytes);
        if id.is_nil() { OpaqueId::from_u128(1) } else { id }
    }
    pub fn new(
        message_id: MessageId,
        conversation_id: ConversationId,
        actor_id: OpaqueId,
        emoji: impl Into<String>,
        active: bool,
        updated_at: Timestamp,
    ) -> Result<Self, MessageError> {
        let emoji = emoji.into();
        if emoji.is_empty() || emoji.len() > 32 || emoji.contains('\0') {
            return Err(MessageError::InvalidReaction);
        }
        Ok(Self { message_id, conversation_id, actor_id, emoji, active, updated_at })
    }
    pub const fn message_id(&self) -> MessageId {
        self.message_id
    }
    pub const fn conversation_id(&self) -> ConversationId {
        self.conversation_id
    }
    pub const fn actor_id(&self) -> OpaqueId {
        self.actor_id
    }
    pub fn emoji(&self) -> &str {
        &self.emoji
    }
    pub const fn active(&self) -> bool {
        self.active
    }
    pub const fn updated_at(&self) -> Timestamp {
        self.updated_at
    }
}
/// One local delivery attempt.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeliveryAttempt {
    pub number: u32,
    pub at: Timestamp,
    pub error_code: Option<ErrorCode>,
}

/// One text message aggregate.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Message {
    id: MessageId,
    conversation_id: ConversationId,
    body: MessageBody,
    reply_to: Option<ReplyReference>,
    direction: MessageDirection,
    status: MessageStatus,
    created_at: Timestamp,
    updated_at: Timestamp,
    sent_at: Option<Timestamp>,
    delivered_at: Option<Timestamp>,
    read_at: Option<Timestamp>,
    attempts: Vec<DeliveryAttempt>,
}

impl Message {
    /// Creates a queued outbound message.
    pub const fn outbound(
        id: MessageId,
        conversation_id: ConversationId,
        body: MessageBody,
        reply_to: Option<ReplyReference>,
        at: Timestamp,
    ) -> Self {
        Self {
            id,
            conversation_id,
            body,
            reply_to,
            direction: MessageDirection::Outbound,
            status: MessageStatus::Queued,
            created_at: at,
            updated_at: at,
            sent_at: None,
            delivered_at: None,
            read_at: None,
            attempts: Vec::new(),
        }
    }
    /// Creates a delivered inbound message.
    pub const fn inbound(
        id: MessageId,
        conversation_id: ConversationId,
        body: MessageBody,
        reply_to: Option<ReplyReference>,
        at: Timestamp,
    ) -> Self {
        Self {
            id,
            conversation_id,
            body,
            reply_to,
            direction: MessageDirection::Inbound,
            status: MessageStatus::Delivered,
            created_at: at,
            updated_at: at,
            sent_at: None,
            delivered_at: Some(at),
            read_at: None,
            attempts: Vec::new(),
        }
    }
    /// Restores an aggregate from trusted persistence after validating attempt numbering.
    #[allow(clippy::too_many_arguments)]
    pub fn from_persisted(
        id: MessageId,
        conversation_id: ConversationId,
        body: MessageBody,
        reply_to: Option<ReplyReference>,
        direction: MessageDirection,
        status: MessageStatus,
        created_at: Timestamp,
        updated_at: Timestamp,
        sent_at: Option<Timestamp>,
        delivered_at: Option<Timestamp>,
        read_at: Option<Timestamp>,
        attempts: Vec<DeliveryAttempt>,
    ) -> Result<Self, MessageError> {
        for (index, attempt) in attempts.iter().enumerate() {
            let expected = u32::try_from(index)
                .map_err(|_| MessageError::InvalidPersistedState)?
                .checked_add(1)
                .ok_or(MessageError::InvalidPersistedState)?;
            if attempt.number != expected {
                return Err(MessageError::InvalidPersistedState);
            }
        }
        if direction == MessageDirection::Inbound && !attempts.is_empty() {
            return Err(MessageError::InvalidPersistedState);
        }
        Ok(Self {
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
        })
    }
    /// Returns the message identifier.
    pub const fn id(&self) -> MessageId {
        self.id
    }
    /// Returns the conversation identifier.
    pub const fn conversation_id(&self) -> ConversationId {
        self.conversation_id
    }
    /// Returns the body.
    pub const fn body(&self) -> &MessageBody {
        &self.body
    }
    /// Returns direction.
    pub const fn direction(&self) -> MessageDirection {
        self.direction
    }
    /// Returns domain status.
    pub const fn status(&self) -> MessageStatus {
        self.status
    }
    /// Returns reply metadata.
    pub const fn reply_to(&self) -> Option<ReplyReference> {
        self.reply_to
    }
    /// Returns creation time for persistence and projections.
    pub const fn created_at(&self) -> Timestamp {
        self.created_at
    }
    /// Returns last mutation time for persistence and projections.
    pub const fn updated_at(&self) -> Timestamp {
        self.updated_at
    }
    pub const fn sent_at(&self) -> Option<Timestamp> {
        self.sent_at
    }
    pub const fn delivered_at(&self) -> Option<Timestamp> {
        self.delivered_at
    }
    pub const fn read_at(&self) -> Option<Timestamp> {
        self.read_at
    }
    /// Returns delivery attempts.
    pub fn attempts(&self) -> &[DeliveryAttempt] {
        &self.attempts
    }

    /// Starts one outbound send attempt.
    pub fn begin_send(&mut self, at: Timestamp) -> Result<u32, MessageError> {
        if self.status != MessageStatus::Queued || self.direction != MessageDirection::Outbound {
            return Err(MessageError::InvalidTransition);
        }
        let number = u32::try_from(self.attempts.len())
            .map_err(|_| MessageError::AttemptsExhausted)?
            .checked_add(1)
            .ok_or(MessageError::AttemptsExhausted)?;
        self.attempts.push(DeliveryAttempt { number, at, error_code: None });
        self.status = MessageStatus::Sending;
        self.updated_at = at;
        Ok(number)
    }
    /// Marks a sending message as accepted by the peer protocol.
    pub fn mark_sent(&mut self, at: Timestamp) -> Result<(), MessageError> {
        if self.status != MessageStatus::Sending {
            return Err(MessageError::InvalidTransition);
        }
        self.status = MessageStatus::Sent;
        self.updated_at = at;
        self.sent_at = Some(at);
        Ok(())
    }
    /// Applies a monotonic delivered state.
    pub fn mark_delivered(&mut self, at: Timestamp) -> Result<bool, MessageError> {
        match self.status {
            MessageStatus::Sent => {
                self.status = MessageStatus::Delivered;
                self.updated_at = at;
                self.delivered_at = Some(at);
                Ok(true)
            }
            MessageStatus::Delivered | MessageStatus::Read => Ok(false),
            _ => Err(MessageError::InvalidTransition),
        }
    }
    /// Applies a monotonic read state.
    pub fn mark_read(&mut self, at: Timestamp) -> Result<bool, MessageError> {
        match self.status {
            MessageStatus::Sent | MessageStatus::Delivered => {
                self.status = MessageStatus::Read;
                self.updated_at = at;
                if self.delivered_at.is_none() {
                    self.delivered_at = Some(at);
                }
                self.read_at = Some(at);
                Ok(true)
            }
            MessageStatus::Read => Ok(false),
            _ => Err(MessageError::InvalidTransition),
        }
    }
    /// Records a failed send attempt.
    pub fn mark_failed(
        &mut self,
        at: Timestamp,
        error_code: ErrorCode,
    ) -> Result<(), MessageError> {
        if self.status != MessageStatus::Sending {
            return Err(MessageError::InvalidTransition);
        }
        if let Some(attempt) = self.attempts.last_mut() {
            attempt.error_code = Some(error_code);
        }
        self.status = MessageStatus::Failed;
        self.updated_at = at;
        Ok(())
    }
    /// Returns a failed message to the queue.
    pub fn retry(&mut self, at: Timestamp) -> Result<(), MessageError> {
        if self.status != MessageStatus::Failed {
            return Err(MessageError::InvalidTransition);
        }
        self.status = MessageStatus::Queued;
        self.updated_at = at;
        Ok(())
    }
    /// Edits a queued or failed outbound message before delivery.
    pub fn edit(&mut self, body: MessageBody, at: Timestamp) -> Result<(), MessageError> {
        if self.direction != MessageDirection::Outbound
            || !matches!(self.status, MessageStatus::Queued | MessageStatus::Failed)
        {
            return Err(MessageError::InvalidTransition);
        }
        self.body = body;
        self.updated_at = at;
        Ok(())
    }
    /// Cancels a queued or failed message.
    pub fn cancel(&mut self, at: Timestamp) -> Result<(), MessageError> {
        if !matches!(self.status, MessageStatus::Queued | MessageStatus::Failed) {
            return Err(MessageError::InvalidTransition);
        }
        self.status = MessageStatus::Cancelled;
        self.updated_at = at;
        Ok(())
    }
}

/// Messaging domain failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MessageError {
    EmptyBody,
    BodyTooLarge {
        actual: usize,
    },
    InvalidBody,
    InvalidReaction,
    InvalidTransition,
    AttemptsExhausted,
    AlreadyExists,
    NotFound,
    /// Persisted aggregate is structurally invalid.
    InvalidPersistedState,
    /// Persistence dependency failed without exposing implementation details.
    RepositoryFailure,
}
impl fmt::Display for MessageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for MessageError {}

/// Bounded exponential retry policy.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
}
impl RetryPolicy {
    /// Returns the delay after the given number of attempts.
    pub fn delay_after(&self, attempts: u32) -> Option<Duration> {
        if attempts >= self.max_attempts {
            return None;
        }
        let exponent = attempts.saturating_sub(1).min(31);
        let multiplier = 1_u32 << exponent;
        self.base_delay.checked_mul(multiplier).map(|delay| delay.min(self.max_delay))
    }
}

/// Message persistence port.
pub trait MessageRepository {
    fn insert(&mut self, message: Message) -> Result<(), MessageError>;
    fn get(&self, id: MessageId) -> Result<Option<Message>, MessageError>;
    fn update(&mut self, message: Message) -> Result<(), MessageError>;
    fn for_conversation(
        &self,
        conversation_id: ConversationId,
    ) -> Result<Vec<Message>, MessageError>;
    fn list(&self) -> Result<Vec<Message>, MessageError>;
    fn upsert_reaction(&mut self, reaction: MessageReaction) -> Result<(), MessageError>;
    fn reactions_for_conversation(
        &self,
        conversation_id: ConversationId,
    ) -> Result<Vec<MessageReaction>, MessageError>;
}

/// In-memory message repository.
#[derive(Clone, Debug, Default)]
pub struct InMemoryMessageRepository {
    messages: BTreeMap<MessageId, Message>,
    reactions: BTreeMap<(MessageId, OpaqueId, String), MessageReaction>,
}
impl MessageRepository for InMemoryMessageRepository {
    fn insert(&mut self, message: Message) -> Result<(), MessageError> {
        if self.messages.contains_key(&message.id()) {
            return Err(MessageError::AlreadyExists);
        }
        self.messages.insert(message.id(), message);
        Ok(())
    }
    fn get(&self, id: MessageId) -> Result<Option<Message>, MessageError> {
        Ok(self.messages.get(&id).cloned())
    }
    fn update(&mut self, message: Message) -> Result<(), MessageError> {
        if !self.messages.contains_key(&message.id()) {
            return Err(MessageError::NotFound);
        }
        self.messages.insert(message.id(), message);
        Ok(())
    }
    fn for_conversation(
        &self,
        conversation_id: ConversationId,
    ) -> Result<Vec<Message>, MessageError> {
        Ok(self
            .messages
            .values()
            .filter(|message| message.conversation_id() == conversation_id)
            .cloned()
            .collect())
    }
    fn list(&self) -> Result<Vec<Message>, MessageError> {
        Ok(self.messages.values().cloned().collect())
    }
    fn upsert_reaction(&mut self, reaction: MessageReaction) -> Result<(), MessageError> {
        self.reactions.insert(
            (reaction.message_id(), reaction.actor_id(), reaction.emoji().to_owned()),
            reaction,
        );
        Ok(())
    }
    fn reactions_for_conversation(
        &self,
        conversation_id: ConversationId,
    ) -> Result<Vec<MessageReaction>, MessageError> {
        Ok(self
            .reactions
            .values()
            .filter(|reaction| reaction.conversation_id() == conversation_id)
            .filter(|reaction| reaction.active())
            .cloned()
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::MessageBody;

    #[test]
    fn message_body_accepts_one_thousand_characters() {
        assert!(MessageBody::new("a".repeat(MessageBody::MAX_CHARACTERS)).is_ok());
    }

    #[test]
    fn message_body_rejects_more_than_one_thousand_characters_even_when_under_byte_cap() {
        let value = "🙂".repeat(MessageBody::MAX_CHARACTERS + 1);
        assert!(MessageBody::new(value).is_err());
    }
}
