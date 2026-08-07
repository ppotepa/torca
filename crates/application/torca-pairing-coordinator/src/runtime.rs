use std::collections::BTreeMap;

use torca_client_engine::{EngineCommand, EngineHandle};
use torca_contacts::ContactRoute;
use torca_foundation::{OpaqueId, Timestamp};
use torca_identity::{IdentityKey, KeyAlgorithm, PublicIdentity};
use torca_pairing::{PairingCode, PairingRole, PairingSession, PairingSessionId, PeerProposal};
use torca_pairing_protocol::{PairingEnvelope, PairingOffer, PairingPayload};

use crate::{
    PairingCoordinator, PairingCoordinatorError, PairingCryptoPort, PairingRendezvousPort,
    encode_invite_uri, invitation_expires_at,
};

/// Public local material used to construct an encrypted pairing offer. Private keys and
/// capability bytes are deliberately absent; only the opaque capability identifier is shared.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalPairingContext {
    pub public_identity: PublicIdentity,
    pub onion_address: String,
    pub capability_id: OpaqueId,
}

/// Presentation-safe result of creating an invitation.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairingInvitation {
    pub session_id: PairingSessionId,
    pub code: PairingCode,
    pub uri: String,
    pub expires_at: Timestamp,
}

/// Summary of one bounded rendezvous poll.
#[must_use]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PairingPollReport {
    pub offers_applied: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PairingRuntimeError {
    Coordinator(PairingCoordinatorError),
    Engine,
    InvalidOffer,
    UnsupportedAlgorithm,
    UnexpectedPayload,
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

/// Single application owner that connects pairing transport events to the ClientEngine state
/// machine. It stores only the local/remote public offer envelopes needed for transcript binding.
pub struct PairingRuntime<R, C> {
    coordinator: PairingCoordinator<R, C>,
    engine: EngineHandle,
    local_offers: BTreeMap<PairingSessionId, PairingEnvelope>,
    remote_offers: BTreeMap<PairingSessionId, PairingEnvelope>,
}

impl<R, C> PairingRuntime<R, C>
where
    R: PairingRendezvousPort,
    C: PairingCryptoPort,
{
    pub const fn new(coordinator: PairingCoordinator<R, C>, engine: EngineHandle) -> Self {
        Self {
            coordinator,
            engine,
            local_offers: BTreeMap::new(),
            remote_offers: BTreeMap::new(),
        }
    }

    pub fn create_invitation(
        &mut self,
        session_id: PairingSessionId,
        local: LocalPairingContext,
        now: Timestamp,
    ) -> Result<PairingInvitation, PairingRuntimeError> {
        let code = self.coordinator.generate_pairing_code()?;
        let expires_at = invitation_expires_at(now)?;
        let local_offer = self.local_offer(session_id, local)?;
        self.coordinator.open_creator(session_id, &code, expires_at)?;
        if self
            .engine
            .dispatch(EngineCommand::StartPairing {
                session_id,
                code: code.clone(),
                expires_at,
            })
            .is_err()
        {
            let _ = self.coordinator.close(session_id);
            return Err(PairingRuntimeError::Engine);
        }
        self.local_offers.insert(session_id, local_offer);
        Ok(PairingInvitation {
            session_id,
            uri: encode_invite_uri(&code),
            code,
            expires_at,
        })
    }

    pub fn join_invitation(
        &mut self,
        session_id: PairingSessionId,
        code: PairingCode,
        local: LocalPairingContext,
        now: Timestamp,
    ) -> Result<(), PairingRuntimeError> {
        let expires_at = invitation_expires_at(now)?;
        let local_offer = self.local_offer(session_id, local)?;
        self.coordinator.join(session_id, &code, &local_offer)?;
        if self
            .engine
            .dispatch(EngineCommand::JoinPairing {
                session_id,
                code,
                expires_at,
            })
            .is_err()
        {
            let _ = self.coordinator.close(session_id);
            return Err(PairingRuntimeError::Engine);
        }
        self.local_offers.insert(session_id, local_offer);
        Ok(())
    }

    /// Applies decrypted offers to the engine. Approval/completion payloads are intentionally
    /// rejected until their cryptographic workflow is installed by the next runtime stage.
    pub fn poll(
        &mut self,
        session_id: PairingSessionId,
        now: Timestamp,
    ) -> Result<PairingPollReport, PairingRuntimeError> {
        let session = self.session(session_id)?;
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
                    self.engine
                        .dispatch(EngineCommand::PeerJoined { session_id, proposal, at: now })
                        .map_err(|_| PairingRuntimeError::Engine)?;
                    self.remote_offers.insert(session_id, envelope);
                    report.offers_applied += 1;
                    if session.role() == PairingRole::Creator {
                        let local_offer = self
                            .local_offers
                            .get(&session_id)
                            .cloned()
                            .ok_or(PairingRuntimeError::InvalidOffer)?;
                        self.coordinator.push(session_id, &local_offer)?;
                    }
                }
                PairingPayload::Approval(_) | PairingPayload::Completion(_) => {
                    return Err(PairingRuntimeError::UnexpectedPayload);
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
        self.coordinator.close(session_id).map_err(Into::into)
    }

    pub fn into_parts(self) -> (PairingCoordinator<R, C>, EngineHandle) {
        (self.coordinator, self.engine)
    }

    fn session(&self, session_id: PairingSessionId) -> Result<PairingSession, PairingRuntimeError> {
        self.engine
            .snapshot()
            .map_err(|_| PairingRuntimeError::Engine)?
            .pairings
            .into_iter()
            .find(|session| session.id() == session_id)
            .ok_or(PairingRuntimeError::SessionNotFound)
    }

    fn local_offer(
        &mut self,
        session_id: PairingSessionId,
        local: LocalPairingContext,
    ) -> Result<PairingEnvelope, PairingRuntimeError> {
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
