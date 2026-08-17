// Responsibility: relationship storage boundary and in-memory implementation.

pub trait RelationshipRepository:
    ContactRepository + ConversationRepository + PeerCredentialRepository
{
    fn upsert_avatar_genome(
        &mut self,
        record: AvatarGenomeRecord,
        at: Timestamp,
    ) -> Result<(), EngineError>;
    fn avatar_genome(&self, hash: [u8; 32]) -> Result<Option<AvatarGenomeRecord>, EngineError>;
    fn avatar_genome_for_identity(
        &self,
        _identity_id: IdentityId,
    ) -> Result<Option<AvatarGenomeRecord>, EngineError> {
        Ok(None)
    }
    fn local_avatar_genome(&self) -> Result<Option<AvatarGenomeRecord>, EngineError> {
        Ok(None)
    }
    fn insert_pairing_result(
        &mut self,
        contact: Contact,
        conversation: DirectConversation,
        display_name: &str,
        credential: PeerCredential,
        avatar: Option<AvatarGenomeRecord>,
        at: Timestamp,
    ) -> Result<(), EngineError>;
    fn remove_relationship(&mut self, contact_id: ContactId) -> Result<(), EngineError>;
}

#[derive(Clone, Debug, Default)]
pub struct InMemoryRelationshipRepository {
    contacts: InMemoryContactRepository,
    conversations: InMemoryConversationRepository,
    credentials: InMemoryPeerCredentialRepository,
    avatar_genomes: Vec<AvatarGenomeRecord>,
    local_avatar_hash: Option<[u8; 32]>,
    identity_avatars: Vec<(IdentityId, [u8; 32])>,
}

impl ContactRepository for InMemoryRelationshipRepository {
    fn insert(&mut self, contact: Contact) -> Result<(), ContactError> {
        self.contacts.insert(contact)
    }
    fn get(&self, id: ContactId) -> Result<Option<Contact>, ContactError> {
        self.contacts.get(id)
    }
    fn update(&mut self, contact: Contact) -> Result<(), ContactError> {
        self.contacts.update(contact)
    }
    fn list(&self) -> Result<Vec<Contact>, ContactError> {
        self.contacts.list()
    }
}
impl ConversationRepository for InMemoryRelationshipRepository {
    fn insert(&mut self, conversation: DirectConversation) -> Result<(), ConversationError> {
        self.conversations.insert(conversation)
    }
    fn get(&self, id: ConversationId) -> Result<Option<DirectConversation>, ConversationError> {
        self.conversations.get(id)
    }
    fn for_contact(
        &self,
        contact_id: ContactId,
    ) -> Result<Option<DirectConversation>, ConversationError> {
        self.conversations.for_contact(contact_id)
    }
    fn list(&self) -> Result<Vec<DirectConversation>, ConversationError> {
        self.conversations.list()
    }
    fn update(&mut self, conversation: DirectConversation) -> Result<(), ConversationError> {
        self.conversations.update(conversation)
    }
}
impl PeerCredentialRepository for InMemoryRelationshipRepository {
    fn insert_credential(&mut self, credential: PeerCredential) -> Result<(), ContactError> {
        self.credentials.insert_credential(credential)
    }
    fn credential_for_contact(
        &self,
        contact_id: ContactId,
    ) -> Result<Option<PeerCredential>, ContactError> {
        self.credentials.credential_for_contact(contact_id)
    }
}
impl RelationshipRepository for InMemoryRelationshipRepository {
    fn upsert_avatar_genome(
        &mut self,
        record: AvatarGenomeRecord,
        _at: Timestamp,
    ) -> Result<(), EngineError> {
        self.local_avatar_hash = Some(record.genome_hash);
        if let Some(existing) = self
            .avatar_genomes
            .iter_mut()
            .find(|existing| existing.genome_hash == record.genome_hash)
        {
            *existing = record;
        } else {
            self.avatar_genomes.push(record);
        }
        Ok(())
    }

    fn avatar_genome(&self, hash: [u8; 32]) -> Result<Option<AvatarGenomeRecord>, EngineError> {
        Ok(self.avatar_genomes.iter().find(|record| record.genome_hash == hash).cloned())
    }

    fn avatar_genome_for_identity(
        &self,
        identity_id: IdentityId,
    ) -> Result<Option<AvatarGenomeRecord>, EngineError> {
        let Some((_, hash)) =
            self.identity_avatars.iter().find(|(candidate, _)| *candidate == identity_id)
        else {
            return Ok(None);
        };
        self.avatar_genome(*hash)
    }

    fn local_avatar_genome(&self) -> Result<Option<AvatarGenomeRecord>, EngineError> {
        self.local_avatar_hash.map_or(Ok(None), |hash| self.avatar_genome(hash))
    }

    fn insert_pairing_result(
        &mut self,
        contact: Contact,
        conversation: DirectConversation,
        _display_name: &str,
        credential: PeerCredential,
        avatar: Option<AvatarGenomeRecord>,
        _at: Timestamp,
    ) -> Result<(), EngineError> {
        if contact.id() != conversation.contact_id() || contact.id() != credential.contact_id() {
            return Err(EngineError::InvalidState);
        }
        if ContactRepository::get(self, contact.id()).map_err(|_| EngineError::Repository)?.is_some()
            || ConversationRepository::get(self, conversation.id())
                .map_err(|_| EngineError::Repository)?
                .is_some()
            || ConversationRepository::for_contact(self, contact.id())
                .map_err(|_| EngineError::Repository)?
                .is_some()
            || PeerCredentialRepository::credential_for_contact(self, contact.id())
                .map_err(|_| EngineError::Repository)?
                .is_some()
        {
            return Err(EngineError::Conflict);
        }
        let mut contacts = self.contacts.clone();
        let mut conversations = self.conversations.clone();
        let mut credentials = self.credentials.clone();
        let remote_identity_id = contact.remote_identity().identity_id();
        contacts.insert(contact).map_err(|_| EngineError::Repository)?;
        conversations.insert(conversation).map_err(|_| EngineError::Repository)?;
        credentials.insert_credential(credential).map_err(|_| EngineError::Repository)?;
        self.contacts = contacts;
        self.conversations = conversations;
        self.credentials = credentials;
        if let Some(avatar) = avatar {
            let hash = avatar.genome_hash;
            if let Some(existing) =
                self.avatar_genomes.iter_mut().find(|existing| existing.genome_hash == hash)
            {
                *existing = avatar;
            } else {
                self.avatar_genomes.push(avatar);
            }
            self.identity_avatars.retain(|(identity, _)| *identity != remote_identity_id);
            self.identity_avatars.push((remote_identity_id, hash));
        }
        Ok(())
    }

    fn remove_relationship(&mut self, contact_id: ContactId) -> Result<(), EngineError> {
        if ContactRepository::get(self, contact_id)
            .map_err(|_| EngineError::Repository)?
            .is_none()
        {
            return Err(EngineError::NotFound);
        }
        let mut contacts = self.contacts.clone();
        let mut conversations = self.conversations.clone();
        let mut credentials = self.credentials.clone();
        contacts.remove(contact_id);
        conversations.remove_for_contact(contact_id);
        credentials.remove_credential(contact_id);
        self.contacts = contacts;
        self.conversations = conversations;
        self.credentials = credentials;
        Ok(())
    }
}
