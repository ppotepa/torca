// Responsibility: runtime port implemented by the single-writer engine.

pub trait EngineRuntime: Send + 'static {
    fn dispatch(&mut self, command: EngineCommand) -> Result<EngineResult, EngineError>;
    fn snapshot(&self) -> Result<ClientSnapshot, EngineError>;
    fn overview_snapshot(&self) -> Result<ClientSnapshot, EngineError> {
        self.snapshot()
    }
    fn avatar_genome_for_identity(
        &self,
        identity_id: IdentityId,
    ) -> Result<Option<AvatarGenomeRecord>, EngineError>;
    fn message_status(&self, message_id: MessageId) -> Result<Option<MessageStatus>, EngineError>;
    fn message_contact(&self, message_id: MessageId) -> Result<Option<ContactId>, EngineError>;
}

impl<I, K, P, L, M, R> EngineRuntime for ClientEngine<I, K, P, L, M, R>
where
    I: IdentityRepository + Send + 'static,
    K: IdentityKeyProvider + Send + 'static,
    P: PairingRepository + Send + 'static,
    L: RelationshipRepository + Send + 'static,
    M: MessageRepository + Send + 'static,
    R: ReceiptRepository + Send + 'static,
{
    fn dispatch(&mut self, command: EngineCommand) -> Result<EngineResult, EngineError> {
        ClientEngine::dispatch(self, command)
    }
    fn snapshot(&self) -> Result<ClientSnapshot, EngineError> {
        ClientEngine::snapshot(self)
    }
    fn overview_snapshot(&self) -> Result<ClientSnapshot, EngineError> {
        ClientEngine::overview_snapshot(self)
    }
    fn avatar_genome_for_identity(
        &self,
        identity_id: IdentityId,
    ) -> Result<Option<AvatarGenomeRecord>, EngineError> {
        self.relationships.avatar_genome_for_identity(identity_id)
    }
    fn message_status(&self, message_id: MessageId) -> Result<Option<MessageStatus>, EngineError> {
        ClientEngine::message_status(self, message_id)
    }
    fn message_contact(&self, message_id: MessageId) -> Result<Option<ContactId>, EngineError> {
        ClientEngine::message_contact(self, message_id)
    }
}
