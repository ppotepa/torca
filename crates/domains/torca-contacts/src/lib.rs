//! Verified contact relationship domain.

use core::fmt;
use std::collections::BTreeMap;

use torca_foundation::{OpaqueId, Timestamp};
use torca_identity::PublicIdentity;

#[must_use]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ContactId(OpaqueId);
impl ContactId {
    pub const fn from_opaque(value: OpaqueId) -> Self {
        Self(value)
    }
    pub const fn from_u128(value: u128) -> Self {
        Self(OpaqueId::from_u128(value))
    }
    pub const fn to_opaque(self) -> OpaqueId {
        self.0
    }
}
impl fmt::Display for ContactId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContactStatus {
    Active,
    Blocked,
    Removed,
}

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContactRoute {
    onion_address: String,
    capability_id: OpaqueId,
}
impl ContactRoute {
    pub fn new(
        onion_address: impl Into<String>,
        capability_id: OpaqueId,
    ) -> Result<Self, ContactError> {
        let onion_address = onion_address.into();
        if onion_address.len() > 255
            || !onion_address.to_ascii_lowercase().ends_with(".onion")
            || onion_address.chars().any(char::is_whitespace)
        {
            return Err(ContactError::InvalidOnionAddress);
        }
        Ok(Self { onion_address, capability_id })
    }
    pub fn onion_address(&self) -> &str {
        &self.onion_address
    }
    pub const fn capability_id(&self) -> OpaqueId {
        self.capability_id
    }
}

/// Durable metadata required to authenticate this installation to one verified peer.
///
/// `secret_handle` is only an opaque reference into protected platform storage. Secret bytes are
/// never part of the contact database or domain aggregate.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PeerCredential {
    contact_id: ContactId,
    local_capability_id: OpaqueId,
    secret_handle: OpaqueId,
}
impl PeerCredential {
    pub fn new(
        contact_id: ContactId,
        local_capability_id: OpaqueId,
        secret_handle: OpaqueId,
    ) -> Result<Self, ContactError> {
        if local_capability_id.is_nil() || secret_handle.is_nil() {
            return Err(ContactError::InvalidCredential);
        }
        Ok(Self { contact_id, local_capability_id, secret_handle })
    }
    pub const fn contact_id(&self) -> ContactId {
        self.contact_id
    }
    pub const fn local_capability_id(&self) -> OpaqueId {
        self.local_capability_id
    }
    pub const fn secret_handle(&self) -> OpaqueId {
        self.secret_handle
    }
}

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
    pub const fn new(
        id: ContactId,
        remote_identity: PublicIdentity,
        route: ContactRoute,
        at: Timestamp,
    ) -> Self {
        Self {
            id,
            remote_identity,
            route,
            status: ContactStatus::Active,
            created_at: at,
            updated_at: at,
        }
    }
    pub const fn restore(
        id: ContactId,
        remote_identity: PublicIdentity,
        route: ContactRoute,
        status: ContactStatus,
        created_at: Timestamp,
        updated_at: Timestamp,
    ) -> Self {
        Self { id, remote_identity, route, status, created_at, updated_at }
    }
    pub const fn id(&self) -> ContactId {
        self.id
    }
    pub const fn remote_identity(&self) -> &PublicIdentity {
        &self.remote_identity
    }
    pub const fn route(&self) -> &ContactRoute {
        &self.route
    }
    pub const fn status(&self) -> ContactStatus {
        self.status
    }
    pub const fn created_at(&self) -> Timestamp {
        self.created_at
    }
    pub const fn updated_at(&self) -> Timestamp {
        self.updated_at
    }
    pub fn block(&mut self, at: Timestamp) -> Result<(), ContactError> {
        if self.status != ContactStatus::Active {
            return Err(ContactError::InvalidTransition);
        }
        self.status = ContactStatus::Blocked;
        self.updated_at = at;
        Ok(())
    }
    pub fn unblock(&mut self, at: Timestamp) -> Result<(), ContactError> {
        if self.status != ContactStatus::Blocked {
            return Err(ContactError::InvalidTransition);
        }
        self.status = ContactStatus::Active;
        self.updated_at = at;
        Ok(())
    }
    pub fn remove(&mut self, at: Timestamp) -> Result<(), ContactError> {
        if self.status == ContactStatus::Removed {
            return Err(ContactError::InvalidTransition);
        }
        self.status = ContactStatus::Removed;
        self.updated_at = at;
        Ok(())
    }
    pub fn update_route(&mut self, route: ContactRoute, at: Timestamp) -> Result<(), ContactError> {
        if self.status == ContactStatus::Removed {
            return Err(ContactError::InvalidTransition);
        }
        self.route = route;
        self.updated_at = at;
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContactError {
    InvalidOnionAddress,
    InvalidCredential,
    InvalidTransition,
    AlreadyExists,
    NotFound,
    RepositoryFailure,
}
impl fmt::Display for ContactError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for ContactError {}

pub trait ContactRepository {
    fn insert(&mut self, contact: Contact) -> Result<(), ContactError>;
    fn get(&self, id: ContactId) -> Result<Option<Contact>, ContactError>;
    fn update(&mut self, contact: Contact) -> Result<(), ContactError>;
    fn list(&self) -> Result<Vec<Contact>, ContactError>;
}

/// Persistence port for non-secret peer authentication metadata.
pub trait PeerCredentialRepository {
    fn insert_credential(&mut self, credential: PeerCredential) -> Result<(), ContactError>;
    fn credential_for_contact(
        &self,
        contact_id: ContactId,
    ) -> Result<Option<PeerCredential>, ContactError>;
}

#[derive(Clone, Debug, Default)]
pub struct InMemoryContactRepository {
    contacts: BTreeMap<ContactId, Contact>,
}
impl ContactRepository for InMemoryContactRepository {
    fn insert(&mut self, contact: Contact) -> Result<(), ContactError> {
        if self.contacts.contains_key(&contact.id()) {
            return Err(ContactError::AlreadyExists);
        }
        self.contacts.insert(contact.id(), contact);
        Ok(())
    }
    fn get(&self, id: ContactId) -> Result<Option<Contact>, ContactError> {
        Ok(self.contacts.get(&id).cloned())
    }
    fn update(&mut self, contact: Contact) -> Result<(), ContactError> {
        if !self.contacts.contains_key(&contact.id()) {
            return Err(ContactError::NotFound);
        }
        self.contacts.insert(contact.id(), contact);
        Ok(())
    }
    fn list(&self) -> Result<Vec<Contact>, ContactError> {
        Ok(self.contacts.values().cloned().collect())
    }
}

#[derive(Clone, Debug, Default)]
pub struct InMemoryPeerCredentialRepository {
    credentials: BTreeMap<ContactId, PeerCredential>,
}
impl PeerCredentialRepository for InMemoryPeerCredentialRepository {
    fn insert_credential(&mut self, credential: PeerCredential) -> Result<(), ContactError> {
        if self.credentials.contains_key(&credential.contact_id()) {
            return Err(ContactError::AlreadyExists);
        }
        self.credentials.insert(credential.contact_id(), credential);
        Ok(())
    }

    fn credential_for_contact(
        &self,
        contact_id: ContactId,
    ) -> Result<Option<PeerCredential>, ContactError> {
        Ok(self.credentials.get(&contact_id).copied())
    }
}
