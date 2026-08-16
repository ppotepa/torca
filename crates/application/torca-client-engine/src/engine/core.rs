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

    #[allow(clippy::too_many_lines)]
    pub fn dispatch(&mut self, command: EngineCommand) -> Result<EngineResult, EngineError> {
        match command {
            EngineCommand::CreateIdentity { identity_id, profile, at } => {
                let (_identity, _event) = self
                    .identity
                    .create(CreateIdentity { identity_id, profile, at })
                    .map_err(map_error)?;
                Ok(EngineResult::IdentityCreated)
            }
            EngineCommand::UpdateProfile { display_name, at } => {
                let profile = Profile::new(display_name, None);
                let (_identity, _event) = self
                    .identity
                    .update_profile(UpdateProfile { profile, at })
                    .map_err(map_error)?;
                Ok(EngineResult::ProfileUpdated)
            }
            EngineCommand::SetAvatarGenome { record, at } => {
                self.relationships.upsert_avatar_genome(record, at)?;
                Ok(EngineResult::ProfileUpdated)
            }
            EngineCommand::StartPairing { session_id, code, expires_at } => {
                self.pairings
                    .insert(PairingSession::creator(session_id, code, expires_at))
                    .map_err(map_error)?;
                Ok(EngineResult::PairingStarted)
            }
            EngineCommand::JoinPairing { session_id, code, expires_at } => {
                self.pairings
                    .insert(PairingSession::joining(session_id, code, expires_at))
                    .map_err(map_error)?;
                Ok(EngineResult::PairingJoined)
            }
            EngineCommand::PeerJoined { session_id, proposal, at } => {
                let mut session = self.load_pairing(session_id)?;
                session.peer_joined(proposal, at).map_err(map_error)?;
                self.pairings.update(session).map_err(map_error)?;
                Ok(EngineResult::PairingUpdated)
            }
            EngineCommand::ApprovePairing { session_id, at } => {
                let mut session = self.load_pairing(session_id)?;
                session.approve_local(at).map_err(map_error)?;
                self.pairings.update(session).map_err(map_error)?;
                Ok(EngineResult::PairingUpdated)
            }
            EngineCommand::RejectPairing { session_id } => {
                let mut session = self.load_pairing(session_id)?;
                session.reject().map_err(map_error)?;
                self.pairings.update(session).map_err(map_error)?;
                Ok(EngineResult::PairingRejected)
            }
            EngineCommand::CancelPairing { session_id } => {
                let mut session = self.load_pairing(session_id)?;
                session.cancel().map_err(map_error)?;
                self.pairings.update(session).map_err(map_error)?;
                Ok(EngineResult::PairingCancelled)
            }
            EngineCommand::ExpirePairing { session_id, at } => {
                let mut session = self.load_pairing(session_id)?;
                if !session.expire(at) {
                    return Err(EngineError("pairing session is not due to expire".into()));
                }
                self.pairings.update(session).map_err(map_error)?;
                Ok(EngineResult::PairingUpdated)
            }
            EngineCommand::RemoteApproved { session_id, at } => {
                let mut session = self.load_pairing(session_id)?;
                session.approve_remote(at).map_err(map_error)?;
                self.pairings.update(session).map_err(map_error)?;
                Ok(EngineResult::PairingUpdated)
            }
            EngineCommand::CompletePairing {
                session_id,
                contact_id,
                conversation_id,
                display_name,
                credential,
                at,
            } => {
                if credential.contact_id() != contact_id {
                    return Err(EngineError(
                        "peer credential contact does not match pairing".into(),
                    ));
                }
                let existing_contact =
                    ContactRepository::get(&self.relationships, contact_id).map_err(map_error)?;
                let existing_conversation =
                    ConversationRepository::get(&self.relationships, conversation_id)
                        .map_err(map_error)?;
                let existing_for_contact =
                    ConversationRepository::for_contact(&self.relationships, contact_id)
                        .map_err(map_error)?;
                let existing_credential = PeerCredentialRepository::credential_for_contact(
                    &self.relationships,
                    contact_id,
                )
                .map_err(map_error)?;
                if existing_contact.is_some()
                    && existing_conversation
                        .as_ref()
                        .is_some_and(|conversation| conversation.contact_id() == contact_id)
                    && existing_for_contact.is_some()
                    && existing_credential.is_some()
                {
                    return Ok(EngineResult::PairingCompleted { contact_id, conversation_id });
                }
                if existing_contact.is_some()
                    || existing_conversation.is_some()
                    || existing_for_contact.is_some()
                    || existing_credential.is_some()
                {
                    return Err(EngineError(
                        "contact, conversation or peer credential already exists".into(),
                    ));
                }
                let mut session = self.load_pairing(session_id)?;
                let proposal = session.complete(at).map_err(map_error)?;
                let avatar = proposal.avatar.as_ref().map(|avatar| AvatarGenomeRecord {
                    genome_hash: avatar.genome_hash,
                    schema_version: avatar.schema_version,
                    generator_version: avatar.generator_version.clone(),
                    catalog_version: avatar.catalog_version.clone(),
                    compressed_genome: avatar.compressed_genome.clone(),
                });
                let contact =
                    Contact::new(contact_id, proposal.public_identity, proposal.route, at);
                let conversation = DirectConversation::new(conversation_id, contact_id, at);
                self.relationships.insert_pairing_result(
                    contact,
                    conversation,
                    &display_name,
                    credential,
                    avatar,
                    at,
                )?;
                let _ = self.pairings.update(session);
                Ok(EngineResult::PairingCompleted { contact_id, conversation_id })
            }
            EngineCommand::RemovePairing { session_id } => {
                self.pairings.delete(session_id).map_err(map_error)?;
                Ok(EngineResult::PairingRemoved)
            }
            EngineCommand::EnsureConversation { contact_id, conversation_id, at } => {
                if ContactRepository::get(&self.relationships, contact_id)
                    .map_err(map_error)?
                    .is_none()
                {
                    return Err(EngineError("contact not found".into()));
                }
                if let Some(existing) =
                    ConversationRepository::for_contact(&self.relationships, contact_id)
                        .map_err(map_error)?
                {
                    return Ok(EngineResult::ConversationStarted {
                        conversation_id: existing.id(),
                    });
                }
                if ConversationRepository::get(&self.relationships, conversation_id)
                    .map_err(map_error)?
                    .is_some()
                {
                    return Err(EngineError("conversation id already exists".into()));
                }
                ConversationRepository::insert(
                    &mut self.relationships,
                    DirectConversation::new(conversation_id, contact_id, at),
                )
                .map_err(map_error)?;
                Ok(EngineResult::ConversationStarted { conversation_id })
            }
            EngineCommand::RemoveContact { contact_id } => {
                self.relationships.remove_relationship(contact_id)?;
                Ok(EngineResult::ContactRemoved { contact_id })
            }
            EngineCommand::ArchiveConversation { conversation_id, at } => {
                let mut conversation =
                    ConversationRepository::get(&self.relationships, conversation_id)
                        .map_err(map_error)?
                        .ok_or_else(|| EngineError("conversation not found".into()))?;
                conversation.archive(at).map_err(map_error)?;
                ConversationRepository::update(&mut self.relationships, conversation)
                    .map_err(map_error)?;
                Ok(EngineResult::ConversationUpdated { conversation_id })
            }
            EngineCommand::RestoreConversation { conversation_id, at } => {
                let mut conversation =
                    ConversationRepository::get(&self.relationships, conversation_id)
                        .map_err(map_error)?
                        .ok_or_else(|| EngineError("conversation not found".into()))?;
                conversation.restore(at).map_err(map_error)?;
                ConversationRepository::update(&mut self.relationships, conversation)
                    .map_err(map_error)?;
                Ok(EngineResult::ConversationUpdated { conversation_id })
            }
            EngineCommand::QueueMessage { message_id, conversation_id, body, reply_to, at } => {
                if ConversationRepository::get(&self.relationships, conversation_id)
                    .map_err(map_error)?
                    .is_none()
                {
                    return Err(EngineError("conversation not found".into()));
                }
                self.messages
                    .insert(Message::outbound(message_id, conversation_id, body, reply_to, at))
                    .map_err(map_error)?;
                Ok(EngineResult::MessageQueued { message_id })
            }
            EngineCommand::CancelMessage { message_id, at } => {
                let mut message = self
                    .messages
                    .get(message_id)
                    .map_err(map_error)?
                    .ok_or_else(|| EngineError("message not found".into()))?;
                message.cancel(at).map_err(map_error)?;
                self.messages.update(message).map_err(map_error)?;
                Ok(EngineResult::MessageUpdated { message_id })
            }
            EngineCommand::EditMessage { message_id, body, at } => {
                let mut message = self
                    .messages
                    .get(message_id)
                    .map_err(map_error)?
                    .ok_or_else(|| EngineError("message not found".into()))?;
                message.edit(body, at).map_err(map_error)?;
                self.messages.update(message).map_err(map_error)?;
                Ok(EngineResult::MessageUpdated { message_id })
            }
            EngineCommand::SetMessageReaction { reaction } => {
                let message_id = reaction.message_id();
                if self.messages.get(message_id).map_err(map_error)?.is_none() {
                    return Err(EngineError("message not found".into()));
                }
                self.messages.upsert_reaction(reaction).map_err(map_error)?;
                Ok(EngineResult::ReactionUpdated { message_id })
            }
            EngineCommand::BeginMessageSend { message_id, at } => {
                let mut message = self.load_message(message_id)?;
                let _ = message.begin_send(at).map_err(map_error)?;
                self.messages.update(message).map_err(map_error)?;
                Ok(EngineResult::MessageUpdated { message_id })
            }
            EngineCommand::MarkMessageSent { message_id, at } => {
                let mut message = self.load_message(message_id)?;
                message.mark_sent(at).map_err(map_error)?;
                self.messages.update(message).map_err(map_error)?;
                Ok(EngineResult::MessageUpdated { message_id })
            }
            EngineCommand::MarkMessageFailed { message_id, at, error_code } => {
                let mut message = self.load_message(message_id)?;
                message.mark_failed(at, error_code).map_err(map_error)?;
                self.messages.update(message).map_err(map_error)?;
                Ok(EngineResult::MessageUpdated { message_id })
            }
            EngineCommand::RetryMessage { message_id, at } => {
                let mut message = self.load_message(message_id)?;
                message.retry(at).map_err(map_error)?;
                self.messages.update(message).map_err(map_error)?;
                Ok(EngineResult::MessageUpdated { message_id })
            }
            EngineCommand::ApplyReceipt(receipt) => {
                let mut message = self.load_message(receipt.message_id)?;
                let changed = receipt.apply(&mut message).map_err(map_error)?;
                let _ = self.receipts.record(receipt).map_err(map_error)?;
                self.messages.update(message).map_err(map_error)?;
                Ok(EngineResult::ReceiptApplied { message_id: receipt.message_id, changed })
            }
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
            // Reactions belong to conversation history and are intentionally
            // omitted from the root overview projection.
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
