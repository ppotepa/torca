fn maybe_send_completion(
    &mut self,
    session_id: PairingSessionId,
) -> Result<(), PairingRuntimeError> {
    if self.completion_sent.contains(&session_id) {
        return Ok(());
    }
    let session = self.session(session_id)?;
    if !session.local_approved() || !session.remote_approved() {
        return Ok(());
    }
    let digest = self.transcript_digest(session_id, session.role())?;
    self.coordinator.push(
        session_id,
        &PairingEnvelope {
            pairing_id: self.coordinator.context(session_id)?.0,
            payload: PairingPayload::Completion(PairingCompletion {
                transcript_digest: digest,
            }),
        },
    )?;
    self.completion_sent.insert(session_id);
    Ok(())
}

fn apply_completion(
    &mut self,
    session_id: PairingSessionId,
    completion: PairingCompletion,
    now: Timestamp,
) -> Result<PairingCompletedContact, PairingRuntimeError> {
    let session = self.session(session_id)?;
    let digest = self.transcript_digest(session_id, session.role())?;
    if completion.transcript_digest != digest {
        return Err(PairingRuntimeError::InvalidCompletion);
    }
    let display_name = self.remote_display_name(session_id)?;
    let context = self.coordinator.context(session_id)?;
    let contact_id = ContactId::from_opaque(context.0);
    let conversation_id = ConversationId::from_opaque(context.0);
    if self.completion_applied.contains(&session_id) {
        return Ok(PairingCompletedContact { contact_id, conversation_id, display_name });
    }
    let overview = self.engine.overview_snapshot().map_err(|_| PairingRuntimeError::Engine)?;
    let already_completed = overview.contacts.iter().any(|contact| contact.id() == contact_id)
        && overview
            .conversations
            .iter()
            .any(|conversation| conversation.id() == conversation_id);
    if already_completed {
        self.completion_applied.insert(session_id);
        return Ok(PairingCompletedContact { contact_id, conversation_id, display_name });
    }
    if !session.can_complete(now) {
        return Err(PairingRuntimeError::InvalidCompletion);
    }
    let secret = self.coordinator.derive_peer_secret(session_id, digest)?;
    let secret_handle = self.peer_secrets.store_peer_secret(secret)?;
    let local_capability_id = self.local_capability_id(session_id)?;
    let credential = PeerCredential::new(contact_id, local_capability_id, secret_handle)
        .map_err(|_| PairingRuntimeError::InvalidCompletion)?;
    let durable = self.engine.dispatch(EngineCommand::CompletePairing {
        session_id,
        contact_id,
        conversation_id,
        display_name: display_name.clone(),
        credential,
        at: now,
    });
    match durable {
        Ok(EngineResult::PairingCompleted { .. }) => {
            self.completion_applied.insert(session_id);
            Ok(PairingCompletedContact { contact_id, conversation_id, display_name })
        }
        Ok(_) | Err(_) => {
            let _ = self.peer_secrets.delete_peer_secret(secret_handle);
            Err(PairingRuntimeError::Engine)
        }
    }
}

fn send_completion_ack(
    &mut self,
    session_id: PairingSessionId,
    transcript_digest: [u8; 32],
) -> Result<(), PairingRuntimeError> {
    if self.completion_ack_sent.contains(&session_id) {
        return Ok(());
    }
    self.coordinator.push(
        session_id,
        &PairingEnvelope {
            pairing_id: self.coordinator.context(session_id)?.0,
            payload: PairingPayload::CompletionAck(PairingCompletionAck { transcript_digest }),
        },
    )?;
    self.completion_ack_sent.insert(session_id);
    Ok(())
}

fn apply_completion_ack(
    &mut self,
    session_id: PairingSessionId,
    ack: PairingCompletionAck,
) -> Result<(), PairingRuntimeError> {
    let session = self.session(session_id)?;
    let digest = self.transcript_digest(session_id, session.role())?;
    if ack.transcript_digest != digest || !self.completion_applied.contains(&session_id) {
        return Err(PairingRuntimeError::InvalidCompletion);
    }
    Ok(())
}

fn transcript_digest(
    &self,
    session_id: PairingSessionId,
    role: PairingRole,
) -> Result<[u8; 32], PairingRuntimeError> {
    let (creator, joiner) = self.ordered_offers(session_id, role)?;
    self.approval.transcript_digest(&creator, &joiner).map_err(Into::into)
}

fn local_capability_id(
    &self,
    session_id: PairingSessionId,
) -> Result<OpaqueId, PairingRuntimeError> {
    let envelope =
        self.local_offers.get(&session_id).ok_or(PairingRuntimeError::InvalidOffer)?;
    match &envelope.payload {
        PairingPayload::Offer(offer) if !offer.capability_id.is_nil() => {
            Ok(offer.capability_id)
        }
        _ => Err(PairingRuntimeError::InvalidOffer),
    }
}

fn remote_display_name(
    &self,
    session_id: PairingSessionId,
) -> Result<String, PairingRuntimeError> {
    let envelope =
        self.remote_offers.get(&session_id).ok_or(PairingRuntimeError::InvalidOffer)?;
    match &envelope.payload {
        PairingPayload::Offer(offer) => Ok(offer.display_name.clone()),
        _ => Err(PairingRuntimeError::InvalidOffer),
    }
}

fn cleanup_terminal(&mut self, session_id: PairingSessionId) {
    let _ = self.engine.dispatch(EngineCommand::RemovePairing { session_id });
    let _ = self.close_transport(session_id);
}

fn cleanup_completed(&mut self, session_id: PairingSessionId) {
    let _ = self.engine.dispatch(EngineCommand::RemovePairing { session_id });
    self.local_offers.remove(&session_id);
    self.remote_offers.remove(&session_id);
    self.completion_sent.remove(&session_id);
    self.completion_applied.remove(&session_id);
    self.completion_ack_sent.remove(&session_id);
    let _ = self.peer_secrets.delete_pairing_state(session_id);
    let _ = self.coordinator.detach(session_id);
}

fn discard_unrecoverable_session(&mut self, session_id: PairingSessionId) {
    let _ = self.engine.dispatch(EngineCommand::RemovePairing { session_id });
}

fn persist_session(&mut self, session_id: PairingSessionId) -> Result<(), PairingRuntimeError> {
    let transport = self.coordinator.export_transport(session_id)?;
    let mut encoded = encode_persisted_state(
        transport,
        self.local_offers.get(&session_id),
        self.remote_offers.get(&session_id),
        self.completion_sent.contains(&session_id),
        self.completion_applied.contains(&session_id),
        self.completion_ack_sent.contains(&session_id),
    )?;
    let stored = self.peer_secrets.store_pairing_state(session_id, &encoded);
    encoded.fill(0);
    stored.map_err(Into::into)
}

fn session(&self, session_id: PairingSessionId) -> Result<PairingSession, PairingRuntimeError> {
    self.engine
        .overview_snapshot()
        .map_err(|_| PairingRuntimeError::Engine)?
        .pairings
        .into_iter()
        .find(|session| session.id() == session_id)
        .ok_or(PairingRuntimeError::SessionNotFound)
}

fn ordered_offers(
    &self,
    session_id: PairingSessionId,
    role: PairingRole,
) -> Result<(PairingEnvelope, PairingEnvelope), PairingRuntimeError> {
    let local =
        self.local_offers.get(&session_id).cloned().ok_or(PairingRuntimeError::InvalidOffer)?;
    let remote = self
        .remote_offers
        .get(&session_id)
        .cloned()
        .ok_or(PairingRuntimeError::InvalidOffer)?;
    Ok(match role {
        PairingRole::Creator => (local, remote),
        PairingRole::Joiner => (remote, local),
    })
}

fn remote_identity(
    &self,
    session_id: PairingSessionId,
) -> Result<PublicIdentity, PairingRuntimeError> {
    let envelope =
        self.remote_offers.get(&session_id).ok_or(PairingRuntimeError::InvalidOffer)?;
    match &envelope.payload {
        PairingPayload::Offer(offer) => Ok(peer_proposal(offer)?.public_identity),
        _ => Err(PairingRuntimeError::InvalidOffer),
    }
}

fn local_offer(
    &mut self,
    context: crate::PairingContextId,
    local: LocalPairingContext,
) -> Result<PairingEnvelope, PairingRuntimeError> {
    if local.capability_id.is_nil() {
        return Err(PairingRuntimeError::InvalidOffer);
    }
    let mut transcript_nonce = [0_u8; 32];
    self.coordinator.crypto.fill_random(&mut transcript_nonce)?;
    let key = local.public_identity.key();
    let key_algorithm = match key.algorithm() {
        KeyAlgorithm::Ed25519 => 1,
    };
    let offer = PairingOffer {
        identity_id: local.public_identity.identity_id().to_opaque(),
        key_id: key.key_id().to_opaque(),
        key_algorithm,
        public_key: key.public_key().to_vec(),
        key_generation: local.public_identity.generation(),
        display_name: local.display_name,
        onion_address: local.onion_address,
        capability_id: local.capability_id,
        transcript_nonce,
        avatar: local.avatar,
    };
    offer.validate().map_err(|_| PairingRuntimeError::InvalidOffer)?;
    Ok(PairingEnvelope {
        pairing_id: context.0,
        payload: PairingPayload::Offer(Box::new(offer)),
    })
}
