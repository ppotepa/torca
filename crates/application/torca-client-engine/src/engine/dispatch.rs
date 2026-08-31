// Responsibility: domain-oriented engine command dispatch.

impl<I, K, P, L, M, R> ClientEngine<I, K, P, L, M, R>
where
    I: IdentityRepository,
    K: IdentityKeyProvider,
    P: PairingRepository,
    L: RelationshipRepository,
    M: MessageRepository,
    R: ReceiptRepository,
{
    pub fn dispatch(&mut self, command: EngineCommand) -> Result<EngineResult, EngineError> {
        match command {
            command @ (EngineCommand::CreateIdentity { .. }
            | EngineCommand::UpdateProfile { .. }
            | EngineCommand::SetAvatarGenome { .. }) => self.dispatch_identity_profile(command),
            command @ (EngineCommand::StartPairing { .. }
            | EngineCommand::JoinPairing { .. }
            | EngineCommand::PeerJoined { .. }
            | EngineCommand::ApprovePairing { .. }
            | EngineCommand::RejectPairing { .. }
            | EngineCommand::CancelPairing { .. }
            | EngineCommand::ExpirePairing { .. }
            | EngineCommand::RemoteApproved { .. }
            | EngineCommand::CompletePairing { .. }
            | EngineCommand::RemovePairing { .. }) => self.dispatch_pairing(command),
            command @ (EngineCommand::EnsureConversation { .. }
            | EngineCommand::RemoveContact { .. }
            | EngineCommand::ArchiveConversation { .. }
            | EngineCommand::RestoreConversation { .. }) => self.dispatch_relationship(command),
            command => self.dispatch_messaging(command),
        }
    }

    fn dispatch_identity_profile(
        &mut self,
        command: EngineCommand,
    ) -> Result<EngineResult, EngineError> {
        match command {
            EngineCommand::CreateIdentity { identity_id, profile, at } => {
                let (_identity, _event) = self
                    .identity
                    .create(CreateIdentity { identity_id, profile, at })
                    .map_err(|_| EngineError::Identity)?;
                Ok(EngineResult::IdentityCreated)
            }
            EngineCommand::UpdateProfile { display_name, country_code, at } => {
                let profile = Profile::with_country(display_name, None, country_code)
                    .map_err(|_| EngineError::Identity)?;
                let (_identity, _event) = self
                    .identity
                    .update_profile(UpdateProfile { profile, at })
                    .map_err(|_| EngineError::Identity)?;
                Ok(EngineResult::ProfileUpdated)
            }
            EngineCommand::SetAvatarGenome { record, at } => {
                self.relationships.upsert_avatar_genome(record, at)?;
                Ok(EngineResult::ProfileUpdated)
            }
            _ => unreachable!("identity/profile dispatcher received a foreign command"),
        }
    }

    fn dispatch_pairing(&mut self, command: EngineCommand) -> Result<EngineResult, EngineError> {
        match command {
            EngineCommand::StartPairing { session_id, code, expires_at } => {
                self.pairings
                    .insert(PairingSession::creator(session_id, code, expires_at))
                    .map_err(|_| EngineError::Pairing)?;
                Ok(EngineResult::PairingStarted)
            }
            EngineCommand::JoinPairing { session_id, code, expires_at } => {
                self.pairings
                    .insert(PairingSession::joining(session_id, code, expires_at))
                    .map_err(|_| EngineError::Pairing)?;
                Ok(EngineResult::PairingJoined)
            }
            EngineCommand::PeerJoined { session_id, proposal, at } => {
                let mut session = self.load_pairing(session_id)?;
                session.peer_joined(proposal, at).map_err(|_| EngineError::Pairing)?;
                self.pairings.update(session).map_err(|_| EngineError::Repository)?;
                Ok(EngineResult::PairingUpdated)
            }
            EngineCommand::ApprovePairing { session_id, at } => {
                let mut session = self.load_pairing(session_id)?;
                session.approve_local(at).map_err(|_| EngineError::Pairing)?;
                self.pairings.update(session).map_err(|_| EngineError::Repository)?;
                Ok(EngineResult::PairingUpdated)
            }
            EngineCommand::RejectPairing { session_id } => {
                let mut session = self.load_pairing(session_id)?;
                session.reject().map_err(|_| EngineError::Pairing)?;
                self.pairings.update(session).map_err(|_| EngineError::Repository)?;
                Ok(EngineResult::PairingRejected)
            }
            EngineCommand::CancelPairing { session_id } => {
                let mut session = self.load_pairing(session_id)?;
                session.cancel().map_err(|_| EngineError::Pairing)?;
                self.pairings.update(session).map_err(|_| EngineError::Repository)?;
                Ok(EngineResult::PairingCancelled)
            }
            EngineCommand::ExpirePairing { session_id, at } => {
                let mut session = self.load_pairing(session_id)?;
                if !session.expire(at) {
                    return Err(EngineError::InvalidState);
                }
                self.pairings.update(session).map_err(|_| EngineError::Repository)?;
                Ok(EngineResult::PairingUpdated)
            }
            EngineCommand::RemoteApproved { session_id, at } => {
                let mut session = self.load_pairing(session_id)?;
                session.approve_remote(at).map_err(|_| EngineError::Pairing)?;
                self.pairings.update(session).map_err(|_| EngineError::Repository)?;
                Ok(EngineResult::PairingUpdated)
            }
            EngineCommand::CompletePairing {
                session_id,
                contact_id,
                conversation_id,
                display_name,
                country_code: _country_code,
                credential,
                at,
            } => {
                if credential.contact_id() != contact_id {
                    return Err(EngineError::InvalidState);
                }
                let existing_contact = ContactRepository::get(&self.relationships, contact_id)
                    .map_err(|_| EngineError::Repository)?;
                let existing_conversation =
                    ConversationRepository::get(&self.relationships, conversation_id)
                        .map_err(|_| EngineError::Repository)?;
                let existing_for_contact =
                    ConversationRepository::for_contact(&self.relationships, contact_id)
                        .map_err(|_| EngineError::Repository)?;
                let existing_credential = PeerCredentialRepository::credential_for_contact(
                    &self.relationships,
                    contact_id,
                )
                .map_err(|_| EngineError::Repository)?;
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
                    return Err(EngineError::Conflict);
                }
                let mut session = self.load_pairing(session_id)?;
                let proposal = session.complete(at).map_err(|_| EngineError::Pairing)?;
                let avatar = proposal.avatar.as_ref().map(|avatar| AvatarGenomeRecord {
                    genome_hash: avatar.genome_hash,
                    schema_version: avatar.schema_version,
                    generator_version: avatar.generator_version.clone(),
                    catalog_version: avatar.catalog_version.clone(),
                    compressed_genome: avatar.compressed_genome.clone(),
                });
                let contact = Contact::new(contact_id, proposal.public_identity, proposal.route, at);
                let mut contact = contact;
                contact.set_country_code(proposal.country_code);
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
                self.pairings.delete(session_id).map_err(|_| EngineError::Repository)?;
                Ok(EngineResult::PairingRemoved)
            }
            _ => unreachable!("pairing dispatcher received a foreign command"),
        }
    }

    fn dispatch_relationship(
        &mut self,
        command: EngineCommand,
    ) -> Result<EngineResult, EngineError> {
        match command {
            EngineCommand::EnsureConversation { contact_id, conversation_id, at } => {
                if ContactRepository::get(&self.relationships, contact_id)
                    .map_err(|_| EngineError::Repository)?
                    .is_none()
                {
                    return Err(EngineError::NotFound);
                }
                if let Some(existing) =
                    ConversationRepository::for_contact(&self.relationships, contact_id)
                        .map_err(|_| EngineError::Repository)?
                {
                    return Ok(EngineResult::ConversationStarted {
                        conversation_id: existing.id(),
                    });
                }
                if ConversationRepository::get(&self.relationships, conversation_id)
                    .map_err(|_| EngineError::Repository)?
                    .is_some()
                {
                    return Err(EngineError::Conflict);
                }
                ConversationRepository::insert(
                    &mut self.relationships,
                    DirectConversation::new(conversation_id, contact_id, at),
                )
                .map_err(|_| EngineError::Repository)?;
                Ok(EngineResult::ConversationStarted { conversation_id })
            }
            EngineCommand::RemoveContact { contact_id } => {
                self.relationships.remove_relationship(contact_id)?;
                Ok(EngineResult::ContactRemoved { contact_id })
            }
            EngineCommand::ArchiveConversation { conversation_id, at } => {
                let mut conversation = ConversationRepository::get(&self.relationships, conversation_id)
                    .map_err(|_| EngineError::Repository)?
                    .ok_or(EngineError::NotFound)?;
                conversation.archive(at).map_err(|_| EngineError::InvalidState)?;
                ConversationRepository::update(&mut self.relationships, conversation)
                    .map_err(|_| EngineError::Repository)?;
                Ok(EngineResult::ConversationUpdated { conversation_id })
            }
            EngineCommand::RestoreConversation { conversation_id, at } => {
                let mut conversation = ConversationRepository::get(&self.relationships, conversation_id)
                    .map_err(|_| EngineError::Repository)?
                    .ok_or(EngineError::NotFound)?;
                conversation.restore(at).map_err(|_| EngineError::InvalidState)?;
                ConversationRepository::update(&mut self.relationships, conversation)
                    .map_err(|_| EngineError::Repository)?;
                Ok(EngineResult::ConversationUpdated { conversation_id })
            }
            _ => unreachable!("relationship dispatcher received a foreign command"),
        }
    }

    fn dispatch_messaging(&mut self, command: EngineCommand) -> Result<EngineResult, EngineError> {
        match command {
            EngineCommand::QueueMessage { message_id, conversation_id, body, reply_to, at } => {
                if ConversationRepository::get(&self.relationships, conversation_id)
                    .map_err(|_| EngineError::Repository)?
                    .is_none()
                {
                    return Err(EngineError::NotFound);
                }
                self.messages
                    .insert(Message::outbound(message_id, conversation_id, body, reply_to, at))
                    .map_err(|_| EngineError::Repository)?;
                Ok(EngineResult::MessageQueued { message_id })
            }
            EngineCommand::CancelMessage { message_id, at } => {
                let mut message = self.messages.get(message_id)
                    .map_err(|_| EngineError::Repository)?
                    .ok_or(EngineError::NotFound)?;
                message.cancel(at).map_err(|_| EngineError::Messaging)?;
                self.messages.update(message).map_err(|_| EngineError::Repository)?;
                Ok(EngineResult::MessageUpdated { message_id })
            }
            EngineCommand::DeleteMessage { message_id, at } => {
                let mut message = self.messages.get(message_id)
                    .map_err(|_| EngineError::Repository)?
                    .ok_or(EngineError::NotFound)?;
                message.delete(at).map_err(|_| EngineError::Messaging)?;
                self.messages.update(message).map_err(|_| EngineError::Repository)?;
                Ok(EngineResult::MessageUpdated { message_id })
            }
            EngineCommand::ApplyMessageDeletion { message_id, at } => {
                let mut message = self.messages.get(message_id)
                    .map_err(|_| EngineError::Repository)?
                    .ok_or(EngineError::NotFound)?;
                if message.direction() != MessageDirection::Inbound {
                    return Err(EngineError::InvalidState);
                }
                message.apply_deletion(at);
                self.messages.update(message).map_err(|_| EngineError::Repository)?;
                Ok(EngineResult::MessageUpdated { message_id })
            }
            EngineCommand::EditMessage { message_id, body, at } => {
                let mut message = self.messages.get(message_id)
                    .map_err(|_| EngineError::Repository)?
                    .ok_or(EngineError::NotFound)?;
                message.edit(body, at).map_err(|_| EngineError::Messaging)?;
                self.messages.update(message).map_err(|_| EngineError::Repository)?;
                Ok(EngineResult::MessageUpdated { message_id })
            }
            EngineCommand::SetMessageReaction { reaction } => {
                let message_id = reaction.message_id();
                if self.messages.get(message_id)
                    .map_err(|_| EngineError::Repository)?
                    .is_none()
                {
                    return Err(EngineError::NotFound);
                }
                self.messages.upsert_reaction(reaction).map_err(|_| EngineError::Repository)?;
                Ok(EngineResult::ReactionUpdated { message_id })
            }
            EngineCommand::BeginMessageSend { message_id, at } => {
                let mut message = self.load_message(message_id)?;
                let _ = message.begin_send(at).map_err(|_| EngineError::Messaging)?;
                self.messages.update(message).map_err(|_| EngineError::Repository)?;
                Ok(EngineResult::MessageUpdated { message_id })
            }
            EngineCommand::MarkMessageSent { message_id, at } => {
                let mut message = self.load_message(message_id)?;
                message.mark_sent(at).map_err(|_| EngineError::Messaging)?;
                self.messages.update(message).map_err(|_| EngineError::Repository)?;
                Ok(EngineResult::MessageUpdated { message_id })
            }
            EngineCommand::MarkMessageFailed { message_id, at, error_code } => {
                let mut message = self.load_message(message_id)?;
                message.mark_failed(at, error_code).map_err(|_| EngineError::Messaging)?;
                self.messages.update(message).map_err(|_| EngineError::Repository)?;
                Ok(EngineResult::MessageUpdated { message_id })
            }
            EngineCommand::RetryMessage { message_id, at } => {
                let mut message = self.load_message(message_id)?;
                message.retry(at).map_err(|_| EngineError::Messaging)?;
                self.messages.update(message).map_err(|_| EngineError::Repository)?;
                Ok(EngineResult::MessageUpdated { message_id })
            }
            EngineCommand::ApplyReceipt(receipt) => {
                let mut message = self.load_message(receipt.message_id)?;
                let changed = receipt.apply(&mut message).map_err(|_| EngineError::Messaging)?;
                let _ = self.receipts.record(receipt).map_err(|_| EngineError::Repository)?;
                self.messages.update(message).map_err(|_| EngineError::Repository)?;
                Ok(EngineResult::ReceiptApplied { message_id: receipt.message_id, changed })
            }
            _ => unreachable!("messaging dispatcher received a foreign command"),
        }
    }
}
