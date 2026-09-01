// Responsibility: core engine ownership and read projections.

pub struct ClientEngine<
    I = InMemoryIdentityRepository,
    K = DeterministicKeyProvider,
    P = InMemoryPairingRepository,
    L = InMemoryRelationshipRepository,
    M = InMemoryMessageRepository,
    R = InMemoryReceiptRepository,
> {
    identity: IdentityService<I, K>,
    pairings: P,
    relationships: L,
    messages: M,
    receipts: R,
}

impl Default
    for ClientEngine<
        InMemoryIdentityRepository,
        DeterministicKeyProvider,
        InMemoryPairingRepository,
        InMemoryRelationshipRepository,
        InMemoryMessageRepository,
        InMemoryReceiptRepository,
    >
{
    fn default() -> Self {
        Self::new(
            InMemoryIdentityRepository::default(),
            DeterministicKeyProvider::default(),
            InMemoryPairingRepository::default(),
            InMemoryRelationshipRepository::default(),
            InMemoryMessageRepository::default(),
            InMemoryReceiptRepository::default(),
        )
    }
}

impl<I, K, P, L, M, R> ClientEngine<I, K, P, L, M, R>
where
    I: IdentityRepository,
    K: IdentityKeyProvider,
    P: PairingRepository,
    L: RelationshipRepository,
    M: MessageRepository,
    R: ReceiptRepository,
{
    pub const fn new(
        identity_repository: I,
        key_provider: K,
        pairings: P,
        relationships: L,
        messages: M,
        receipts: R,
    ) -> Self {
        Self {
            identity: IdentityService::new(identity_repository, key_provider),
            pairings,
            relationships,
            messages,
            receipts,
        }
    }

    pub fn snapshot(&self) -> Result<ClientSnapshot, EngineError> {
        let mut snapshot = self.overview_snapshot()?;
        snapshot.messages = self.messages.list().map_err(|_| EngineError::Repository)?;
        let mut conversation_ids = Vec::new();
        for message in &snapshot.messages {
            let id = message.conversation_id();
            if !conversation_ids.contains(&id) {
                conversation_ids.push(id);
            }
        }
        for conversation_id in conversation_ids {
            snapshot.reactions.extend(
                self.messages
                    .reactions_for_conversation(conversation_id)
                    .map_err(|_| EngineError::Repository)?,
            );
        }
        Ok(snapshot)
    }

    pub fn overview_snapshot(&self) -> Result<ClientSnapshot, EngineError> {
        let conversations = ConversationRepository::list(&self.relationships)
            .map_err(|_| EngineError::Repository)?;
        let mut reactions = Vec::new();
        for conversation in &conversations {
            reactions.extend(
                self.messages
                    .reactions_for_conversation(conversation.id())
                    .map_err(|_| EngineError::Repository)?,
            );
        }
        Ok(ClientSnapshot {
            identity: self.identity.load().map_err(|_| EngineError::Identity)?,
            pairings: self.pairings.list().map_err(|_| EngineError::Repository)?,
            contacts: ContactRepository::list(&self.relationships)
                .map_err(|_| EngineError::Repository)?,
            conversations,
            messages: Vec::new(),
            reactions,
            avatar_genome: self.relationships.local_avatar_genome()?,
        })
    }

    pub fn message_status(
        &self,
        message_id: MessageId,
    ) -> Result<Option<MessageStatus>, EngineError> {
        self.messages
            .get(message_id)
            .map(|message| message.map(|value| value.status()))
            .map_err(|_| EngineError::Repository)
    }

    pub fn message(&self, message_id: MessageId) -> Result<Option<Message>, EngineError> {
        self.messages.get(message_id).map_err(|_| EngineError::Repository)
    }

    /// Resolves the peer that owns a durable message without materializing the
    /// complete overview projection. Runtime delivery uses this narrow query
    /// to create a peer demand only for the message recipient.
    pub fn message_contact(
        &self,
        message_id: MessageId,
    ) -> Result<Option<ContactId>, EngineError> {
        let Some(message) = self
            .messages
            .get(message_id)
            .map_err(|_| EngineError::Repository)?
        else {
            return Ok(None);
        };
        ConversationRepository::get(&self.relationships, message.conversation_id())
            .map_err(|_| EngineError::Repository)
            .map(|conversation| conversation.map(|value| value.contact_id()))
    }

    /// Returns only recipients that currently own retryable durable message
    /// work. This avoids treating every stored relationship as a peer-demand
    /// during RuntimeOwner recovery.
    pub fn pending_delivery_contacts(&self) -> Result<Vec<ContactId>, EngineError> {
        let mut contacts = std::collections::BTreeSet::new();
        for message in self.messages.list().map_err(|_| EngineError::Repository)? {
            if message.direction() != MessageDirection::Outbound
                || !matches!(message.status(), MessageStatus::Queued | MessageStatus::Sending)
            {
                continue;
            }
            if let Some(conversation) = ConversationRepository::get(
                &self.relationships,
                message.conversation_id(),
            )
            .map_err(|_| EngineError::Repository)?
            {
                contacts.insert(conversation.contact_id());
            }
        }
        Ok(contacts.into_iter().collect())
    }

    fn load_pairing(&self, id: PairingSessionId) -> Result<PairingSession, EngineError> {
        self.pairings
            .get(id)
            .map_err(|_| EngineError::Repository)?
            .ok_or(EngineError::NotFound)
    }

    fn load_message(&self, id: MessageId) -> Result<Message, EngineError> {
        self.messages
            .get(id)
            .map_err(|_| EngineError::Repository)?
            .ok_or(EngineError::NotFound)
    }
}
