pub fn maintenance(&mut self, now: Timestamp) -> Result<usize, PairingRuntimeError> {
    let due = self
        .engine
        .overview_snapshot()
        .map_err(|_| PairingRuntimeError::Engine)?
        .pairings
        .into_iter()
        .filter(|session| !is_terminal(session.state()) && now >= session.expires_at())
        .map(|session| session.id())
        .collect::<Vec<_>>();
    for id in &due {
        let _ = self
            .engine
            .dispatch(EngineCommand::ExpirePairing { session_id: *id, at: now })
            .map_err(|_| PairingRuntimeError::Engine)?;
        self.cleanup_terminal(*id);
    }
    Ok(due.len())
}

pub fn poll(
    &mut self,
    session_id: PairingSessionId,
    now: Timestamp,
) -> Result<PairingPollReport, PairingRuntimeError> {
    let role = self.session(session_id)?.role();
    let batch = self.coordinator.poll(session_id)?;
    let mut report = PairingPollReport::default();
    let mut cleanup_terminal_after_ack = false;
    let mut cleanup_completed_after_ack = false;
    for envelope in batch.envelopes {
        match &envelope.payload {
            PairingPayload::Offer(offer) => {
                if let Some(existing) = self.remote_offers.get(&session_id) {
                    if existing == &envelope {
                        continue;
                    }
                    return Err(PairingRuntimeError::InvalidOffer);
                }
                let proposal = peer_proposal(offer)?;
                self.remote_offers.insert(session_id, envelope);
                self.persist_session(session_id)?;
                let _ = self
                    .engine
                    .dispatch(EngineCommand::PeerJoined { session_id, proposal, at: now })
                    .map_err(|_| PairingRuntimeError::Engine)?;
                report.offers_applied += 1;
                if role == PairingRole::Creator {
                    let local = self
                        .local_offers
                        .get(&session_id)
                        .cloned()
                        .ok_or(PairingRuntimeError::InvalidOffer)?;
                    self.coordinator.push(session_id, &local)?;
                } else {
                    self.approve_joiner_consent(session_id, now)?;
                }
            }
            PairingPayload::Approval(remote) => {
                let digest = self.transcript_digest(session_id, role)?;
                if remote.transcript_digest != digest {
                    return Err(PairingRuntimeError::Approval(
                        PairingApprovalError::InvalidTranscript,
                    ));
                }
                let identity = self.remote_identity(session_id)?;
                self.approval.verify_approval(
                    &identity,
                    self.coordinator.context(session_id)?.0,
                    digest,
                    &remote.proof,
                )?;
                let session = self.session(session_id)?;
                if !session.remote_approved() {
                    let _ = self
                        .engine
                        .dispatch(EngineCommand::RemoteApproved { session_id, at: now })
                        .map_err(|_| PairingRuntimeError::Engine)?;
                    report.approvals_applied += 1;
                }
                self.maybe_send_completion(session_id)?;
            }
            PairingPayload::Completion(completion) => {
                let completed = self.apply_completion(session_id, *completion, now)?;
                self.send_completion_ack(session_id, completion.transcript_digest)?;
                report.completions_applied += 1;
                report.completed_contact = Some(completed);
            }
            PairingPayload::CompletionAck(ack) => {
                self.apply_completion_ack(session_id, *ack)?;
                report.completion_acks_applied += 1;
                cleanup_completed_after_ack = true;
                break;
            }
            PairingPayload::Rejection(_) => {
                let session = self.session(session_id)?;
                if session.state() != PairingState::Rejected {
                    let _ = self
                        .engine
                        .dispatch(EngineCommand::RejectPairing { session_id })
                        .map_err(|_| PairingRuntimeError::Engine)?;
                    report.rejections_applied += 1;
                }
                cleanup_terminal_after_ack = true;
                break;
            }
            PairingPayload::Cancellation(_) => {
                let session = self.session(session_id)?;
                if session.state() != PairingState::Cancelled {
                    let _ = self
                        .engine
                        .dispatch(EngineCommand::CancelPairing { session_id })
                        .map_err(|_| PairingRuntimeError::Engine)?;
                    report.cancellations_applied += 1;
                }
                cleanup_terminal_after_ack = true;
                break;
            }
        }
    }
    if self.local_offers.contains_key(&session_id) {
        self.persist_session(session_id)?;
    }
    if let Some(received_through) = batch.received_through {
        self.coordinator.ack(session_id, received_through)?;
    }
    if cleanup_completed_after_ack {
        self.cleanup_completed(session_id);
    } else if cleanup_terminal_after_ack {
        self.cleanup_terminal(session_id);
    }
    Ok(report)
}

pub fn close_transport(
    &mut self,
    session_id: PairingSessionId,
) -> Result<(), PairingRuntimeError> {
    self.local_offers.remove(&session_id);
    self.remote_offers.remove(&session_id);
    self.completion_sent.remove(&session_id);
    self.completion_applied.remove(&session_id);
    self.completion_ack_sent.remove(&session_id);
    let _ = self.peer_secrets.delete_pairing_state(session_id);
    self.coordinator.close(session_id).map_err(Into::into)
}

pub fn network_changed(&mut self) {
    self.coordinator.network_changed();
}

pub fn into_parts(self) -> (PairingCoordinator<R, C>, EngineHandle, A, S) {
    (self.coordinator, self.engine, self.approval, self.peer_secrets)
}
