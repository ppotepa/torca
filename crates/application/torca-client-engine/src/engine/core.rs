//! Core engine ownership and read projections.

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
        snapshot.messages = self.messages.list().map_err(map_error)?;
        let mut conversation_ids = Vec::new();
        for message in &snapshot.messages {
            let id = message.conversation_id();
            if !conversation_ids.contains(&id) {
                conversation_ids.push(id);
            }
        }
        for conversation_id in conversation_ids {
            snapshot.reactions.extend(
                self.messages.reactions_for_conversation(conversation_id).map_err(map_error)?,
            );
        }
        Ok(snapshot)
    }

    pub fn overview_snapshot(&self) -> Result<ClientSnapshot, EngineError> {
        let conversations = ConversationRepository::list(&self.relationships).map_err(map_error)?;
        Ok(ClientSnapshot {
            identity: self.identity.load().map_err(map_error)?,
            pairings: self.pairings.list().map_err(map_error)?,
            contacts: ContactRepository::list(&self.relationships).map_err(map_error)?,
            conversations,
            messages: Vec::new(),
            reactions: Vec::new(),
            avatar_genome: self.relationships.local_avatar_genome().map_err(map_error)?,
        })
    }

    pub fn message_status(
        &self,
        message_id: MessageId,
    ) -> Result<Option<MessageStatus>, EngineError> {
        self.messages
            .get(message_id)
            .map(|message| message.map(|value| value.status()))
            .map_err(map_error)
    }

    fn load_pairing(&self, id: PairingSessionId) -> Result<PairingSession, EngineError> {
        self.pairings
            .get(id)
            .map_err(map_error)?
            .ok_or_else(|| EngineError("pairing session not found".into()))
    }

    fn load_message(&self, id: MessageId) -> Result<Message, EngineError> {
        self.messages
            .get(id)
            .map_err(map_error)?
            .ok_or_else(|| EngineError("message not found".into()))
    }
}
