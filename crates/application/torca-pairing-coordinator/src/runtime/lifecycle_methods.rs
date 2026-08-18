impl<R, C, A, S> PairingRuntime<R, C, A, S>
where
    R: PairingRendezvousPort,
    C: PairingCryptoPort,
    A: PairingApprovalPort,
    S: PairingPeerSecretStore,
{
pub const fn new(
    coordinator: PairingCoordinator<R, C>,
    engine: EngineHandle,
    approval: A,
    peer_secrets: S,
) -> Self {
    Self {
        coordinator,
        engine,
        approval,
        peer_secrets,
        local_offers: BTreeMap::new(),
        remote_offers: BTreeMap::new(),
        completion_sent: BTreeSet::new(),
        completion_applied: BTreeSet::new(),
        completion_ack_sent: BTreeSet::new(),
    }
}

pub fn restore_active_sessions(&mut self) -> Result<usize, PairingRuntimeError> {
    let sessions =
        self.engine.overview_snapshot().map_err(|_| PairingRuntimeError::Engine)?.pairings;
    let mut restored = 0;
    for session in sessions {
        let mut state = match self.peer_secrets.load_pairing_state(session.id())? {
            Some(state) => state,
            None => {
                self.discard_unrecoverable_session(session.id());
                continue;
            }
        };
        let persisted = decode_persisted_state(session.id(), &state);
        state.fill(0);
        let persisted = match persisted {
            Ok(persisted) => persisted,
            Err(_) => {
                let _ = self.peer_secrets.delete_pairing_state(session.id());
                self.discard_unrecoverable_session(session.id());
                continue;
            }
        };
        if persisted.transport.role != session.role() {
            let _ = self.peer_secrets.delete_pairing_state(session.id());
            self.discard_unrecoverable_session(session.id());
            continue;
        }
        if self.coordinator.restore_transport(session.id(), persisted.transport).is_err() {
            let _ = self.peer_secrets.delete_pairing_state(session.id());
            self.discard_unrecoverable_session(session.id());
            continue;
        }
        if let Some(offer) = persisted.local_offer {
            self.local_offers.insert(session.id(), offer);
        }
        if let Some(offer) = persisted.remote_offer {
            self.remote_offers.insert(session.id(), offer);
        }
        if persisted.completion_sent {
            self.completion_sent.insert(session.id());
        }
        if persisted.completion_applied {
            self.completion_applied.insert(session.id());
        }
        if persisted.completion_ack_sent {
            self.completion_ack_sent.insert(session.id());
        }
        restored += 1;
    }
    Ok(restored)
}

pub fn create_invitation(
    &mut self,
    session_id: PairingSessionId,
    local: LocalPairingContext,
    now: Timestamp,
) -> Result<PairingInvitation, PairingRuntimeError> {
    let code = self.coordinator.generate_pairing_code()?;
    let ticket = self.coordinator.generate_pairing_ticket()?;
    let requested_expires_at = invitation_expires_at(now)?;
    let (_, expires_at) = self.coordinator.open_creator(
        session_id,
        &code,
        requested_expires_at,
        *ticket.as_bytes(),
    )?;
    let context = self.coordinator.context(session_id)?;
    let local_offer = self.local_offer(context, local)?;
    if self
        .engine
        .dispatch(EngineCommand::StartPairing { session_id, code: code.clone(), expires_at })
        .is_err()
    {
        let _ = self.coordinator.close(session_id);
        return Err(PairingRuntimeError::Engine);
    }
    self.local_offers.insert(session_id, local_offer);
    self.persist_session(session_id)?;
    Ok(PairingInvitation {
        session_id,
        uri: encode_invite_uri(&code, Some(&ticket)),
        code,
        expires_at,
        ticket,
    })
}

pub fn create_invitation_pending_route(
    &mut self,
    session_id: PairingSessionId,
    now: Timestamp,
) -> Result<PairingInvitation, PairingRuntimeError> {
    let code = self.coordinator.generate_pairing_code()?;
    let ticket = self.coordinator.generate_pairing_ticket()?;
    let requested_expires_at = invitation_expires_at(now)?;
    let (_, expires_at) = self.coordinator.open_creator(
        session_id,
        &code,
        requested_expires_at,
        *ticket.as_bytes(),
    )?;
    if self
        .engine
        .dispatch(EngineCommand::StartPairing { session_id, code: code.clone(), expires_at })
        .is_err()
    {
        let _ = self.coordinator.close(session_id);
        return Err(PairingRuntimeError::Engine);
    }
    self.persist_session(session_id)?;
    Ok(PairingInvitation {
        session_id,
        uri: encode_invite_uri(&code, Some(&ticket)),
        code,
        expires_at,
        ticket,
    })
}

pub fn join_invitation(
    &mut self,
    session_id: PairingSessionId,
    code: PairingCode,
    local: LocalPairingContext,
    _now: Timestamp,
    ticket: Option<[u8; 16]>,
) -> Result<(), PairingRuntimeError> {
    let (_, expires_at) = self.coordinator.join(session_id, &code, ticket)?;
    let context = self.coordinator.context(session_id)?;
    let local_offer = self.local_offer(context, local)?;
    self.coordinator.push(session_id, &local_offer)?;
    if self
        .engine
        .dispatch(EngineCommand::JoinPairing { session_id, code, expires_at })
        .is_err()
    {
        let _ = self.coordinator.close(session_id);
        return Err(PairingRuntimeError::Engine);
    }
    self.local_offers.insert(session_id, local_offer);
    self.persist_session(session_id)?;
    Ok(())
}

pub fn join_invitation_pending_route(
    &mut self,
    session_id: PairingSessionId,
    code: PairingCode,
    ticket: Option<[u8; 16]>,
) -> Result<(), PairingRuntimeError> {
    let (_, expires_at) = self.coordinator.join(session_id, &code, ticket)?;
    if self
        .engine
        .dispatch(EngineCommand::JoinPairing { session_id, code, expires_at })
        .is_err()
    {
        let _ = self.coordinator.close(session_id);
        return Err(PairingRuntimeError::Engine);
    }
    self.persist_session(session_id)
}

pub fn publish_local_offer(
    &mut self,
    session_id: PairingSessionId,
    local: LocalPairingContext,
) -> Result<bool, PairingRuntimeError> {
    if self.local_offers.contains_key(&session_id) {
        return Ok(false);
    }
    let role = self.session(session_id)?.role();
    let context = self.coordinator.context(session_id)?;
    let local_offer = self.local_offer(context, local)?;
    if role == PairingRole::Joiner {
        self.coordinator.push(session_id, &local_offer)?;
    }
    self.local_offers.insert(session_id, local_offer);
    self.persist_session(session_id)?;
    Ok(true)
}

pub fn approve(
    &mut self,
    session_id: PairingSessionId,
    now: Timestamp,
) -> Result<(), PairingRuntimeError> {
    let session = self.session(session_id)?;
    if session.role() != PairingRole::Creator {
        return Err(PairingRuntimeError::CreatorApprovalRequired);
    }
    self.record_local_approval(session_id, now)
}

fn approve_joiner_consent(
    &mut self,
    session_id: PairingSessionId,
    now: Timestamp,
) -> Result<(), PairingRuntimeError> {
    let session = self.session(session_id)?;
    if session.role() != PairingRole::Joiner {
        return Err(PairingRuntimeError::CreatorApprovalRequired);
    }
    self.record_local_approval(session_id, now)
}

fn record_local_approval(
    &mut self,
    session_id: PairingSessionId,
    now: Timestamp,
) -> Result<(), PairingRuntimeError> {
    let session = self.session(session_id)?;
    let digest = self.transcript_digest(session_id, session.role())?;
    let identity = self
        .engine
        .overview_snapshot()
        .map_err(|_| PairingRuntimeError::Engine)?
        .identity
        .ok_or(PairingRuntimeError::IdentityMissing)?;
    let proof = self.approval.sign_approval(
        identity.public().key().key_id(),
        self.coordinator.context(session_id)?.0,
        digest,
    )?;
    self.coordinator.push(
        session_id,
        &PairingEnvelope {
            pairing_id: self.coordinator.context(session_id)?.0,
            payload: PairingPayload::Approval(PairingApproval {
                transcript_digest: digest,
                proof,
            }),
        },
    )?;
    if !session.local_approved() {
        let _ = self
            .engine
            .dispatch(EngineCommand::ApprovePairing { session_id, at: now })
            .map_err(|_| PairingRuntimeError::Engine)?;
    }
    self.maybe_send_completion(session_id)?;
    self.persist_session(session_id)?;
    Ok(())
}

pub fn reject(&mut self, session_id: PairingSessionId) -> Result<(), PairingRuntimeError> {
    let session = self.session(session_id)?;
    if session.state() == PairingState::Rejected {
        return Ok(());
    }
    self.coordinator.push(
        session_id,
        &PairingEnvelope {
            pairing_id: self.coordinator.context(session_id)?.0,
            payload: PairingPayload::Rejection(PairingRejection),
        },
    )?;
    let _ = self
        .engine
        .dispatch(EngineCommand::RejectPairing { session_id })
        .map_err(|_| PairingRuntimeError::Engine)?;
    self.cleanup_terminal(session_id);
    Ok(())
}

pub fn cancel(&mut self, session_id: PairingSessionId) -> Result<(), PairingRuntimeError> {
    let session = self.session(session_id)?;
    if session.state() == PairingState::Cancelled {
        return Ok(());
    }
    self.coordinator.push(
        session_id,
        &PairingEnvelope {
            pairing_id: self.coordinator.context(session_id)?.0,
            payload: PairingPayload::Cancellation(PairingCancellation),
        },
    )?;
    let _ = self
        .engine
        .dispatch(EngineCommand::CancelPairing { session_id })
        .map_err(|_| PairingRuntimeError::Engine)?;
    self.cleanup_terminal(session_id);
    Ok(())
}
}
