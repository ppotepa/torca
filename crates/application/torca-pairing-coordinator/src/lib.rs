//! Application orchestration for ephemeral pairing transport state.
//!
//! Domain approval state remains in `torca-pairing`. This coordinator owns only rendezvous slot
//! capabilities and crypto handles needed while one pairing session is active.

use core::fmt;
use std::collections::BTreeMap;

use torca_foundation::{OpaqueId, Timestamp};
use torca_pairing::{PairingCode, PairingSessionId};
use torca_pairing_protocol::PairingEnvelope;

#[must_use]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PairingCryptoHandle(pub OpaqueId);

#[must_use]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PairingSlotId(pub OpaqueId);

#[must_use]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PairingSlotCapability(pub OpaqueId);

#[must_use]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PairingSideToken(pub OpaqueId);

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairingEphemeralKey {
    pub handle: PairingCryptoHandle,
    pub public_key: [u8; 32],
}

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncryptedPairingPayload {
    pub sender_public_key: [u8; 32],
    pub nonce: [u8; 24],
    pub ciphertext: Vec<u8>,
}

pub trait PairingCryptoPort {
    fn generate_ephemeral_key(&mut self) -> Result<PairingEphemeralKey, PairingCoordinatorError>;
    fn release_ephemeral_key(
        &mut self,
        handle: PairingCryptoHandle,
    ) -> Result<(), PairingCoordinatorError>;
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
        let setup = (|| {
            let capability = PairingSlotCapability(self.random_id()?);
            let token = PairingSideToken(self.random_id()?);
            let slot = self
                .rendezvous
                .open(code, expires_at, key.public_key.to_vec(), capability, token)?;
            Ok::<_, PairingCoordinatorError>((slot, capability, token))
        })();
        let (slot, capability, token) = match setup {
            Ok(value) => value,
            Err(error) => {
                let _ = self.crypto.release_ephemeral_key(key.handle);
                return Err(error);
            }
        };
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
        let setup = (|| {
            let token = PairingSideToken(self.random_id()?);
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
            Ok::<_, PairingCoordinatorError>((slot, token, creator_public_key))
        })();
        let (slot, token, creator_public_key) = match setup {
            Ok(value) => value,
            Err(error) => {
                let _ = self.crypto.release_ephemeral_key(key.handle);
                return Err(error);
            }
        };
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

    pub fn close(&mut self, session_id: PairingSessionId) -> Result<(), PairingCoordinatorError> {
        let session = self
            .sessions
            .remove(&session_id)
            .ok_or(PairingCoordinatorError::SessionNotFound)?;
        let relay_result = if session.role == LocalRole::Creator {
            match session.slot_capability {
                Some(capability) => self.rendezvous.close(session.slot, capability),
                None => Err(PairingCoordinatorError::InvalidRole),
            }
        } else {
            Ok(())
        };
        let release_result = self.crypto.release_ephemeral_key(session.key.handle);
        relay_result?;
        release_result
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
        for _ in 0..8 {
            let mut bytes = [0_u8; 16];
            self.crypto.fill_random(&mut bytes)?;
            let id = OpaqueId::from_bytes(bytes);
            if !id.is_nil() {
                return Ok(id);
            }
        }
        Err(PairingCoordinatorError::Crypto)
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
