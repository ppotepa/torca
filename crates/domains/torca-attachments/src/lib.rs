//! Bounded attachment metadata and lifecycle domain.

use core::fmt;
use std::collections::BTreeMap;

use torca_foundation::{ErrorCode, OpaqueId, Timestamp};
use torca_messaging::MessageId;

/// Maximum attachment size accepted by Torca 0.1: 16 MiB.
pub const MAX_ATTACHMENT_BYTES: u64 = 16 * 1024 * 1024;

/// Stable attachment identifier.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AttachmentId(OpaqueId);
impl AttachmentId {
    /// Creates an identifier from an opaque value.
    pub const fn from_opaque(value: OpaqueId) -> Self { Self(value) }
    /// Creates a deterministic identifier.
    pub const fn from_u128(value: u128) -> Self { Self(OpaqueId::from_u128(value)) }
    /// Returns the opaque value.
    pub const fn to_opaque(self) -> OpaqueId { self.0 }
}
impl fmt::Display for AttachmentId { fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result { fmt::Display::fmt(&self.0, formatter) } }

/// Validated user-facing file name without path information.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachmentName(String);
impl AttachmentName {
    /// Maximum UTF-8 byte length.
    pub const MAX_BYTES: usize = 255;
    /// Validates a file name.
    pub fn new(value: impl Into<String>) -> Result<Self, AttachmentError> {
        let value = value.into();
        if value.is_empty() { return Err(AttachmentError::EmptyName); }
        if value.len() > Self::MAX_BYTES { return Err(AttachmentError::NameTooLong { actual: value.len() }); }
        if value.chars().any(|character| character.is_control() || matches!(character, '/' | '\\')) || value == "." || value == ".." { return Err(AttachmentError::InvalidName); }
        Ok(Self(value))
    }
    /// Returns the file name.
    pub fn as_str(&self) -> &str { &self.0 }
}

/// Validated MIME media type.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MediaType(String);
impl MediaType {
    /// Maximum byte length.
    pub const MAX_BYTES: usize = 127;
    /// Validates a basic `type/subtype` media type.
    pub fn new(value: impl Into<String>) -> Result<Self, AttachmentError> {
        let value = value.into().to_ascii_lowercase();
        let mut parts = value.split('/');
        let valid = parts.next().is_some_and(valid_token) && parts.next().is_some_and(valid_token) && parts.next().is_none();
        if !valid || value.len() > Self::MAX_BYTES { return Err(AttachmentError::InvalidMediaType); }
        Ok(Self(value))
    }
    /// Returns the media type.
    pub fn as_str(&self) -> &str { &self.0 }
}
fn valid_token(value: &str) -> bool { !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'+' | b'.')) }

/// Attachment lifecycle state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttachmentStatus { Prepared, Encrypting, Queued, Transferring, Available, Failed, Cancelled }

/// One recorded transfer attempt.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AttachmentAttempt { pub number: u32, pub at: Timestamp, pub error_code: Option<ErrorCode> }

/// Attachment aggregate linked to a message.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Attachment {
    id: AttachmentId,
    message_id: MessageId,
    name: AttachmentName,
    media_type: MediaType,
    size: u64,
    status: AttachmentStatus,
    created_at: Timestamp,
    updated_at: Timestamp,
    attempts: Vec<AttachmentAttempt>,
}
impl Attachment {
    /// Creates prepared attachment metadata.
    pub fn prepare(id: AttachmentId, message_id: MessageId, name: AttachmentName, media_type: MediaType, size: u64, at: Timestamp) -> Result<Self, AttachmentError> {
        if size == 0 { return Err(AttachmentError::EmptyContent); }
        if size > MAX_ATTACHMENT_BYTES { return Err(AttachmentError::ContentTooLarge { actual: size }); }
        Ok(Self { id, message_id, name, media_type, size, status: AttachmentStatus::Prepared, created_at: at, updated_at: at, attempts: Vec::new() })
    }
    /// Returns the identifier.
    pub const fn id(&self) -> AttachmentId { self.id }
    /// Returns owning message.
    pub const fn message_id(&self) -> MessageId { self.message_id }
    /// Returns the file name.
    pub const fn name(&self) -> &AttachmentName { &self.name }
    /// Returns the media type.
    pub const fn media_type(&self) -> &MediaType { &self.media_type }
    /// Returns the expected plaintext size.
    pub const fn size(&self) -> u64 { self.size }
    /// Returns lifecycle state.
    pub const fn status(&self) -> AttachmentStatus { self.status }
    /// Starts encryption.
    pub fn begin_encryption(&mut self, at: Timestamp) -> Result<(), AttachmentError> { self.transition(AttachmentStatus::Prepared, AttachmentStatus::Encrypting, at) }
    /// Marks encrypted content as queued.
    pub fn mark_queued(&mut self, at: Timestamp) -> Result<(), AttachmentError> { self.transition(AttachmentStatus::Encrypting, AttachmentStatus::Queued, at) }
    /// Starts or retries transfer.
    pub fn begin_transfer(&mut self, at: Timestamp) -> Result<u32, AttachmentError> {
        if !matches!(self.status, AttachmentStatus::Queued | AttachmentStatus::Failed) { return Err(AttachmentError::InvalidTransition); }
        let number = u32::try_from(self.attempts.len()).unwrap_or(u32::MAX).saturating_add(1);
        self.attempts.push(AttachmentAttempt { number, at, error_code: None });
        self.status = AttachmentStatus::Transferring; self.updated_at = at; Ok(number)
    }
    /// Marks content available after authenticated storage or transfer.
    pub fn mark_available(&mut self, at: Timestamp) -> Result<(), AttachmentError> { self.transition(AttachmentStatus::Transferring, AttachmentStatus::Available, at) }
    /// Records transfer failure.
    pub fn mark_failed(&mut self, at: Timestamp, error_code: ErrorCode) -> Result<(), AttachmentError> {
        if self.status != AttachmentStatus::Transferring { return Err(AttachmentError::InvalidTransition); }
        if let Some(attempt) = self.attempts.last_mut() { attempt.error_code = Some(error_code); }
        self.status = AttachmentStatus::Failed; self.updated_at = at; Ok(())
    }
    /// Cancels before availability.
    pub fn cancel(&mut self, at: Timestamp) -> Result<(), AttachmentError> {
        if matches!(self.status, AttachmentStatus::Available | AttachmentStatus::Cancelled) { return Err(AttachmentError::InvalidTransition); }
        self.status = AttachmentStatus::Cancelled; self.updated_at = at; Ok(())
    }
    fn transition(&mut self, expected: AttachmentStatus, next: AttachmentStatus, at: Timestamp) -> Result<(), AttachmentError> {
        if self.status != expected { return Err(AttachmentError::InvalidTransition); }
        self.status = next; self.updated_at = at; Ok(())
    }
}

/// Attachment domain failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachmentError { EmptyName, NameTooLong { actual: usize }, InvalidName, InvalidMediaType, EmptyContent, ContentTooLarge { actual: u64 }, InvalidTransition, AlreadyExists, NotFound }
impl fmt::Display for AttachmentError { fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result { write!(formatter, "{self:?}") } }
impl std::error::Error for AttachmentError {}

/// Attachment repository port.
pub trait AttachmentRepository {
    /// Inserts metadata.
    fn insert(&mut self, attachment: Attachment) -> Result<(), AttachmentError>;
    /// Loads metadata.
    fn get(&self, id: AttachmentId) -> Result<Option<Attachment>, AttachmentError>;
    /// Updates metadata.
    fn update(&mut self, attachment: Attachment) -> Result<(), AttachmentError>;
    /// Lists attachments of a message.
    fn for_message(&self, message_id: MessageId) -> Result<Vec<Attachment>, AttachmentError>;
}

/// In-memory attachment repository.
#[derive(Clone, Debug, Default)]
pub struct InMemoryAttachmentRepository { attachments: BTreeMap<AttachmentId, Attachment> }
impl AttachmentRepository for InMemoryAttachmentRepository {
    fn insert(&mut self, attachment: Attachment) -> Result<(), AttachmentError> { if self.attachments.contains_key(&attachment.id()) { return Err(AttachmentError::AlreadyExists); } self.attachments.insert(attachment.id(), attachment); Ok(()) }
    fn get(&self, id: AttachmentId) -> Result<Option<Attachment>, AttachmentError> { Ok(self.attachments.get(&id).cloned()) }
    fn update(&mut self, attachment: Attachment) -> Result<(), AttachmentError> { if !self.attachments.contains_key(&attachment.id()) { return Err(AttachmentError::NotFound); } self.attachments.insert(attachment.id(), attachment); Ok(()) }
    fn for_message(&self, message_id: MessageId) -> Result<Vec<Attachment>, AttachmentError> { Ok(self.attachments.values().filter(|attachment| attachment.message_id() == message_id).cloned().collect()) }
}
