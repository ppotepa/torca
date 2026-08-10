//! Versioned plaintext payloads encrypted before crossing the rendezvous relay.

mod invite_uri;

pub use invite_uri::{
    InviteUriError, PairingInviteCode, PairingInviteTicket, decode_invite_uri, encode_invite_uri,
};

use core::fmt;
use torca_foundation::OpaqueId;

pub const MAX_PAIRING_PAYLOAD_LEN: usize = 48 * 1024;
pub const MAX_PUBLIC_KEY_LEN: usize = 512;
pub const MAX_DISPLAY_NAME_LEN: usize = 256;
pub const MAX_ONION_ADDRESS_LEN: usize = 255;
pub const MAX_APPROVAL_PROOF_LEN: usize = 512;
const MAGIC: &[u8; 4] = b"TRCP";
const VERSION: u16 = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum PairingPayloadKind {
    Offer = 1,
    Approval = 2,
    Completion = 3,
    Rejection = 4,
    Cancellation = 5,
    CompletionAck = 6,
}

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairingOffer {
    pub identity_id: OpaqueId,
    pub key_id: OpaqueId,
    pub key_algorithm: u16,
    pub public_key: Vec<u8>,
    pub key_generation: u32,
    pub display_name: String,
    pub onion_address: String,
    pub capability_id: OpaqueId,
    pub transcript_nonce: [u8; 32],
}
impl PairingOffer {
    pub fn validate(&self) -> Result<(), PairingProtocolError> {
        if self.public_key.is_empty() || self.public_key.len() > MAX_PUBLIC_KEY_LEN {
            return Err(PairingProtocolError::InvalidPublicKeyLength);
        }
        let name = self.display_name.trim();
        let count = name.chars().count();
        if count == 0
            || count > 64
            || self.display_name.len() > MAX_DISPLAY_NAME_LEN
            || name.chars().any(char::is_control)
        {
            return Err(PairingProtocolError::InvalidDisplayName);
        }
        if self.onion_address.is_empty()
            || self.onion_address.len() > MAX_ONION_ADDRESS_LEN
            || !self.onion_address.is_ascii()
            || self.onion_address.bytes().any(|b| b.is_ascii_whitespace() || b == 0)
            || !self.onion_address.to_ascii_lowercase().ends_with(".onion")
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
            Err(PairingProtocolError::InvalidApprovalProofLength)
        } else {
            Ok(())
        }
    }
}
#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PairingCompletion {
    pub transcript_digest: [u8; 32],
}
/// Confirms that the recipient durably created its contact and conversation.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PairingCompletionAck {
    pub transcript_digest: [u8; 32],
}
#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PairingRejection;
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
    CompletionAck(PairingCompletionAck),
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
            PairingPayload::CompletionAck(_) => PairingPayloadKind::CompletionAck,
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
            PairingPayload::Offer(o) => o.validate()?,
            PairingPayload::Approval(a) => a.validate()?,
            PairingPayload::Completion(_)
            | PairingPayload::Rejection(_)
            | PairingPayload::Cancellation(_)
            | PairingPayload::CompletionAck(_) => {}
        }
        let mut out = Vec::with_capacity(320);
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&VERSION.to_be_bytes());
        out.push(self.kind() as u8);
        out.extend_from_slice(&self.pairing_id.into_bytes());
        match &self.payload {
            PairingPayload::Offer(o) => encode_offer(o, &mut out)?,
            PairingPayload::Approval(a) => encode_approval(a, &mut out)?,
            PairingPayload::Completion(c) => out.extend_from_slice(&c.transcript_digest),
            PairingPayload::CompletionAck(ack) => out.extend_from_slice(&ack.transcript_digest),
            PairingPayload::Rejection(_) | PairingPayload::Cancellation(_) => {}
        }
        if out.len() > MAX_PAIRING_PAYLOAD_LEN {
            return Err(PairingProtocolError::PayloadTooLarge { actual: out.len() });
        }
        Ok(out)
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, PairingProtocolError> {
        if bytes.len() > MAX_PAIRING_PAYLOAD_LEN {
            return Err(PairingProtocolError::PayloadTooLarge { actual: bytes.len() });
        }
        let mut c = Cursor::new(bytes);
        if c.take(4)? != MAGIC {
            return Err(PairingProtocolError::InvalidMagic);
        }
        let version = c.u16()?;
        if version != VERSION {
            return Err(PairingProtocolError::UnsupportedVersion(version));
        }
        let kind = c.u8()?;
        let pairing_id = OpaqueId::from_bytes(c.array_16()?);
        let payload = match kind {
            1 => PairingPayload::Offer(decode_offer(&mut c)?),
            2 => PairingPayload::Approval(decode_approval(&mut c)?),
            3 => PairingPayload::Completion(PairingCompletion { transcript_digest: c.array_32()? }),
            4 => PairingPayload::Rejection(PairingRejection),
            5 => PairingPayload::Cancellation(PairingCancellation),
            6 => PairingPayload::CompletionAck(PairingCompletionAck {
                transcript_digest: c.array_32()?,
            }),
            _ => return Err(PairingProtocolError::UnknownPayloadKind(kind)),
        };
        if !c.is_empty() {
            return Err(PairingProtocolError::TrailingBytes);
        }
        let e = Self { pairing_id, payload };
        match &e.payload {
            PairingPayload::Offer(o) => o.validate()?,
            PairingPayload::Approval(a) => a.validate()?,
            _ => {}
        }
        Ok(e)
    }
    pub fn transcript_component(&self) -> Result<Vec<u8>, PairingProtocolError> {
        match self.payload {
            PairingPayload::Offer(_) => self.encode(),
            _ => Err(PairingProtocolError::NotAnOffer),
        }
    }
}
fn encode_offer(o: &PairingOffer, out: &mut Vec<u8>) -> Result<(), PairingProtocolError> {
    out.extend_from_slice(&o.identity_id.into_bytes());
    out.extend_from_slice(&o.key_id.into_bytes());
    out.extend_from_slice(&o.key_algorithm.to_be_bytes());
    put_bytes(&o.public_key, out)?;
    out.extend_from_slice(&o.key_generation.to_be_bytes());
    put_bytes(o.display_name.as_bytes(), out)?;
    put_bytes(o.onion_address.as_bytes(), out)?;
    out.extend_from_slice(&o.capability_id.into_bytes());
    out.extend_from_slice(&o.transcript_nonce);
    Ok(())
}
fn decode_offer(c: &mut Cursor<'_>) -> Result<PairingOffer, PairingProtocolError> {
    let identity_id = OpaqueId::from_bytes(c.array_16()?);
    let key_id = OpaqueId::from_bytes(c.array_16()?);
    let key_algorithm = c.u16()?;
    let public_key = c.bytes(MAX_PUBLIC_KEY_LEN)?;
    let key_generation = c.u32()?;
    let display_name = String::from_utf8(c.bytes(MAX_DISPLAY_NAME_LEN)?)
        .map_err(|_| PairingProtocolError::InvalidDisplayName)?;
    let onion_address = String::from_utf8(c.bytes(MAX_ONION_ADDRESS_LEN)?)
        .map_err(|_| PairingProtocolError::InvalidOnionAddress)?;
    let capability_id = OpaqueId::from_bytes(c.array_16()?);
    let transcript_nonce = c.array_32()?;
    Ok(PairingOffer {
        identity_id,
        key_id,
        key_algorithm,
        public_key,
        key_generation,
        display_name,
        onion_address,
        capability_id,
        transcript_nonce,
    })
}
fn encode_approval(a: &PairingApproval, out: &mut Vec<u8>) -> Result<(), PairingProtocolError> {
    out.extend_from_slice(&a.transcript_digest);
    put_bytes(&a.proof, out)
}
fn decode_approval(c: &mut Cursor<'_>) -> Result<PairingApproval, PairingProtocolError> {
    Ok(PairingApproval {
        transcript_digest: c.array_32()?,
        proof: c.bytes(MAX_APPROVAL_PROOF_LEN)?,
    })
}
fn put_bytes(v: &[u8], out: &mut Vec<u8>) -> Result<(), PairingProtocolError> {
    let len = u16::try_from(v.len()).map_err(|_| PairingProtocolError::FieldTooLarge)?;
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(v);
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
    fn take(&mut self, len: usize) -> Result<&'a [u8], PairingProtocolError> {
        let end = self.offset.checked_add(len).ok_or(PairingProtocolError::Truncated)?;
        if end > self.bytes.len() {
            return Err(PairingProtocolError::Truncated);
        }
        let v = &self.bytes[self.offset..end];
        self.offset = end;
        Ok(v)
    }
    fn u8(&mut self) -> Result<u8, PairingProtocolError> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, PairingProtocolError> {
        Ok(u16::from_be_bytes(
            self.take(2)?.try_into().map_err(|_| PairingProtocolError::Truncated)?,
        ))
    }
    fn u32(&mut self) -> Result<u32, PairingProtocolError> {
        Ok(u32::from_be_bytes(
            self.take(4)?.try_into().map_err(|_| PairingProtocolError::Truncated)?,
        ))
    }
    fn array_16(&mut self) -> Result<[u8; 16], PairingProtocolError> {
        self.take(16)?.try_into().map_err(|_| PairingProtocolError::Truncated)
    }
    fn array_32(&mut self) -> Result<[u8; 32], PairingProtocolError> {
        self.take(32)?.try_into().map_err(|_| PairingProtocolError::Truncated)
    }
    fn bytes(&mut self, max: usize) -> Result<Vec<u8>, PairingProtocolError> {
        let len = usize::from(self.u16()?);
        if len > max {
            return Err(PairingProtocolError::FieldTooLarge);
        }
        Ok(self.take(len)?.to_vec())
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
    InvalidDisplayName,
    InvalidOnionAddress,
    InvalidApprovalProofLength,
    PayloadTooLarge { actual: usize },
    FieldTooLarge,
    Truncated,
    TrailingBytes,
    NotAnOffer,
}
impl fmt::Display for PairingProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for PairingProtocolError {}
#[cfg(test)]
mod tests {
    use super::{PairingCompletionAck, PairingEnvelope, PairingOffer, PairingPayload};
    use torca_foundation::OpaqueId;
    fn offer(pairing: u128) -> PairingEnvelope {
        PairingEnvelope {
            pairing_id: OpaqueId::from_u128(pairing),
            payload: PairingPayload::Offer(PairingOffer {
                identity_id: OpaqueId::from_u128(2),
                key_id: OpaqueId::from_u128(3),
                key_algorithm: 1,
                public_key: vec![7; 32],
                key_generation: 0,
                display_name: "Orca".into(),
                onion_address: format!("{}.onion", "a".repeat(56)),
                capability_id: OpaqueId::from_u128(4),
                transcript_nonce: [9; 32],
            }),
        }
    }
    #[test]
    fn offer_round_trips_and_is_bound_to_pairing_id() {
        let e = offer(1);
        let encoded = e.encode().expect("encode");
        let decoded = PairingEnvelope::decode(&encoded).expect("decode");
        assert_eq!(decoded, e);
        assert!(decoded.validate_pairing_id(OpaqueId::from_u128(1)).is_ok());
        assert!(decoded.validate_pairing_id(OpaqueId::from_u128(99)).is_err());
    }

    #[test]
    fn completion_ack_round_trips_exactly() {
        let envelope = PairingEnvelope {
            pairing_id: OpaqueId::from_u128(1),
            payload: PairingPayload::CompletionAck(PairingCompletionAck {
                transcript_digest: [7; 32],
            }),
        };
        let encoded = envelope.encode().expect("encode");
        assert_eq!(PairingEnvelope::decode(&encoded).expect("decode"), envelope);
    }
}
