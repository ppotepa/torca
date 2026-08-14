//! Direct conversation container domain.

use core::fmt;
use std::collections::BTreeMap;

use torca_contacts::ContactId;
use torca_foundation::{OpaqueId, Timestamp};

/// Stable conversation identifier.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ConversationId(OpaqueId);
impl ConversationId {
    /// Creates an ID.
    pub const fn from_opaque(value: OpaqueId) -> Self {
        Self(value)
    }
    /// Creates an ID from an integer.
    pub const fn from_u128(value: u128) -> Self {
        Self(OpaqueId::from_u128(value))
    }
    /// Returns the opaque value.
    pub const fn to_opaque(self) -> OpaqueId {
        self.0
    }
}
impl fmt::Display for ConversationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

/// Conversation lifecycle state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConversationStatus {
    Active,
    Archived,
}

/// One direct conversation associated with one verified contact.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectConversation {
    id: ConversationId,
    contact_id: ContactId,
    status: ConversationStatus,
    created_at: Timestamp,
    updated_at: Timestamp,
}
impl DirectConversation {
    /// Creates an active direct conversation.
    pub const fn new(id: ConversationId, contact_id: ContactId, at: Timestamp) -> Self {
        Self { id, contact_id, status: ConversationStatus::Active, created_at: at, updated_at: at }
    }
    /// Restores a conversation aggregate from persistence.
    pub const fn from_persisted(
        id: ConversationId,
        contact_id: ContactId,
        status: ConversationStatus,
        created_at: Timestamp,
        updated_at: Timestamp,
    ) -> Self {
        Self { id, contact_id, status, created_at, updated_at }
    }
    /// Returns the ID.
    pub const fn id(&self) -> ConversationId {
        self.id
    }
    /// Returns contact ownership.
    pub const fn contact_id(&self) -> ContactId {
        self.contact_id
    }
    /// Returns lifecycle state.
    pub const fn status(&self) -> ConversationStatus {
        self.status
    }
    /// Returns creation time.
    pub const fn created_at(&self) -> Timestamp {
        self.created_at
    }
    /// Returns last mutation time.
    pub const fn updated_at(&self) -> Timestamp {
        self.updated_at
    }
    /// Archives the conversation.
    pub fn archive(&mut self, at: Timestamp) -> Result<(), ConversationError> {
        if self.status == ConversationStatus::Archived {
            return Err(ConversationError::InvalidTransition);
        }
        self.status = ConversationStatus::Archived;
        self.updated_at = at;
        Ok(())
    }
    /// Restores the conversation.
    pub fn restore(&mut self, at: Timestamp) -> Result<(), ConversationError> {
        if self.status == ConversationStatus::Active {
            return Err(ConversationError::InvalidTransition);
        }
        self.status = ConversationStatus::Active;
        self.updated_at = at;
        Ok(())
    }
}

/// Conversation error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConversationError {
    InvalidTransition,
    AlreadyExists,
    NotFound,
    ContactAlreadyHasConversation,
    /// Persistence dependency failed without exposing implementation details.
    RepositoryFailure,
}
impl fmt::Display for ConversationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for ConversationError {}

/// Conversation persistence port.
pub trait ConversationRepository {
    /// Inserts a direct conversation, preserving one conversation per contact.
    fn insert(&mut self, conversation: DirectConversation) -> Result<(), ConversationError>;
    /// Loads by ID.
    fn get(&self, id: ConversationId) -> Result<Option<DirectConversation>, ConversationError>;
    /// Loads by contact.
    fn for_contact(
        &self,
        contact_id: ContactId,
    ) -> Result<Option<DirectConversation>, ConversationError>;
    /// Lists conversations.
    fn list(&self) -> Result<Vec<DirectConversation>, ConversationError>;
    /// Persists a changed conversation aggregate.
    fn update(&mut self, conversation: DirectConversation) -> Result<(), ConversationError>;
}

/// In-memory repository.
#[derive(Clone, Debug, Default)]
pub struct InMemoryConversationRepository {
    conversations: BTreeMap<ConversationId, DirectConversation>,
}
impl InMemoryConversationRepository {
    /// Removes the conversation belonging to a deleted local relationship.
    pub fn remove_for_contact(&mut self, contact_id: ContactId) -> Option<DirectConversation> {
        let id = self.conversations.iter().find_map(|(id, conversation)| {
            (conversation.contact_id() == contact_id).then_some(*id)
        })?;
        self.conversations.remove(&id)
    }
}
impl ConversationRepository for InMemoryConversationRepository {
    fn insert(&mut self, conversation: DirectConversation) -> Result<(), ConversationError> {
        if self.conversations.contains_key(&conversation.id()) {
            return Err(ConversationError::AlreadyExists);
        }
        if self.conversations.values().any(|item| item.contact_id() == conversation.contact_id()) {
            return Err(ConversationError::ContactAlreadyHasConversation);
        }
        self.conversations.insert(conversation.id(), conversation);
        Ok(())
    }
    fn get(&self, id: ConversationId) -> Result<Option<DirectConversation>, ConversationError> {
        Ok(self.conversations.get(&id).cloned())
    }
    fn for_contact(
        &self,
        contact_id: ContactId,
    ) -> Result<Option<DirectConversation>, ConversationError> {
        Ok(self.conversations.values().find(|item| item.contact_id() == contact_id).cloned())
    }
    fn list(&self) -> Result<Vec<DirectConversation>, ConversationError> {
        Ok(self.conversations.values().cloned().collect())
    }
    fn update(&mut self, conversation: DirectConversation) -> Result<(), ConversationError> {
        if !self.conversations.contains_key(&conversation.id()) {
            return Err(ConversationError::NotFound);
        }
        self.conversations.insert(conversation.id(), conversation);
        Ok(())
    }
}
