use torca_contacts::{
    Contact, ContactError, ContactId, ContactRepository, ContactStatus, PeerCredential,
    PeerCredentialRepository,
};

/// Peer-link view of the durable relationship repository.
///
/// Product/UI code can still inspect blocked contacts through the normal repository, while the
/// authenticated socket owner can only resolve contacts that are currently Active. This prevents
/// both outbound reconnects and inbound handshakes for blocked relationships.
pub struct ActiveRelationshipStore<R> {
    inner: R,
}

impl<R> ActiveRelationshipStore<R> {
    pub const fn new(inner: R) -> Self {
        Self { inner }
    }
}

impl<R: ContactRepository> ContactRepository for ActiveRelationshipStore<R> {
    fn insert(&mut self, contact: Contact) -> Result<(), ContactError> {
        self.inner.insert(contact)
    }

    fn get(&self, id: ContactId) -> Result<Option<Contact>, ContactError> {
        Ok(self
            .inner
            .get(id)?
            .filter(|contact| contact.status() == ContactStatus::Active))
    }

    fn update(&mut self, contact: Contact) -> Result<(), ContactError> {
        self.inner.update(contact)
    }

    fn list(&self) -> Result<Vec<Contact>, ContactError> {
        Ok(self
            .inner
            .list()?
            .into_iter()
            .filter(|contact| contact.status() == ContactStatus::Active)
            .collect())
    }
}

impl<R: PeerCredentialRepository> PeerCredentialRepository for ActiveRelationshipStore<R> {
    fn insert_credential(&mut self, credential: PeerCredential) -> Result<(), ContactError> {
        self.inner.insert_credential(credential)
    }

    fn credential_for_contact(
        &self,
        contact_id: ContactId,
    ) -> Result<Option<PeerCredential>, ContactError> {
        self.inner.credential_for_contact(contact_id)
    }
}
