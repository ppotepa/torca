//! Verified contact relationship domain.

use core::fmt;
use std::collections::BTreeMap;

use torca_foundation::{OpaqueId, ProviderId, Timestamp};
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
    capability_id: OpaqueId,
    endpoints: BTreeMap<ProviderId, Vec<u8>>,
}
impl ContactRoute {
    pub fn for_provider_endpoint(
        capability_id: OpaqueId,
        provider: impl Into<String>,
        endpoint: Vec<u8>,
    ) -> Result<Self, ContactError> {
        let provider =
            ProviderId::new(provider).map_err(|_| ContactError::InvalidTransportRoute)?;
        if endpoint.is_empty() || endpoint.len() > 8 * 1024 {
            return Err(ContactError::InvalidTransportRoute);
        }
        Ok(Self { capability_id, endpoints: BTreeMap::from([(provider, endpoint)]) })
    }

    pub fn from_provider_endpoints(
        capability_id: OpaqueId,
        endpoints: BTreeMap<ProviderId, Vec<u8>>,
    ) -> Result<Self, ContactError> {
        if endpoints.is_empty()
            || endpoints.values().any(|endpoint| endpoint.is_empty() || endpoint.len() > 8 * 1024)
        {
            return Err(ContactError::InvalidTransportRoute);
        }
        Ok(Self { capability_id, endpoints })
    }
    pub const fn capability_id(&self) -> OpaqueId {
        self.capability_id
    }
    pub fn provider_endpoint(&self, provider: &str) -> Option<&[u8]> {
        let provider = ProviderId::new(provider).ok()?;
        self.endpoints.get(&provider).map(Vec::as_slice)
    }
    pub fn provider_endpoints(&self) -> &BTreeMap<ProviderId, Vec<u8>> {
        &self.endpoints
    }

    /// Replaces one provider's opaque route while preserving routes owned by
    /// other providers.  Route refresh is used after a network migration;
    /// the endpoint bytes remain provider-owned and are never interpreted by
    /// the contacts domain.
    pub fn update_provider_endpoint(
        &mut self,
        provider: impl Into<String>,
        endpoint: Vec<u8>,
    ) -> Result<(), ContactError> {
        let provider =
            ProviderId::new(provider).map_err(|_| ContactError::InvalidTransportRoute)?;
        if endpoint.is_empty() || endpoint.len() > 8 * 1024 {
            return Err(ContactError::InvalidTransportRoute);
        }
        self.endpoints.insert(provider, endpoint);
        Ok(())
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
    country_code: Option<String>,
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
            country_code: None,
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
        Self { id, remote_identity, route, status, country_code: None, created_at, updated_at }
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
    pub fn set_country_code(&mut self, country_code: Option<String>) {
        self.country_code = country_code;
    }
    pub fn country_code(&self) -> Option<&str> {
        self.country_code.as_deref()
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
    InvalidTransportRoute,
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
impl InMemoryContactRepository {
    /// Removes a contact from this ephemeral repository.
    pub fn remove(&mut self, id: ContactId) -> Option<Contact> {
        self.contacts.remove(&id)
    }
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
impl InMemoryPeerCredentialRepository {
    /// Removes the credential belonging to a deleted local relationship.
    pub fn remove_credential(&mut self, contact_id: ContactId) -> Option<PeerCredential> {
        self.credentials.remove(&contact_id)
    }
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

#[cfg(test)]
mod tests {
    use super::ContactRoute;
    use torca_foundation::OpaqueId;

    #[test]
    fn direct_provider_route_keeps_opaque_bytes() {
        let route = ContactRoute::for_provider_endpoint(
            OpaqueId::from_u128(1),
            "iroh",
            b"node-opaque-endpoint".to_vec(),
        )
        .expect("valid direct endpoint");

        assert_eq!(route.provider_endpoint("iroh"), Some(b"node-opaque-endpoint".as_slice()));
    }

    #[test]
    fn provider_route_refresh_preserves_other_provider_entries() {
        let mut route = ContactRoute::for_provider_endpoint(
            OpaqueId::from_u128(1),
            "tor",
            b"opaque-tor-route".to_vec(),
        )
        .expect("valid opaque route");
        route
            .update_provider_endpoint("iroh", b"new-opaque-endpoint".to_vec())
            .expect("refresh route");

        assert_eq!(route.provider_endpoint("tor"), Some(b"opaque-tor-route".as_slice()));
        assert_eq!(route.provider_endpoint("iroh"), Some(b"new-opaque-endpoint".as_slice()));
    }
}
