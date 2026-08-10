use std::collections::{BTreeMap, BTreeSet};

use torca_client_engine::{EngineCommand, EngineHandle, EngineResult};
use torca_contacts::{ContactId, ContactRoute, PeerCredential};
use torca_conversations::ConversationId;
use torca_foundation::{OpaqueId, Timestamp};
use torca_identity::{IdentityKey, KeyAlgorithm, PublicIdentity};
use torca_pairing::{
    PairingCode, PairingRole, PairingSession, PairingSessionId, PairingState, PeerProposal,
};
use torca_pairing_protocol::{
    PairingApproval, PairingCancellation, PairingCompletion, PairingEnvelope, PairingOffer,
    PairingPayload, PairingRejection,
};

use crate::{
    PairingApprovalError, PairingApprovalPort, PairingCoordinator, PairingCoordinatorError,
    PairingCredentialError, PairingCryptoPort, PairingPeerSecretStore, PairingRendezvousPort,
    encode_invite_uri, invitation_expires_at,
};

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalPairingContext {
    pub public_identity: PublicIdentity,
    pub display_name: String,
    pub onion_address: String,
    pub capability_id: OpaqueId,
}

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairingInvitation {
    pub session_id: PairingSessionId,
    pub code: PairingCode,
    pub uri: String,
    pub expires_at: Timestamp,
}

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairingCompletedContact {
    pub contact_id: ContactId,
    pub display_name: String,
}

#[must_use]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PairingPollReport {
    pub offers_applied: usize,
    pub approvals_applied: usize,
    pub completions_applied: usize,
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
        }
    }

    pub fn create_invitation(
        &mut self,
        session_id: PairingSessionId,
        local: LocalPairingContext,
        now: Timestamp,
    ) -> Result<PairingInvitation, PairingRuntimeError> {
        let code = self.coordinator.generate_pairing_code()?;
        let requested_expires_at = invitation_expires_at(now)?;
        let local_offer = self.local_offer(session_id, local)?;
        let (_, expires_at) =
            self.coordinator.open_creator(session_id, &code, requested_expires_at)?;
        if self
            .engine
            .dispatch(EngineCommand::StartPairing { session_id, code: code.clone(), expires_at })
            .is_err()
        {
            let _ = self.coordinator.close(session_id);
            return Err(PairingRuntimeError::Engine);
        }
        self.local_offers.insert(session_id, local_offer);
        Ok(PairingInvitation { session_id, uri: encode_invite_uri(&code), code, expires_at })
    }

    pub fn join_invitation(
        &mut self,
        session_id: PairingSessionId,
        code: PairingCode,
        local: LocalPairingContext,
        now: Timestamp,
    ) -> Result<(), PairingRuntimeError> {
        let local_offer = self.local_offer(session_id, local)?;
        let (_, expires_at) = self.coordinator.join(session_id, &code, &local_offer)?;
        if self
            .engine
            .dispatch(EngineCommand::JoinPairing { session_id, code, expires_at })
            .is_err()
        {
            let _ = self.coordinator.close(session_id);
            return Err(PairingRuntimeError::Engine);
        }
        self.local_offers.insert(session_id, local_offer);
        Ok(())
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
        let proof =
            self.approval.sign_approval(identity.public().key().key_id(), session_id, digest)?;
        self.coordinator.push(
            session_id,
            &PairingEnvelope {
                pairing_id: session_id.to_opaque(),
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
                pairing_id: session_id.to_opaque(),
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
                pairing_id: session_id.to_opaque(),
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
        let envelopes = self.coordinator.poll(session_id)?;
        let mut report = PairingPollReport::default();
        for envelope in envelopes {
            match &envelope.payload {
                PairingPayload::Offer(offer) => {
                    if let Some(existing) = self.remote_offers.get(&session_id) {
                        if existing == &envelope {
                            continue;
                        }
                        return Err(PairingRuntimeError::InvalidOffer);
                    }
                    let proposal = peer_proposal(offer)?;
                    let _ = self
                        .engine
                        .dispatch(EngineCommand::PeerJoined { session_id, proposal, at: now })
                        .map_err(|_| PairingRuntimeError::Engine)?;
                    self.remote_offers.insert(session_id, envelope);
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
                    self.approval.verify_approval(&identity, session_id, digest, &remote.proof)?;
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
                    report.completions_applied += 1;
                    report.completed_contact = Some(completed);
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
                    self.cleanup_terminal(session_id);
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
                    self.cleanup_terminal(session_id);
                    break;
                }
            }
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
        self.coordinator.close(session_id).map_err(Into::into)
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
                pairing_id: session_id.to_opaque(),
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
        if !session.can_complete(now) {
            return Err(PairingRuntimeError::InvalidCompletion);
        }
        let digest = self.transcript_digest(session_id, session.role())?;
        if completion.transcript_digest != digest {
            return Err(PairingRuntimeError::InvalidCompletion);
        }
        let display_name = self.remote_display_name(session_id)?;
        let secret = self.coordinator.derive_peer_secret(session_id, digest)?;
        let secret_handle = self.peer_secrets.store_peer_secret(secret)?;
        let contact_id = ContactId::from_opaque(session_id.to_opaque());
        let conversation_id = ConversationId::from_opaque(session_id.to_opaque());
        let local_capability_id = self.local_capability_id(session_id)?;
        let credential = PeerCredential::new(contact_id, local_capability_id, secret_handle)
            .map_err(|_| PairingRuntimeError::InvalidCompletion)?;
        let durable = self.engine.dispatch(EngineCommand::CompletePairing {
            session_id,
            contact_id,
            conversation_id,
            credential,
            at: now,
        });
        match durable {
            Ok(EngineResult::PairingCompleted { .. }) => {
                self.cleanup_terminal(session_id);
                Ok(PairingCompletedContact { contact_id, display_name })
            }
            Ok(_) | Err(_) => {
                let _ = self.peer_secrets.delete_peer_secret(secret_handle);
                Err(PairingRuntimeError::Engine)
            }
        }
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
        session_id: PairingSessionId,
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
        };
        offer.validate().map_err(|_| PairingRuntimeError::InvalidOffer)?;
        Ok(PairingEnvelope {
            pairing_id: session_id.to_opaque(),
            payload: PairingPayload::Offer(offer),
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
    Ok(PeerProposal { public_identity, route })
}
