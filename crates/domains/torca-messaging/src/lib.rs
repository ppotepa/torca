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
    pub const fn from_opaque(value: OpaqueId) -> Self { Self(value) }
    /// Creates a deterministic identifier from an integer.
    pub const fn from_u128(value: u128) -> Self { Self(OpaqueId::from_u128(value)) }
    /// Returns the opaque value.
    pub const fn to_opaque(self) -> OpaqueId { self.0 }
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

    /// Validates a message body.
    pub fn new(value: impl Into<String>) -> Result<Self, MessageError> {
        let value = value.into();
        if value.is_empty() { return Err(MessageError::EmptyBody); }
        if value.len() > Self::MAX_BYTES { return Err(MessageError::BodyTooLarge { actual: value.len() }); }
        if value.contains('\0') { return Err(MessageError::InvalidBody); }
        Ok(Self(value))
    }

    /// Returns the body text.
    pub fn as_str(&self) -> &str { &self.0 }
}

/// Message direction relative to the local installation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageDirection { Outbound, Inbound }
/// Domain delivery state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageStatus { Queued, Sending, Sent, Delivered, Read, Failed, Cancelled }
/// Optional reply relationship.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReplyReference { pub message_id: MessageId }
/// One local delivery attempt.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeliveryAttempt { pub number: u32, pub at: Timestamp, pub error_code: Option<ErrorCode> }

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
    attempts: Vec<DeliveryAttempt>,
}

impl Message {
    /// Creates a queued outbound message.
    pub const fn outbound(id: MessageId, conversation_id: ConversationId, body: MessageBody, reply_to: Option<ReplyReference>, at: Timestamp) -> Self {
        Self { id, conversation_id, body, reply_to, direction: MessageDirection::Outbound, status: MessageStatus::Queued, created_at: at, updated_at: at, attempts: Vec::new() }
    }
    /// Creates a delivered inbound message.
    pub const fn inbound(id: MessageId, conversation_id: ConversationId, body: MessageBody, reply_to: Option<ReplyReference>, at: Timestamp) -> Self {
        Self { id, conversation_id, body, reply_to, direction: MessageDirection::Inbound, status: MessageStatus::Delivered, created_at: at, updated_at: at, attempts: Vec::new() }
    }
    /// Returns the message identifier.
    pub const fn id(&self) -> MessageId { self.id }
    /// Returns the conversation identifier.
    pub const fn conversation_id(&self) -> ConversationId { self.conversation_id }
    /// Returns the body.
    pub const fn body(&self) -> &MessageBody { &self.body }
    /// Returns direction.
    pub const fn direction(&self) -> MessageDirection { self.direction }
    /// Returns domain status.
    pub const fn status(&self) -> MessageStatus { self.status }
    /// Returns reply metadata.
    pub const fn reply_to(&self) -> Option<ReplyReference> { self.reply_to }
    /// Returns creation time for persistence and projections.
    pub const fn created_at(&self) -> Timestamp { self.created_at }
    /// Returns last mutation time for persistence and projections.
    pub const fn updated_at(&self) -> Timestamp { self.updated_at }
    /// Returns delivery attempts.
    pub fn attempts(&self) -> &[DeliveryAttempt] { &self.attempts }

    /// Starts one outbound send attempt.
    pub fn begin_send(&mut self, at: Timestamp) -> Result<u32, MessageError> {
        if self.status != MessageStatus::Queued || self.direction != MessageDirection::Outbound { return Err(MessageError::InvalidTransition); }
        let number = u32::try_from(self.attempts.len()).map_err(|_| MessageError::AttemptsExhausted)?.checked_add(1).ok_or(MessageError::AttemptsExhausted)?;
        self.attempts.push(DeliveryAttempt { number, at, error_code: None });
        self.status = MessageStatus::Sending;
        self.updated_at = at;
        Ok(number)
    }
    /// Marks a sending message as accepted by the peer protocol.
    pub fn mark_sent(&mut self, at: Timestamp) -> Result<(), MessageError> {
        if self.status != MessageStatus::Sending { return Err(MessageError::InvalidTransition); }
        self.status = MessageStatus::Sent;
        self.updated_at = at;
        Ok(())
    }
    /// Applies a monotonic delivered state.
    pub fn mark_delivered(&mut self, at: Timestamp) -> Result<bool, MessageError> {
        match self.status {
            MessageStatus::Sent => { self.status = MessageStatus::Delivered; self.updated_at = at; Ok(true) }
            MessageStatus::Delivered | MessageStatus::Read => Ok(false),
            _ => Err(MessageError::InvalidTransition),
        }
    }
    /// Applies a monotonic read state.
    pub fn mark_read(&mut self, at: Timestamp) -> Result<bool, MessageError> {
        match self.status {
            MessageStatus::Sent | MessageStatus::Delivered => { self.status = MessageStatus::Read; self.updated_at = at; Ok(true) }
            MessageStatus::Read => Ok(false),
            _ => Err(MessageError::InvalidTransition),
        }
    }
    /// Records a failed send attempt.
    pub fn mark_failed(&mut self, at: Timestamp, error_code: ErrorCode) -> Result<(), MessageError> {
        if self.status != MessageStatus::Sending { return Err(MessageError::InvalidTransition); }
        if let Some(attempt) = self.attempts.last_mut() { attempt.error_code = Some(error_code); }
        self.status = MessageStatus::Failed;
        self.updated_at = at;
        Ok(())
    }
    /// Returns a failed message to the queue.
    pub fn retry(&mut self, at: Timestamp) -> Result<(), MessageError> {
        if self.status != MessageStatus::Failed { return Err(MessageError::InvalidTransition); }
        self.status = MessageStatus::Queued;
        self.updated_at = at;
        Ok(())
    }
    /// Cancels a queued or failed message.
    pub fn cancel(&mut self, at: Timestamp) -> Result<(), MessageError> {
        if !matches!(self.status, MessageStatus::Queued | MessageStatus::Failed) { return Err(MessageError::InvalidTransition); }
        self.status = MessageStatus::Cancelled;
        self.updated_at = at;
        Ok(())
    }
}

/// Messaging domain failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MessageError { EmptyBody, BodyTooLarge { actual: usize }, InvalidBody, InvalidTransition, AttemptsExhausted, AlreadyExists, NotFound }
impl fmt::Display for MessageError { fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result { write!(formatter, "{self:?}") } }
impl std::error::Error for MessageError {}

/// Bounded exponential retry policy.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetryPolicy { pub max_attempts: u32, pub base_delay: Duration, pub max_delay: Duration }
impl RetryPolicy {
    /// Returns the delay after the given number of attempts.
    pub fn delay_after(&self, attempts: u32) -> Option<Duration> {
        if attempts >= self.max_attempts { return None; }
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
    fn for_conversation(&self, conversation_id: ConversationId) -> Result<Vec<Message>, MessageError>;
    fn list(&self) -> Result<Vec<Message>, MessageError>;
}

/// In-memory message repository.
#[derive(Clone, Debug, Default)]
pub struct InMemoryMessageRepository { messages: BTreeMap<MessageId, Message> }
impl MessageRepository for InMemoryMessageRepository {
    fn insert(&mut self, message: Message) -> Result<(), MessageError> { if self.messages.contains_key(&message.id()) { return Err(MessageError::AlreadyExists); } self.messages.insert(message.id(), message); Ok(()) }
    fn get(&self, id: MessageId) -> Result<Option<Message>, MessageError> { Ok(self.messages.get(&id).cloned()) }
    fn update(&mut self, message: Message) -> Result<(), MessageError> { if !self.messages.contains_key(&message.id()) { return Err(MessageError::NotFound); } self.messages.insert(message.id(), message); Ok(()) }
    fn for_conversation(&self, conversation_id: ConversationId) -> Result<Vec<Message>, MessageError> { Ok(self.messages.values().filter(|message| message.conversation_id() == conversation_id).cloned().collect()) }
    fn list(&self) -> Result<Vec<Message>, MessageError> { Ok(self.messages.values().cloned().collect()) }
}
