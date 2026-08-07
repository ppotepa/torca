//! Versioned plaintext payloads that are encrypted before crossing the rendezvous relay.
//!
//! The relay protocol treats these bytes as opaque. This crate defines only the canonical
//! client-to-client representation and transcript-binding material; encryption, signatures,
//! pairing state transitions and contact persistence remain outside this protocol crate.

use core::fmt;

use torca_foundation::OpaqueId;

pub const MAX_PAIRING_PAYLOAD_LEN: usize = 48 * 1024;
pub const MAX_PUBLIC_KEY_LEN: usize = 512;
pub const MAX_ONION_ADDRESS_LEN: usize = 255;
pub const MAX_APPROVAL_PROOF_LEN: usize = 512;

const MAGIC: &[u8; 4] = b"TRCP";
const VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum PairingPayloadKind {
    Offer = 1,
    Approval = 2,
    Completion = 3,
    Rejection = 4,
    Cancellation = 5,
}

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairingOffer {
    pub identity_id: OpaqueId,
    pub key_id: OpaqueId,
    pub key_algorithm: u16,
    pub public_key: Vec<u8>,
    pub key_generation: u32,
    pub onion_address: String,
    pub capability_id: OpaqueId,
    pub transcript_nonce: [u8; 32],
}

impl PairingOffer {
    pub fn validate(&self) -> Result<(), PairingProtocolError> {
        if self.public_key.is_empty() || self.public_key.len() > MAX_PUBLIC_KEY_LEN {
            return Err(PairingProtocolError::InvalidPublicKeyLength);
        }
        if self.onion_address.is_empty()
            || self.onion_address.len() > MAX_ONION_ADDRESS_LEN
            || !self.onion_address.is_ascii()
            || self.onion_address.bytes().any(|byte| byte.is_ascii_whitespace() || byte == 0)
            || !self.onion_address.ends_with(".onion")
        {
            return Err(PairingProtocolError::InvalidOnionAddress);
        }
        Ok(())
    }
}

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairingApproval {
    pub transcript_digest: [u8; 32],
    pub proof: Vec<u8>,
}

impl PairingApproval {
    fn validate(&self) -> Result<(), PairingProtocolError> {
        if self.proof.is_empty() || self.proof.len() > MAX_APPROVAL_PROOF_LEN {
            return Err(PairingProtocolError::InvalidApprovalProofLength);
        }
        Ok(())
    }
}

#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PairingCompletion {
    pub transcript_digest: [u8; 32],
}

#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PairingRejection;

/// Explicit cancellation by one side, distinct from rejecting the peer after inspection.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PairingCancellation;

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PairingPayload {
    Offer(PairingOffer),
    Approval(PairingApproval),
    Completion(PairingCompletion),
    Rejection(PairingRejection),
    Cancellation(PairingCancellation),
}

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairingEnvelope {
    pub pairing_id: OpaqueId,
    pub payload: PairingPayload,
}

impl PairingEnvelope {
    pub const fn kind(&self) -> PairingPayloadKind {
        match self.payload {
            PairingPayload::Offer(_) => PairingPayloadKind::Offer,
            PairingPayload::Approval(_) => PairingPayloadKind::Approval,
            PairingPayload::Completion(_) => PairingPayloadKind::Completion,
            PairingPayload::Rejection(_) => PairingPayloadKind::Rejection,
            PairingPayload::Cancellation(_) => PairingPayloadKind::Cancellation,
        }
    }

    pub fn validate_pairing_id(&self, expected: OpaqueId) -> Result<(), PairingProtocolError> {
        if self.pairing_id == expected {
            Ok(())
        } else {
            Err(PairingProtocolError::PairingIdMismatch)
        }
    }

    pub fn encode(&self) -> Result<Vec<u8>, PairingProtocolError> {
        match &self.payload {
            PairingPayload::Offer(offer) => offer.validate()?,
            PairingPayload::Approval(approval) => approval.validate()?,
            PairingPayload::Completion(_)
            | PairingPayload::Rejection(_)
            | PairingPayload::Cancellation(_) => {}
        }

        let mut output = Vec::with_capacity(256);
        output.extend_from_slice(MAGIC);
        output.extend_from_slice(&VERSION.to_be_bytes());
        output.push(self.kind() as u8);
        output.extend_from_slice(&self.pairing_id.into_bytes());

        match &self.payload {
            PairingPayload::Offer(offer) => encode_offer(offer, &mut output)?,
            PairingPayload::Approval(approval) => encode_approval(approval, &mut output)?,
            PairingPayload::Completion(completion) => {
                output.extend_from_slice(&completion.transcript_digest);
            }
            PairingPayload::Rejection(_) | PairingPayload::Cancellation(_) => {}
        }

        if output.len() > MAX_PAIRING_PAYLOAD_LEN {
            return Err(PairingProtocolError::PayloadTooLarge { actual: output.len() });
        }
        Ok(output)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, PairingProtocolError> {
        if bytes.len() > MAX_PAIRING_PAYLOAD_LEN {
            return Err(PairingProtocolError::PayloadTooLarge { actual: bytes.len() });
        }
        let mut cursor = Cursor::new(bytes);
        if cursor.take(4)? != MAGIC {
            return Err(PairingProtocolError::InvalidMagic);
        }
        let version = cursor.u16()?;
        if version != VERSION {
            return Err(PairingProtocolError::UnsupportedVersion(version));
        }
        let kind = cursor.u8()?;
        let pairing_id = OpaqueId::from_bytes(cursor.array_16()?);
        let payload = match kind {
            1 => PairingPayload::Offer(decode_offer(&mut cursor)?),
            2 => PairingPayload::Approval(decode_approval(&mut cursor)?),
            3 => PairingPayload::Completion(PairingCompletion {
                transcript_digest: cursor.array_32()?,
            }),
            4 => PairingPayload::Rejection(PairingRejection),
            5 => PairingPayload::Cancellation(PairingCancellation),
            _ => return Err(PairingProtocolError::UnknownPayloadKind(kind)),
        };
        if !cursor.is_empty() {
            return Err(PairingProtocolError::TrailingBytes);
        }
        let envelope = Self { pairing_id, payload };
        match &envelope.payload {
            PairingPayload::Offer(offer) => offer.validate()?,
            PairingPayload::Approval(approval) => approval.validate()?,
            PairingPayload::Completion(_)
            | PairingPayload::Rejection(_)
            | PairingPayload::Cancellation(_) => {}
        }
        Ok(envelope)
    }

    pub fn transcript_component(&self) -> Result<Vec<u8>, PairingProtocolError> {
        match self.payload {
            PairingPayload::Offer(_) => self.encode(),
            _ => Err(PairingProtocolError::NotAnOffer),
        }
    }
}

fn encode_offer(offer: &PairingOffer, output: &mut Vec<u8>) -> Result<(), PairingProtocolError> {
    output.extend_from_slice(&offer.identity_id.into_bytes());
    output.extend_from_slice(&offer.key_id.into_bytes());
    output.extend_from_slice(&offer.key_algorithm.to_be_bytes());
    put_bytes(&offer.public_key, output)?;
    output.extend_from_slice(&offer.key_generation.to_be_bytes());
    put_bytes(offer.onion_address.as_bytes(), output)?;
    output.extend_from_slice(&offer.capability_id.into_bytes());
    output.extend_from_slice(&offer.transcript_nonce);
    Ok(())
}

fn decode_offer(cursor: &mut Cursor<'_>) -> Result<PairingOffer, PairingProtocolError> {
    let identity_id = OpaqueId::from_bytes(cursor.array_16()?);
    let key_id = OpaqueId::from_bytes(cursor.array_16()?);
    let key_algorithm = cursor.u16()?;
    let public_key = cursor.bytes(MAX_PUBLIC_KEY_LEN)?;
    let key_generation = cursor.u32()?;
    let onion_bytes = cursor.bytes(MAX_ONION_ADDRESS_LEN)?;
    let onion_address = String::from_utf8(onion_bytes)
        .map_err(|_| PairingProtocolError::InvalidOnionAddress)?;
    let capability_id = OpaqueId::from_bytes(cursor.array_16()?);
    let transcript_nonce = cursor.array_32()?;
    Ok(PairingOffer {
        identity_id,
        key_id,
        key_algorithm,
        public_key,
        key_generation,
        onion_address,
        capability_id,
        transcript_nonce,
    })
}

fn encode_approval(
    approval: &PairingApproval,
    output: &mut Vec<u8>,
) -> Result<(), PairingProtocolError> {
    output.extend_from_slice(&approval.transcript_digest);
    put_bytes(&approval.proof, output)
}

fn decode_approval(cursor: &mut Cursor<'_>) -> Result<PairingApproval, PairingProtocolError> {
    Ok(PairingApproval {
        transcript_digest: cursor.array_32()?,
        proof: cursor.bytes(MAX_APPROVAL_PROOF_LEN)?,
    })
}

fn put_bytes(value: &[u8], output: &mut Vec<u8>) -> Result<(), PairingProtocolError> {
    let length = u16::try_from(value.len()).map_err(|_| PairingProtocolError::FieldTooLarge)?;
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(value);
    Ok(())
}

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Cursor<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    fn take(&mut self, length: usize) -> Result<&'a [u8], PairingProtocolError> {
        let end = self.offset.checked_add(length).ok_or(PairingProtocolError::Truncated)?;
        if end > self.bytes.len() {
            return Err(PairingProtocolError::Truncated);
        }
        let value = &self.bytes[self.offset..end];
        self.offset = end;
        Ok(value)
    }
    fn u8(&mut self) -> Result<u8, PairingProtocolError> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, PairingProtocolError> {
        Ok(u16::from_be_bytes(self.take(2)?.try_into().map_err(|_| PairingProtocolError::Truncated)?))
    }
    fn u32(&mut self) -> Result<u32, PairingProtocolError> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().map_err(|_| PairingProtocolError::Truncated)?))
    }
    fn array_16(&mut self) -> Result<[u8; 16], PairingProtocolError> {
        self.take(16)?.try_into().map_err(|_| PairingProtocolError::Truncated)
    }
    fn array_32(&mut self) -> Result<[u8; 32], PairingProtocolError> {
        self.take(32)?.try_into().map_err(|_| PairingProtocolError::Truncated)
    }
    fn bytes(&mut self, maximum: usize) -> Result<Vec<u8>, PairingProtocolError> {
        let length = usize::from(self.u16()?);
        if length > maximum {
            return Err(PairingProtocolError::FieldTooLarge);
        }
        Ok(self.take(length)?.to_vec())
    }
    const fn is_empty(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PairingProtocolError {
    InvalidMagic,
    UnsupportedVersion(u16),
    UnknownPayloadKind(u8),
    PairingIdMismatch,
    InvalidPublicKeyLength,
    InvalidOnionAddress,
    InvalidApprovalProofLength,
    PayloadTooLarge { actual: usize },
    FieldTooLarge,
    Truncated,
    TrailingBytes,
    NotAnOffer,
}
impl fmt::Display for PairingProtocolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for PairingProtocolError {}

#[cfg(test)]
mod tests {
    use torca_foundation::OpaqueId;

    use super::{PairingEnvelope, PairingOffer, PairingPayload};

    fn offer(pairing: u128) -> PairingEnvelope {
        PairingEnvelope {
            pairing_id: OpaqueId::from_u128(pairing),
            payload: PairingPayload::Offer(PairingOffer {
                identity_id: OpaqueId::from_u128(2),
                key_id: OpaqueId::from_u128(3),
                key_algorithm: 1,
                public_key: vec![7; 32],
                key_generation: 0,
                onion_address: format!("{}.onion", "a".repeat(56)),
                capability_id: OpaqueId::from_u128(4),
                transcript_nonce: [9; 32],
            }),
        }
    }

    #[test]
    fn offer_round_trips_and_is_bound_to_pairing_id() {
        let envelope = offer(1);
        let encoded = envelope.encode().expect("encode");
        let decoded = PairingEnvelope::decode(&encoded).expect("decode");
        assert_eq!(decoded, envelope);
        assert!(decoded.validate_pairing_id(OpaqueId::from_u128(1)).is_ok());
        assert!(decoded.validate_pairing_id(OpaqueId::from_u128(99)).is_err());
    }
}
