//! Verified contact relationship domain.

use core::fmt;
use std::collections::BTreeMap;

use torca_foundation::{OpaqueId, Timestamp};
use torca_identity::PublicIdentity;

/// Stable contact identifier.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ContactId(OpaqueId);
impl ContactId {
    /// Creates an identifier.
    pub const fn from_opaque(value: OpaqueId) -> Self { Self(value) }
    /// Creates an identifier from an integer for deterministic composition and tests.
    pub const fn from_u128(value: u128) -> Self { Self(OpaqueId::from_u128(value)) }
    /// Returns the opaque value.
    pub const fn to_opaque(self) -> OpaqueId { self.0 }
}
impl fmt::Display for ContactId { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { fmt::Display::fmt(&self.0, f) } }

/// Current domain relationship state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContactStatus { Active, Blocked, Removed }

/// Direct onion route and opaque capability handle.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContactRoute { onion_address: String, capability_id: OpaqueId }
impl ContactRoute {
    /// Validates an onion route.
    pub fn new(onion_address: impl Into<String>, capability_id: OpaqueId) -> Result<Self, ContactError> {
        let onion_address = onion_address.into();
        if onion_address.len() > 255 || !onion_address.ends_with(".onion") || onion_address.chars().any(char::is_whitespace) {
            return Err(ContactError::InvalidOnionAddress);
        }
        Ok(Self { onion_address, capability_id })
    }
    /// Returns the onion address.
    pub fn onion_address(&self) -> &str { &self.onion_address }
    /// Returns the capability handle. Secret capability bytes remain in infrastructure.
    pub const fn capability_id(&self) -> OpaqueId { self.capability_id }
}

/// Verified contact aggregate.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Contact {
    id: ContactId,
    remote_identity: PublicIdentity,
    route: ContactRoute,
    status: ContactStatus,
    created_at: Timestamp,
    updated_at: Timestamp,
}
impl Contact {
    /// Creates an active verified contact.
    pub const fn new(id: ContactId, remote_identity: PublicIdentity, route: ContactRoute, at: Timestamp) -> Self {
        Self { id, remote_identity, route, status: ContactStatus::Active, created_at: at, updated_at: at }
    }
    /// Returns the ID.
    pub const fn id(&self) -> ContactId { self.id }
    /// Returns public remote identity.
    pub const fn remote_identity(&self) -> &PublicIdentity { &self.remote_identity }
    /// Returns the route.
    pub const fn route(&self) -> &ContactRoute { &self.route }
    /// Returns relationship state.
    pub const fn status(&self) -> ContactStatus { self.status }
    /// Returns creation time.
    pub const fn created_at(&self) -> Timestamp { self.created_at }
    /// Blocks an active contact.
    pub fn block(&mut self, at: Timestamp) -> Result<(), ContactError> {
        if self.status != ContactStatus::Active { return Err(ContactError::InvalidTransition); }
        self.status = ContactStatus::Blocked; self.updated_at = at; Ok(())
    }
    /// Restores a blocked contact.
    pub fn unblock(&mut self, at: Timestamp) -> Result<(), ContactError> {
        if self.status != ContactStatus::Blocked { return Err(ContactError::InvalidTransition); }
        self.status = ContactStatus::Active; self.updated_at = at; Ok(())
    }
    /// Removes a non-removed contact.
    pub fn remove(&mut self, at: Timestamp) -> Result<(), ContactError> {
        if self.status == ContactStatus::Removed { return Err(ContactError::InvalidTransition); }
        self.status = ContactStatus::Removed; self.updated_at = at; Ok(())
    }
    /// Updates the peer route while preserving relationship state.
    pub fn update_route(&mut self, route: ContactRoute, at: Timestamp) -> Result<(), ContactError> {
        if self.status == ContactStatus::Removed { return Err(ContactError::InvalidTransition); }
        self.route = route; self.updated_at = at; Ok(())
    }
}

/// Contact domain error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContactError { InvalidOnionAddress, InvalidTransition, AlreadyExists, NotFound }
impl fmt::Display for ContactError { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{self:?}") } }
impl std::error::Error for ContactError {}

/// Contact persistence port.
pub trait ContactRepository {
    /// Inserts a verified contact.
    fn insert(&mut self, contact: Contact) -> Result<(), ContactError>;
    /// Reads a contact.
    fn get(&self, id: ContactId) -> Result<Option<Contact>, ContactError>;
    /// Replaces a contact.
    fn update(&mut self, contact: Contact) -> Result<(), ContactError>;
    /// Lists contacts.
    fn list(&self) -> Result<Vec<Contact>, ContactError>;
}

/// In-memory repository for tests and engine composition.
#[derive(Clone, Debug, Default)]
pub struct InMemoryContactRepository { contacts: BTreeMap<ContactId, Contact> }
impl ContactRepository for InMemoryContactRepository {
    fn insert(&mut self, contact: Contact) -> Result<(), ContactError> {
        if self.contacts.contains_key(&contact.id()) { return Err(ContactError::AlreadyExists); }
        self.contacts.insert(contact.id(), contact); Ok(())
    }
    fn get(&self, id: ContactId) -> Result<Option<Contact>, ContactError> { Ok(self.contacts.get(&id).cloned()) }
    fn update(&mut self, contact: Contact) -> Result<(), ContactError> {
        if !self.contacts.contains_key(&contact.id()) { return Err(ContactError::NotFound); }
        self.contacts.insert(contact.id(), contact); Ok(())
    }
    fn list(&self) -> Result<Vec<Contact>, ContactError> { Ok(self.contacts.values().cloned().collect()) }
}
