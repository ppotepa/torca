use std::collections::{BTreeMap, BTreeSet};

use torca_client_engine::{EngineCommand, EngineHandle, EngineResult};
use torca_contacts::{ContactId, ContactRoute, PeerCredential};
use torca_conversations::ConversationId;
use torca_foundation::{OpaqueId, Timestamp};
use torca_identity::{IdentityKey, KeyAlgorithm, PublicIdentity};
use torca_pairing::{
    AvatarGenomeReference, PairingCode, PairingRole, PairingSession, PairingSessionId,
    PairingState, PeerProposal,
};
use torca_pairing_protocol::{
    AvatarEnvelope, PairingApproval, PairingCancellation, PairingCompletion, PairingCompletionAck,
    PairingEnvelope, PairingInviteTicket, PairingOffer, PairingPayload, PairingRejection,
};

use crate::{
    PairingApprovalError, PairingApprovalPort, PairingCoordinator, PairingCoordinatorError,
    PairingCredentialError, PairingCryptoPort, PairingPeerSecretStore, PairingRendezvousPort,
    PairingSideToken, PairingSlotCapability, PairingSlotId, PairingTransportSnapshot,
    encode_invite_uri, invitation_expires_at,
};

const PAIRING_STATE_VERSION: u8 = 2;

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalPairingContext {
    pub public_identity: PublicIdentity,
    pub display_name: String,
    pub onion_address: String,
    pub capability_id: OpaqueId,
    /// Immutable avatar genome exchanged with the signed offer when available.
    pub avatar: Option<AvatarEnvelope>,
}

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairingInvitation {
    pub session_id: PairingSessionId,
    pub code: PairingCode,
    pub uri: String,
    pub expires_at: Timestamp,
    pub ticket: PairingInviteTicket,
}

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairingCompletedContact {
    pub contact_id: ContactId,
    pub conversation_id: ConversationId,
    pub display_name: String,
}

#[must_use]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PairingPollReport {
    pub offers_applied: usize,
    pub approvals_applied: usize,
    pub completions_applied: usize,
    pub completion_acks_applied: usize,
    pub rejections_applied: usize,
    pub cancellations_applied: usize,
    pub completed_contact: Option<PairingCompletedContact>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PairingRuntimeError {
    Coordinator(PairingCoordinatorError),
    Approval(PairingApprovalError),
    Credential(PairingCredentialError),
    Engine,
    IdentityMissing,
    InvalidOffer,
    InvalidCompletion,
    UnsupportedAlgorithm,
    CreatorApprovalRequired,
    SessionNotFound,
}
impl core::fmt::Display for PairingRuntimeError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for PairingRuntimeError {}
impl From<PairingCoordinatorError> for PairingRuntimeError {
    fn from(value: PairingCoordinatorError) -> Self {
        Self::Coordinator(value)
    }
}
impl From<PairingApprovalError> for PairingRuntimeError {
    fn from(value: PairingApprovalError) -> Self {
        Self::Approval(value)
    }
}
impl From<PairingCredentialError> for PairingRuntimeError {
    fn from(value: PairingCredentialError) -> Self {
        Self::Credential(value)
    }
}

pub struct PairingRuntime<R, C, A, S> {
    coordinator: PairingCoordinator<R, C>,
    engine: EngineHandle,
    approval: A,
    peer_secrets: S,
    local_offers: BTreeMap<PairingSessionId, PairingEnvelope>,
    remote_offers: BTreeMap<PairingSessionId, PairingEnvelope>,
    completion_sent: BTreeSet<PairingSessionId>,
    completion_applied: BTreeSet<PairingSessionId>,
    completion_ack_sent: BTreeSet<PairingSessionId>,
}

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

    /// Reconstructs in-flight sessions from the durable engine snapshot and the platform's
    /// protected-secret store. This is called once while composing the process runtime, before
    /// its maintenance loop can poll the relay.
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

    /// Reserves a creator slot and persists the pairing session without
    /// requiring the local onion route to be available yet. The caller must
    /// publish the local offer with [`Self::publish_local_offer`] once the
    /// onion service becomes reachable.
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

    /// Joins the relay slot without requiring the local onion route. The
    /// authenticated offer is sent later by [`Self::publish_local_offer`].
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

    /// Creates and durably records the local authenticated offer exactly once.
    /// Joiners publish immediately; creators publish in response to the remote
    /// offer, preserving the existing rendezvous protocol ordering.
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

    /// Joining an invitation is the joiner's explicit consent.  It is recorded
    /// as soon as the creator's authenticated offer has been verified; only the
    /// creator is subsequently presented with an approval decision.
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
                    // The relay delivery is destructive. Record the authenticated offer in the
                    // protected restart state before advancing the durable domain aggregate so a
                    // crash cannot consume the only copy of the transcript.
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
        // A relay slot without its protected transport key cannot be resumed safely. Removing
        // the local summary is preferable to leaving a permanently pending invitation; the
        // relay's five-minute TTL removes the counterpart automatically.
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
}

fn is_terminal(state: PairingState) -> bool {
    matches!(
        state,
        PairingState::Rejected
            | PairingState::Cancelled
            | PairingState::Expired
            | PairingState::Completed
    )
}

fn peer_proposal(offer: &PairingOffer) -> Result<PeerProposal, PairingRuntimeError> {
    let algorithm = match offer.key_algorithm {
        1 => KeyAlgorithm::Ed25519,
        _ => return Err(PairingRuntimeError::UnsupportedAlgorithm),
    };
    let key = IdentityKey::new(
        torca_identity::KeyId::from_opaque(offer.key_id),
        algorithm,
        offer.public_key.clone(),
    )
    .map_err(|_| PairingRuntimeError::InvalidOffer)?;
    let public_identity = PublicIdentity::new(
        torca_identity::IdentityId::from_opaque(offer.identity_id),
        key,
        offer.key_generation,
    );
    let route = ContactRoute::new(offer.onion_address.clone(), offer.capability_id)
        .map_err(|_| PairingRuntimeError::InvalidOffer)?;
    Ok(PeerProposal {
        public_identity,
        display_name: offer.display_name.clone(),
        route,
        avatar: offer.avatar.as_ref().map(|avatar| AvatarGenomeReference {
            schema_version: avatar.schema,
            generator_version: avatar.generator_version.clone(),
            catalog_version: avatar.catalog_version.clone(),
            genome_hash: avatar.genome_hash,
            compressed_genome: avatar.compressed_genome.clone(),
        }),
    })
}

struct PersistedPairingState {
    transport: PairingTransportSnapshot,
    local_offer: Option<PairingEnvelope>,
    remote_offer: Option<PairingEnvelope>,
    completion_sent: bool,
    completion_applied: bool,
    completion_ack_sent: bool,
}

fn encode_persisted_state(
    transport: PairingTransportSnapshot,
    local_offer: Option<&PairingEnvelope>,
    remote_offer: Option<&PairingEnvelope>,
    completion_sent: bool,
    completion_applied: bool,
    completion_ack_sent: bool,
) -> Result<Vec<u8>, PairingRuntimeError> {
    let PairingTransportSnapshot {
        role,
        context,
        mut private_key,
        slot,
        token,
        slot_capability,
        remote_public_key,
    } = transport;
    let local = encode_optional_offer(local_offer)?;
    let remote = encode_optional_offer(remote_offer)?;
    let mut output = Vec::with_capacity(128 + local.len() + remote.len());
    output.push(PAIRING_STATE_VERSION);
    output.push(match role {
        PairingRole::Creator => 1,
        PairingRole::Joiner => 2,
    });
    output.extend_from_slice(&private_key);
    private_key.fill(0);
    output.extend_from_slice(&context.0.into_bytes());
    output.extend_from_slice(&slot.0.into_bytes());
    output.extend_from_slice(&token.0.into_bytes());
    match slot_capability {
        Some(capability) => {
            output.push(1);
            output.extend_from_slice(&capability.0.into_bytes());
        }
        None => output.push(0),
    }
    match remote_public_key {
        Some(key) => {
            output.push(1);
            output.extend_from_slice(&key);
        }
        None => output.push(0),
    }
    output.push(u8::from(completion_sent));
    output.push(u8::from(completion_applied));
    output.push(u8::from(completion_ack_sent));
    output.extend_from_slice(&local);
    output.extend_from_slice(&remote);
    Ok(output)
}

fn encode_optional_offer(offer: Option<&PairingEnvelope>) -> Result<Vec<u8>, PairingRuntimeError> {
    let bytes = match offer {
        Some(offer) => offer.encode().map_err(|_| PairingRuntimeError::InvalidOffer)?,
        None => Vec::new(),
    };
    let length = u16::try_from(bytes.len()).map_err(|_| PairingRuntimeError::InvalidOffer)?;
    let mut output = Vec::with_capacity(2 + bytes.len());
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(&bytes);
    Ok(output)
}

fn decode_persisted_state(
    _session_id: PairingSessionId,
    bytes: &[u8],
) -> Result<PersistedPairingState, PairingRuntimeError> {
    let mut input = bytes;
    if take_u8(&mut input)? != PAIRING_STATE_VERSION {
        return Err(PairingRuntimeError::InvalidOffer);
    }
    let role = match take_u8(&mut input)? {
        1 => PairingRole::Creator,
        2 => PairingRole::Joiner,
        _ => return Err(PairingRuntimeError::InvalidOffer),
    };
    let private_key = take_array::<32>(&mut input)?;
    let context = crate::PairingContextId(OpaqueId::from_bytes(take_array::<16>(&mut input)?));
    let slot = PairingSlotId(OpaqueId::from_bytes(take_array::<16>(&mut input)?));
    let token = PairingSideToken(OpaqueId::from_bytes(take_array::<16>(&mut input)?));
    let slot_capability = match take_u8(&mut input)? {
        0 => None,
        1 => Some(PairingSlotCapability(OpaqueId::from_bytes(take_array::<16>(&mut input)?))),
        _ => return Err(PairingRuntimeError::InvalidOffer),
    };
    let remote_public_key = match take_u8(&mut input)? {
        0 => None,
        1 => Some(take_array::<32>(&mut input)?),
        _ => return Err(PairingRuntimeError::InvalidOffer),
    };
    let completion_sent = take_bool(&mut input)?;
    let completion_applied = take_bool(&mut input)?;
    let completion_ack_sent = take_bool(&mut input)?;
    let local_offer = decode_optional_offer(context, &mut input)?;
    let remote_offer = decode_optional_offer(context, &mut input)?;
    if !input.is_empty() {
        return Err(PairingRuntimeError::InvalidOffer);
    }
    Ok(PersistedPairingState {
        transport: PairingTransportSnapshot {
            role,
            context,
            private_key,
            slot,
            token,
            slot_capability,
            remote_public_key,
        },
        local_offer,
        remote_offer,
        completion_sent,
        completion_applied,
        completion_ack_sent,
    })
}

fn decode_optional_offer(
    context: crate::PairingContextId,
    input: &mut &[u8],
) -> Result<Option<PairingEnvelope>, PairingRuntimeError> {
    let length = usize::from(u16::from_be_bytes(take_array::<2>(input)?));
    if length == 0 {
        return Ok(None);
    }
    let envelope = PairingEnvelope::decode(take(input, length)?)
        .map_err(|_| PairingRuntimeError::InvalidOffer)?;
    envelope.validate_pairing_id(context.0).map_err(|_| PairingRuntimeError::InvalidOffer)?;
    if !matches!(&envelope.payload, PairingPayload::Offer(_)) {
        return Err(PairingRuntimeError::InvalidOffer);
    }
    Ok(Some(envelope))
}

fn take_bool(input: &mut &[u8]) -> Result<bool, PairingRuntimeError> {
    match take_u8(input)? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(PairingRuntimeError::InvalidOffer),
    }
}

fn take_u8(input: &mut &[u8]) -> Result<u8, PairingRuntimeError> {
    Ok(take(input, 1)?[0])
}

fn take_array<const N: usize>(input: &mut &[u8]) -> Result<[u8; N], PairingRuntimeError> {
    take(input, N)?.try_into().map_err(|_| PairingRuntimeError::InvalidOffer)
}

fn take<'a>(input: &mut &'a [u8], length: usize) -> Result<&'a [u8], PairingRuntimeError> {
    if input.len() < length {
        return Err(PairingRuntimeError::InvalidOffer);
    }
    let (head, tail) = input.split_at(length);
    *input = tail;
    Ok(head)
}
