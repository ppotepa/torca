//! Authenticated peer-session protocol vocabulary and bounded payload codec.

use core::fmt;
use torca_foundation::{OpaqueId, Timestamp};

/// Maximum encrypted application payload carried by one peer message.
pub const MAX_PEER_DATA_LEN: usize = 4 * 1024 * 1024;
/// Maximum handshake signature/proof length.
pub const MAX_PROOF_LEN: usize = 512;
/// Maximum provider wire-name length in a route advertisement.
pub const MAX_ROUTE_PROVIDER_LEN: usize = 32;
/// Maximum opaque endpoint hint carried by a route advertisement.
pub const MAX_ROUTE_ENDPOINT_LEN: usize = 8 * 1024;

/// Stable application payload discriminants carried inside `PeerMessage::Data`.
///
/// Values are append-only for the current peer wire generation. Existing kinds
/// must not be renumbered without an explicit protocol-version migration.
#[repr(u16)]
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PeerApplicationKind {
    Text = 1,
    Receipt = 2,
    Attachment = 3,
    Probe = 4,
    RadioControl = 5,
    Reaction = 6,
}
impl PeerApplicationKind {
    pub const ALL: [Self; 6] = [
        Self::Text,
        Self::Receipt,
        Self::Attachment,
        Self::Probe,
        Self::RadioControl,
        Self::Reaction,
    ];

    pub const fn as_u16(self) -> u16 {
        self as u16
    }
}

/// Protocol-level acknowledgement distinct from user delivery receipts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AckStatus {
    Accepted,
    Duplicate,
    Rejected,
}
impl AckStatus {
    const fn to_u8(self) -> u8 {
        match self {
            Self::Accepted => 1,
            Self::Duplicate => 2,
            Self::Rejected => 3,
        }
    }
    fn from_u8(value: u8) -> Result<Self, PeerProtocolError> {
        match value {
            1 => Ok(Self::Accepted),
            2 => Ok(Self::Duplicate),
            3 => Ok(Self::Rejected),
            _ => Err(PeerProtocolError::Malformed),
        }
    }
}

/// Initial authenticated handshake message.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HandshakeHello {
    pub session_id: OpaqueId,
    pub identity_id: OpaqueId,
    pub capability_id: OpaqueId,
    pub issued_at: Timestamp,
    pub nonce: [u8; 32],
    pub proof: Vec<u8>,
}
impl HandshakeHello {
    /// Builds and signs the canonical initiator hello without exposing signing-key bytes.
    pub fn signed<S: HandshakeSigner>(
        session_id: OpaqueId,
        identity_id: OpaqueId,
        capability_id: OpaqueId,
        issued_at: Timestamp,
        nonce: [u8; 32],
        signer: &S,
    ) -> Result<Self, HandshakeBuildError> {
        let mut hello =
            Self { session_id, identity_id, capability_id, issued_at, nonce, proof: Vec::new() };
        let proof = signer.sign(&canonical_hello_bytes(&hello))?;
        if proof.len() > MAX_PROOF_LEN {
            return Err(HandshakeBuildError::ProofTooLarge);
        }
        hello.proof = proof;
        Ok(hello)
    }
}

/// Authenticated acknowledgement echoing the initiator challenge.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HandshakeAck {
    pub session_id: OpaqueId,
    pub nonce: [u8; 32],
    pub proof: Vec<u8>,
}

/// Provider-owned route update sent after an authenticated session is ready.
///
/// The peer protocol transports the provider name and opaque endpoint bytes,
/// but never parses them. A selected provider decides how to encode and use
/// the endpoint. Because this message is sent on the authenticated peer
/// stream, it can safely refresh direct routes after a network migration.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteAdvertisement {
    pub provider: String,
    pub generation: u64,
    pub endpoint: Vec<u8>,
}
impl RouteAdvertisement {
    pub fn new(
        provider: impl Into<String>,
        generation: u64,
        endpoint: Vec<u8>,
    ) -> Result<Self, PeerProtocolError> {
        let provider = provider.into();
        if provider.is_empty()
            || provider.len() > MAX_ROUTE_PROVIDER_LEN
            || provider.chars().any(char::is_whitespace)
            || endpoint.is_empty()
            || endpoint.len() > MAX_ROUTE_ENDPOINT_LEN
        {
            return Err(PeerProtocolError::Malformed);
        }
        Ok(Self { provider, generation, endpoint })
    }
}
impl HandshakeAck {
    /// Builds and signs a canonical responder acknowledgement.
    pub fn signed<S: HandshakeSigner>(
        session_id: OpaqueId,
        nonce: [u8; 32],
        signer: &S,
    ) -> Result<Self, HandshakeBuildError> {
        let mut ack = Self { session_id, nonce, proof: Vec::new() };
        let proof = signer.sign(&canonical_ack_bytes(&ack))?;
        if proof.len() > MAX_PROOF_LEN {
            return Err(HandshakeBuildError::ProofTooLarge);
        }
        ack.proof = proof;
        Ok(ack)
    }
}

/// Signing boundary used by canonical handshake builders.
pub trait HandshakeSigner {
    /// Signs canonical handshake bytes while retaining private key material behind the adapter.
    fn sign(&self, canonical: &[u8]) -> Result<Vec<u8>, HandshakeSigningError>;
}

/// Redaction-safe signing adapter failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HandshakeSigningError(pub String);
impl fmt::Display for HandshakeSigningError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}
impl std::error::Error for HandshakeSigningError {}

/// Failure to construct a locally authenticated handshake frame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HandshakeBuildError {
    Signing(HandshakeSigningError),
    ProofTooLarge,
}
impl fmt::Display for HandshakeBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for HandshakeBuildError {}
impl From<HandshakeSigningError> for HandshakeBuildError {
    fn from(value: HandshakeSigningError) -> Self {
        Self::Signing(value)
    }
}

/// Peer protocol payload.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PeerMessage {
    Hello(HandshakeHello),
    HelloAck(HandshakeAck),
    Data { envelope_id: OpaqueId, message_kind: u16, ciphertext: Vec<u8> },
    Ack { envelope_id: OpaqueId, status: AckStatus },
    Ping(u64),
    Pong(u64),
    Route(RouteAdvertisement),
}

/// Signature verification port owned by the protocol boundary.
pub trait HandshakeVerifier {
    /// Verifies canonical handshake bytes.
    fn verify(&self, canonical: &[u8], proof: &[u8]) -> bool;
}

/// Expected peer binding and freshness policy.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HandshakePolicy {
    pub expected_identity: OpaqueId,
    pub expected_capability: OpaqueId,
    pub max_clock_skew_ms: i64,
}
impl HandshakePolicy {
    /// Validates identity, capability, freshness and proof.
    pub fn validate_hello<V: HandshakeVerifier>(
        &self,
        hello: &HandshakeHello,
        now: Timestamp,
        verifier: &V,
    ) -> Result<(), PeerProtocolError> {
        if hello.identity_id != self.expected_identity {
            return Err(PeerProtocolError::IdentityMismatch);
        }
        if hello.capability_id != self.expected_capability {
            return Err(PeerProtocolError::CapabilityMismatch);
        }
        let delta = now.to_unix_millis().abs_diff(hello.issued_at.to_unix_millis());
        if delta > self.max_clock_skew_ms.unsigned_abs() {
            return Err(PeerProtocolError::StaleHandshake);
        }
        if hello.proof.len() > MAX_PROOF_LEN {
            return Err(PeerProtocolError::ProofTooLarge);
        }
        if !verifier.verify(&canonical_hello_bytes(hello), &hello.proof) {
            return Err(PeerProtocolError::InvalidProof);
        }
        Ok(())
    }
    /// Validates that an acknowledgement proves and echoes the initiator challenge.
    pub fn validate_ack<V: HandshakeVerifier>(
        &self,
        ack: &HandshakeAck,
        expected_session: OpaqueId,
        expected_nonce: [u8; 32],
        verifier: &V,
    ) -> Result<(), PeerProtocolError> {
        if ack.session_id != expected_session || ack.nonce != expected_nonce {
            return Err(PeerProtocolError::ChallengeMismatch);
        }
        if ack.proof.len() > MAX_PROOF_LEN {
            return Err(PeerProtocolError::ProofTooLarge);
        }
        if !verifier.verify(&canonical_ack_bytes(ack), &ack.proof) {
            return Err(PeerProtocolError::InvalidProof);
        }
        Ok(())
    }
}

/// Peer payload codec error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PeerProtocolError {
    Truncated,
    Malformed,
    PayloadTooLarge { actual: usize },
    ProofTooLarge,
    IdentityMismatch,
    CapabilityMismatch,
    StaleHandshake,
    InvalidProof,
    ChallengeMismatch,
    TrailingBytes,
}
impl fmt::Display for PeerProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for PeerProtocolError {}

/// Canonical bytes signed for a hello, excluding its proof.
pub fn canonical_hello_bytes(hello: &HandshakeHello) -> Vec<u8> {
    let mut output = Vec::new();
    output.extend_from_slice(b"TORCA-PEER-HELLO-V1");
    encode_hello_core(hello, &mut output);
    output
}
/// Canonical bytes signed for an acknowledgement, excluding its proof.
pub fn canonical_ack_bytes(ack: &HandshakeAck) -> Vec<u8> {
    let mut output = Vec::new();
    output.extend_from_slice(b"TORCA-PEER-ACK-V1");
    output.extend_from_slice(ack.session_id.as_bytes());
    output.extend_from_slice(&ack.nonce);
    output
}

/// Deterministic peer-payload codec. Outer framing belongs to `torca-wire`.
pub struct PeerCodec;
impl PeerCodec {
    /// Encodes one peer payload.
    pub fn encode(message: &PeerMessage) -> Result<Vec<u8>, PeerProtocolError> {
        let mut output = Vec::new();
        match message {
            PeerMessage::Hello(hello) => {
                if hello.proof.len() > MAX_PROOF_LEN {
                    return Err(PeerProtocolError::ProofTooLarge);
                }
                output.push(1);
                encode_hello_core(hello, &mut output);
                write_bytes(&hello.proof, &mut output)?;
            }
            PeerMessage::HelloAck(ack) => {
                if ack.proof.len() > MAX_PROOF_LEN {
                    return Err(PeerProtocolError::ProofTooLarge);
                }
                output.push(2);
                output.extend_from_slice(ack.session_id.as_bytes());
                output.extend_from_slice(&ack.nonce);
                write_bytes(&ack.proof, &mut output)?;
            }
            PeerMessage::Data { envelope_id, message_kind, ciphertext } => {
                if ciphertext.len() > MAX_PEER_DATA_LEN {
                    return Err(PeerProtocolError::PayloadTooLarge { actual: ciphertext.len() });
                }
                output.push(3);
                output.extend_from_slice(envelope_id.as_bytes());
                output.extend_from_slice(&message_kind.to_be_bytes());
                write_bytes(ciphertext, &mut output)?;
            }
            PeerMessage::Ack { envelope_id, status } => {
                output.push(4);
                output.extend_from_slice(envelope_id.as_bytes());
                output.push(status.to_u8());
            }
            PeerMessage::Ping(value) => {
                output.push(5);
                output.extend_from_slice(&value.to_be_bytes());
            }
            PeerMessage::Pong(value) => {
                output.push(6);
                output.extend_from_slice(&value.to_be_bytes());
            }
            PeerMessage::Route(route) => {
                // RouteAdvertisement::new is the public validation boundary,
                // but encode defensively for callers constructing the enum
                // directly.
                if route.provider.is_empty()
                    || route.provider.len() > MAX_ROUTE_PROVIDER_LEN
                    || route.provider.chars().any(char::is_whitespace)
                    || route.endpoint.is_empty()
                    || route.endpoint.len() > MAX_ROUTE_ENDPOINT_LEN
                {
                    return Err(PeerProtocolError::Malformed);
                }
                output.push(7);
                write_bytes(route.provider.as_bytes(), &mut output)?;
                output.extend_from_slice(&route.generation.to_be_bytes());
                write_bytes(&route.endpoint, &mut output)?;
            }
        }
        Ok(output)
    }
    /// Decodes one exact payload.
    pub fn decode(input: &[u8]) -> Result<PeerMessage, PeerProtocolError> {
        let mut cursor = Cursor::new(input);
        let message = match cursor.u8()? {
            1 => PeerMessage::Hello(HandshakeHello {
                session_id: cursor.id()?,
                identity_id: cursor.id()?,
                capability_id: cursor.id()?,
                issued_at: Timestamp::from_unix_millis(cursor.i64()?)
                    .map_err(|_| PeerProtocolError::Malformed)?,
                nonce: cursor.array_32()?,
                proof: cursor.bytes(MAX_PROOF_LEN)?,
            }),
            2 => PeerMessage::HelloAck(HandshakeAck {
                session_id: cursor.id()?,
                nonce: cursor.array_32()?,
                proof: cursor.bytes(MAX_PROOF_LEN)?,
            }),
            3 => PeerMessage::Data {
                envelope_id: cursor.id()?,
                message_kind: cursor.u16()?,
                ciphertext: cursor.bytes(MAX_PEER_DATA_LEN)?,
            },
            4 => PeerMessage::Ack {
                envelope_id: cursor.id()?,
                status: AckStatus::from_u8(cursor.u8()?)?,
            },
            5 => PeerMessage::Ping(cursor.u64()?),
            6 => PeerMessage::Pong(cursor.u64()?),
            7 => {
                let provider = String::from_utf8(cursor.bytes(MAX_ROUTE_PROVIDER_LEN)?)
                    .map_err(|_| PeerProtocolError::Malformed)?;
                let generation = cursor.u64()?;
                let endpoint = cursor.bytes(MAX_ROUTE_ENDPOINT_LEN)?;
                RouteAdvertisement::new(provider, generation, endpoint).map(PeerMessage::Route)?
            }
            _ => return Err(PeerProtocolError::Malformed),
        };
        if cursor.remaining() != 0 {
            return Err(PeerProtocolError::TrailingBytes);
        }
        Ok(message)
    }
}
fn encode_hello_core(hello: &HandshakeHello, output: &mut Vec<u8>) {
    output.extend_from_slice(hello.session_id.as_bytes());
    output.extend_from_slice(hello.identity_id.as_bytes());
    output.extend_from_slice(hello.capability_id.as_bytes());
    output.extend_from_slice(&hello.issued_at.to_unix_millis().to_be_bytes());
    output.extend_from_slice(&hello.nonce);
}
fn write_bytes(value: &[u8], output: &mut Vec<u8>) -> Result<(), PeerProtocolError> {
    let length = u32::try_from(value.len())
        .map_err(|_| PeerProtocolError::PayloadTooLarge { actual: value.len() })?;
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(value);
    Ok(())
}
struct Cursor<'a> {
    input: &'a [u8],
    offset: usize,
}
impl<'a> Cursor<'a> {
    const fn new(input: &'a [u8]) -> Self {
        Self { input, offset: 0 }
    }
    fn remaining(&self) -> usize {
        self.input.len().saturating_sub(self.offset)
    }
    fn take(&mut self, length: usize) -> Result<&'a [u8], PeerProtocolError> {
        let end = self.offset.checked_add(length).ok_or(PeerProtocolError::Malformed)?;
        let value = self.input.get(self.offset..end).ok_or(PeerProtocolError::Truncated)?;
        self.offset = end;
        Ok(value)
    }
    fn u8(&mut self) -> Result<u8, PeerProtocolError> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, PeerProtocolError> {
        Ok(u16::from_be_bytes(self.take(2)?.try_into().map_err(|_| PeerProtocolError::Truncated)?))
    }
    fn u32(&mut self) -> Result<u32, PeerProtocolError> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().map_err(|_| PeerProtocolError::Truncated)?))
    }
    fn u64(&mut self) -> Result<u64, PeerProtocolError> {
        Ok(u64::from_be_bytes(self.take(8)?.try_into().map_err(|_| PeerProtocolError::Truncated)?))
    }
    fn i64(&mut self) -> Result<i64, PeerProtocolError> {
        Ok(i64::from_be_bytes(self.take(8)?.try_into().map_err(|_| PeerProtocolError::Truncated)?))
    }
    fn id(&mut self) -> Result<OpaqueId, PeerProtocolError> {
        Ok(OpaqueId::from_bytes(
            self.take(16)?.try_into().map_err(|_| PeerProtocolError::Truncated)?,
        ))
    }
    fn array_32(&mut self) -> Result<[u8; 32], PeerProtocolError> {
        self.take(32)?.try_into().map_err(|_| PeerProtocolError::Truncated)
    }
    fn bytes(&mut self, maximum: usize) -> Result<Vec<u8>, PeerProtocolError> {
        let length = usize::try_from(self.u32()?).map_err(|_| PeerProtocolError::Malformed)?;
        if length > maximum {
            return Err(PeerProtocolError::PayloadTooLarge { actual: length });
        }
        Ok(self.take(length)?.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::{PeerApplicationKind, PeerCodec, PeerMessage, RouteAdvertisement};
    use std::collections::BTreeSet;

    #[test]
    fn application_kind_discriminants_are_unique_and_stable() {
        let values = PeerApplicationKind::ALL.map(PeerApplicationKind::as_u16);
        assert_eq!(values.iter().copied().collect::<BTreeSet<_>>().len(), values.len());
        assert_eq!(PeerApplicationKind::Text.as_u16(), 1);
        assert_eq!(PeerApplicationKind::Receipt.as_u16(), 2);
        assert_eq!(PeerApplicationKind::Attachment.as_u16(), 3);
        assert_eq!(PeerApplicationKind::Probe.as_u16(), 4);
        assert_eq!(PeerApplicationKind::RadioControl.as_u16(), 5);
        assert_eq!(PeerApplicationKind::Reaction.as_u16(), 6);
    }

    #[test]
    fn route_advertisement_round_trips_opaque_provider_bytes() {
        let route = RouteAdvertisement::new("iroh", 42, vec![0, 1, 2, 255]).expect("route");
        let encoded = PeerCodec::encode(&PeerMessage::Route(route.clone())).expect("encode");
        let decoded = PeerCodec::decode(&encoded).expect("decode");
        assert_eq!(decoded, PeerMessage::Route(route));
    }

    #[test]
    fn route_advertisement_rejects_unbounded_or_ambiguous_provider_data() {
        assert!(RouteAdvertisement::new("iroh direct", 1, vec![1]).is_err());
        assert!(RouteAdvertisement::new("iroh", 1, Vec::new()).is_err());
        assert!(
            RouteAdvertisement::new("iroh", 1, vec![1; super::MAX_ROUTE_ENDPOINT_LEN + 1]).is_err()
        );
    }
}
