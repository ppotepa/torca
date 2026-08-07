//! Application orchestration for ephemeral pairing transport state.
//!
//! Domain approval state remains in `torca-pairing`. This coordinator owns only rendezvous slot
//! capabilities and crypto handles needed while one pairing session is active.

use core::fmt;
use std::collections::BTreeMap;

use torca_foundation::{OpaqueId, Timestamp};
use torca_pairing::{PairingCode, PairingSessionId};
use torca_pairing_protocol::PairingEnvelope;

/// Opaque crypto-provider handle for one ephemeral rendezvous key pair.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PairingCryptoHandle(pub OpaqueId);

/// Slot address returned by the rendezvous service.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PairingSlotId(pub OpaqueId);

/// Client-generated administrative capability for a rendezvous slot.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PairingSlotCapability(pub OpaqueId);

/// Client-generated capability for one side of a rendezvous slot.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PairingSideToken(pub OpaqueId);

/// Ephemeral key material exposed by a crypto adapter without private bytes.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairingEphemeralKey {
    pub handle: PairingCryptoHandle,
    pub public_key: [u8; 32],
}

/// Encrypted relay payload. The relay sees only these opaque bytes.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncryptedPairingPayload {
    pub sender_public_key: [u8; 32],
    pub nonce: [u8; 24],
    pub ciphertext: Vec<u8>,
}

/// Semantic crypto operations needed by pairing orchestration.
pub trait PairingCryptoPort {
    fn generate_ephemeral_key(&mut self) -> Result<PairingEphemeralKey, PairingCoordinatorError>;
    fn fill_random(&mut self, output: &mut [u8]) -> Result<(), PairingCoordinatorError>;
    fn seal_for_peer(
        &self,
        local_key: PairingCryptoHandle,
        remote_public_key: [u8; 32],
        nonce: [u8; 24],
        associated_data: &[u8],
        plaintext: &[u8],
    ) -> Result<Vec<u8>, PairingCoordinatorError>;
    fn open_from_peer(
        &self,
        local_key: PairingCryptoHandle,
        remote_public_key: [u8; 32],
        nonce: [u8; 24],
        associated_data: &[u8],
        ciphertext: &[u8],
    ) -> Result<Vec<u8>, PairingCoordinatorError>;
}

/// Ephemeral rendezvous transport. Implementations may use WebSocket/Tor but must not persist
/// contacts, messages or user identity data.
pub trait PairingRendezvousPort {
    fn open(
        &mut self,
        code: &PairingCode,
        expires_at: Timestamp,
        creator_blob: Vec<u8>,
        capability: PairingSlotCapability,
        creator_token: PairingSideToken,
    ) -> Result<PairingSlotId, PairingCoordinatorError>;
    fn join(
        &mut self,
        code: &PairingCode,
        joiner_blob: Vec<u8>,
        joiner_token: PairingSideToken,
    ) -> Result<(PairingSlotId, Vec<u8>), PairingCoordinatorError>;
    fn push(
        &mut self,
        slot: PairingSlotId,
        token: PairingSideToken,
        blob: Vec<u8>,
    ) -> Result<(), PairingCoordinatorError>;
    fn poll(
        &mut self,
        slot: PairingSlotId,
        token: PairingSideToken,
    ) -> Result<Vec<Vec<u8>>, PairingCoordinatorError>;
    fn close(
        &mut self,
        slot: PairingSlotId,
        capability: PairingSlotCapability,
    ) -> Result<(), PairingCoordinatorError>;
}

/// Redaction-safe pairing orchestration error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PairingCoordinatorError {
    SessionAlreadyExists,
    SessionNotFound,
    InvalidRole,
    InvalidBlob,
    Protocol,
    Crypto,
    Rendezvous,
}
impl fmt::Display for PairingCoordinatorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for PairingCoordinatorError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LocalRole {
    Creator,
    Joiner,
}

#[derive(Clone, Debug)]
struct TransportSession {
    role: LocalRole,
    key: PairingEphemeralKey,
    slot: PairingSlotId,
    token: PairingSideToken,
    slot_capability: Option<PairingSlotCapability>,
    remote_public_key: Option<[u8; 32]>,
}

/// Coordinator for ephemeral rendezvous/crypto state keyed by the domain pairing session ID.
pub struct PairingCoordinator<R, C> {
    rendezvous: R,
    crypto: C,
    sessions: BTreeMap<PairingSessionId, TransportSession>,
}

impl<R, C> PairingCoordinator<R, C>
where
    R: PairingRendezvousPort,
    C: PairingCryptoPort,
{
    pub const fn new(rendezvous: R, crypto: C) -> Self {
        Self { rendezvous, crypto, sessions: BTreeMap::new() }
    }

    /// Opens a creator slot. The relay receives only the ephemeral public rendezvous key.
    pub fn open_creator(
        &mut self,
        session_id: PairingSessionId,
        code: &PairingCode,
        expires_at: Timestamp,
    ) -> Result<PairingSlotId, PairingCoordinatorError> {
        if self.sessions.contains_key(&session_id) {
            return Err(PairingCoordinatorError::SessionAlreadyExists);
        }
        let key = self.crypto.generate_ephemeral_key()?;
        let capability = PairingSlotCapability(self.random_id()?);
        let token = PairingSideToken(self.random_id()?);
        let slot = self
            .rendezvous
            .open(code, expires_at, key.public_key.to_vec(), capability, token)?;
        self.sessions.insert(
            session_id,
            TransportSession {
                role: LocalRole::Creator,
                key,
                slot,
                token,
                slot_capability: Some(capability),
                remote_public_key: None,
            },
        );
        Ok(slot)
    }

    /// Joins a creator slot and returns the decrypted creator-independent handshake key.
    ///
    /// `local_offer` is encrypted before it crosses the relay. The creator public key is the
    /// creator blob returned by rendezvous and therefore never treated as secret material.
    pub fn join(
        &mut self,
        session_id: PairingSessionId,
        code: &PairingCode,
        local_offer: &PairingEnvelope,
    ) -> Result<PairingSlotId, PairingCoordinatorError> {
        if self.sessions.contains_key(&session_id) {
            return Err(PairingCoordinatorError::SessionAlreadyExists);
        }
        local_offer
            .validate_pairing_id(session_id.to_opaque())
            .map_err(|_| PairingCoordinatorError::Protocol)?;
        let key = self.crypto.generate_ephemeral_key()?;
        let token = PairingSideToken(self.random_id()?);

        // First join installs our token and public key. The returned creator blob is its ephemeral
        // public key. The actual encrypted offer is then pushed with transcript-bound AAD.
        let (slot, creator_blob) = self.rendezvous.join(code, key.public_key.to_vec(), token)?;
        let creator_public_key: [u8; 32] = creator_blob
            .try_into()
            .map_err(|_| PairingCoordinatorError::InvalidBlob)?;
        let encrypted = self.encrypt_envelope(
            session_id,
            &key,
            creator_public_key,
            local_offer,
        )?;
        self.rendezvous.push(slot, token, encode_encrypted(&encrypted))?;
        self.sessions.insert(
            session_id,
            TransportSession {
                role: LocalRole::Joiner,
                key,
                slot,
                token,
                slot_capability: None,
                remote_public_key: Some(creator_public_key),
            },
        );
        Ok(slot)
    }

    /// Polls and decrypts every currently queued pairing envelope for the session.
    pub fn poll(
        &mut self,
        session_id: PairingSessionId,
    ) -> Result<Vec<PairingEnvelope>, PairingCoordinatorError> {
        let session = self
            .sessions
            .get(&session_id)
            .cloned()
            .ok_or(PairingCoordinatorError::SessionNotFound)?;
        let blobs = self.rendezvous.poll(session.slot, session.token)?;
        let mut envelopes = Vec::with_capacity(blobs.len());
        for blob in blobs {
            let encrypted = decode_encrypted(&blob)?;
            let remote = match session.remote_public_key {
                Some(expected) if expected != encrypted.sender_public_key => {
                    return Err(PairingCoordinatorError::InvalidBlob);
                }
                Some(expected) => expected,
                None => encrypted.sender_public_key,
            };
            let plaintext = self.crypto.open_from_peer(
                session.key.handle,
                remote,
                encrypted.nonce,
                &associated_data(session_id),
                &encrypted.ciphertext,
            )?;
            let envelope = PairingEnvelope::decode(&plaintext)
                .map_err(|_| PairingCoordinatorError::Protocol)?;
            envelope
                .validate_pairing_id(session_id.to_opaque())
                .map_err(|_| PairingCoordinatorError::Protocol)?;
            envelopes.push(envelope);
            if session.remote_public_key.is_none() {
                if let Some(stored) = self.sessions.get_mut(&session_id) {
                    stored.remote_public_key = Some(remote);
                }
            }
        }
        Ok(envelopes)
    }

    /// Encrypts and pushes one protocol envelope to the authenticated opposite side.
    pub fn push(
        &mut self,
        session_id: PairingSessionId,
        envelope: &PairingEnvelope,
    ) -> Result<(), PairingCoordinatorError> {
        envelope
            .validate_pairing_id(session_id.to_opaque())
            .map_err(|_| PairingCoordinatorError::Protocol)?;
        let session = self
            .sessions
            .get(&session_id)
            .cloned()
            .ok_or(PairingCoordinatorError::SessionNotFound)?;
        let remote = session
            .remote_public_key
            .ok_or(PairingCoordinatorError::InvalidBlob)?;
        let encrypted = self.encrypt_envelope(session_id, &session.key, remote, envelope)?;
        self.rendezvous
            .push(session.slot, session.token, encode_encrypted(&encrypted))
    }

    /// Closes creator-owned rendezvous state and always drops local ephemeral handles.
    pub fn close(&mut self, session_id: PairingSessionId) -> Result<(), PairingCoordinatorError> {
        let session = self
            .sessions
            .remove(&session_id)
            .ok_or(PairingCoordinatorError::SessionNotFound)?;
        if session.role == LocalRole::Creator {
            let capability = session
                .slot_capability
                .ok_or(PairingCoordinatorError::InvalidRole)?;
            self.rendezvous.close(session.slot, capability)?;
        }
        Ok(())
    }

    pub fn into_parts(self) -> (R, C) {
        (self.rendezvous, self.crypto)
    }

    fn encrypt_envelope(
        &mut self,
        session_id: PairingSessionId,
        local_key: &PairingEphemeralKey,
        remote_public_key: [u8; 32],
        envelope: &PairingEnvelope,
    ) -> Result<EncryptedPairingPayload, PairingCoordinatorError> {
        let plaintext = envelope.encode().map_err(|_| PairingCoordinatorError::Protocol)?;
        let mut nonce = [0_u8; 24];
        self.crypto.fill_random(&mut nonce)?;
        let ciphertext = self.crypto.seal_for_peer(
            local_key.handle,
            remote_public_key,
            nonce,
            &associated_data(session_id),
            &plaintext,
        )?;
        Ok(EncryptedPairingPayload {
            sender_public_key: local_key.public_key,
            nonce,
            ciphertext,
        })
    }

    fn random_id(&mut self) -> Result<OpaqueId, PairingCoordinatorError> {
        let mut bytes = [0_u8; 16];
        self.crypto.fill_random(&mut bytes)?;
        Ok(OpaqueId::from_bytes(bytes))
    }
}

fn associated_data(session_id: PairingSessionId) -> [u8; 16] {
    session_id.to_opaque().into_bytes()
}

fn encode_encrypted(payload: &EncryptedPairingPayload) -> Vec<u8> {
    let mut output = Vec::with_capacity(32 + 24 + 4 + payload.ciphertext.len());
    output.extend_from_slice(&payload.sender_public_key);
    output.extend_from_slice(&payload.nonce);
    let length = u32::try_from(payload.ciphertext.len()).unwrap_or(u32::MAX);
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(&payload.ciphertext);
    output
}

fn decode_encrypted(bytes: &[u8]) -> Result<EncryptedPairingPayload, PairingCoordinatorError> {
    if bytes.len() < 60 {
        return Err(PairingCoordinatorError::InvalidBlob);
    }
    let sender_public_key = bytes[0..32]
        .try_into()
        .map_err(|_| PairingCoordinatorError::InvalidBlob)?;
    let nonce = bytes[32..56]
        .try_into()
        .map_err(|_| PairingCoordinatorError::InvalidBlob)?;
    let length = u32::from_be_bytes(
        bytes[56..60]
            .try_into()
            .map_err(|_| PairingCoordinatorError::InvalidBlob)?,
    );
    let length = usize::try_from(length).map_err(|_| PairingCoordinatorError::InvalidBlob)?;
    if bytes.len() != 60_usize.saturating_add(length) {
        return Err(PairingCoordinatorError::InvalidBlob);
    }
    Ok(EncryptedPairingPayload {
        sender_public_key,
        nonce,
        ciphertext: bytes[60..].to_vec(),
    })
}
