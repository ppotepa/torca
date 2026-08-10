//! Versioned opaque rendezvous-relay protocol and strict binary wire codec.

use core::fmt;

use torca_foundation::{OpaqueId, Timestamp};

/// Maximum opaque blob size relayed for one operation.
pub const MAX_RELAY_BLOB_LEN: usize = 64 * 1024;
/// Maximum number of blobs returned by one poll response.
pub const MAX_RELAY_BATCH_BLOBS: usize = 32;
/// Fixed relay frame header length.
pub const RELAY_HEADER_LEN: usize = 12;
/// Maximum encoded frame length, including a full bounded poll batch.
pub const MAX_RELAY_FRAME_LEN: usize =
    RELAY_HEADER_LEN + 4 + MAX_RELAY_BATCH_BLOBS * (4 + MAX_RELAY_BLOB_LEN);

const RELAY_MAGIC: &[u8; 4] = b"TCRL";
const REQUEST_DIRECTION: u8 = 1;
const RESPONSE_DIRECTION: u8 = 2;
const CROCKFORD_BASE32: &str = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// Protocol version.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RelayProtocolVersion(pub u16);
impl RelayProtocolVersion {
    /// Capability-authenticated rendezvous protocol.
    pub const V3: Self = Self(3);
}

/// Relay slot ID. It is an address, not an authorization secret.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RelaySlotId(pub OpaqueId);

/// Opaque capability that authorizes destructive slot administration.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RelaySlotCapability(pub OpaqueId);

/// Opaque per-side capability used for push/poll operations.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RelaySideToken(pub OpaqueId);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RelayJoinTicket(pub [u8; 16]);

/// Validated rendezvous code independent from the pairing domain model.
#[must_use]
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RelayCode(String);
impl RelayCode {
    /// Creates the exact six-character Crockford Base32 invitation code.
    pub fn new(value: impl Into<String>) -> Result<Self, RelayProtocolError> {
        let mut value = value
            .into()
            .chars()
            .filter(|character| !matches!(character, '-' | ' '))
            .collect::<String>()
            .to_ascii_uppercase();
        value = value
            .chars()
            .map(|character| match character {
                'O' => '0',
                'I' | 'L' => '1',
                other => other,
            })
            .collect();
        if value.len() != 6 || !value.chars().all(|character| CROCKFORD_BASE32.contains(character))
        {
            return Err(RelayProtocolError::InvalidCode);
        }
        Ok(Self(value))
    }
    /// Returns text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Opaque relay request.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RelayRequest {
    /// Unauthenticated protocol-level health check.
    Health,
    /// Opens a short-lived slot. All capabilities are generated client-side with a CSPRNG.
    Open {
        code: RelayCode,
        expires_at: Timestamp,
        creator_blob: Vec<u8>,
        slot_capability: RelaySlotCapability,
        creator_token: RelaySideToken,
        ticket: RelayJoinTicket,
    },
    /// Joins a slot and installs the joiner's client-generated side token.
    Join {
        code: RelayCode,
        joiner_blob: Vec<u8>,
        joiner_token: RelaySideToken,
        ticket: Option<RelayJoinTicket>,
    },
    /// Pushes an opaque blob to the opposite side. Side is inferred from the capability.
    Push { slot_id: RelaySlotId, token: RelaySideToken, blob: Vec<u8> },
    /// Polls queued blobs for the authenticated side.
    Poll { slot_id: RelaySlotId, token: RelaySideToken },
    /// Closes a slot using its separate administrative capability.
    Close { slot_id: RelaySlotId, capability: RelaySlotCapability },
}

/// Side of the rendezvous slot after capability authentication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelaySide {
    Creator,
    Joiner,
}

/// Opaque relay response.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RelayResponse {
    Healthy,
    Opened {
        slot_id: RelaySlotId,
        /// Relay-clock deadline. Clients project this value instead of trusting
        /// their own wall clock for invitation expiry.
        expires_at: Timestamp,
    },
    Joined {
        slot_id: RelaySlotId,
        /// Relay-clock deadline shared with the joining client.
        expires_at: Timestamp,
        creator_blob: Vec<u8>,
    },
    Accepted,
    Blobs(Vec<Vec<u8>>),
    Closed,
    /// Application-level relay rejection. Transport succeeded and callers must not retry blindly.
    Error(RelayProtocolError),
}

/// Relay protocol error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RelayProtocolError {
    InvalidCode,
    BlobTooLarge { actual: usize },
    SlotNotFound,
    SlotExpired,
    SlotAlreadyJoined,
    Unauthorized,
    QueueFull,
    InvalidOperation,
}
impl fmt::Display for RelayProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for RelayProtocolError {}

/// Strict frame codec failure. These errors describe malformed transport bytes rather than a
/// semantically rejected relay operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RelayCodecError {
    Truncated,
    InvalidMagic,
    UnsupportedVersion(u16),
    InvalidDirection,
    UnknownKind(u8),
    InvalidField,
    PayloadTooLarge { actual: usize },
    TooManyBlobs { actual: usize },
    TrailingBytes,
}
impl fmt::Display for RelayCodecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for RelayCodecError {}

/// Deterministic binary codec used on every relay network transport.
pub struct RelayCodec;
impl RelayCodec {
    /// Encodes one exact request frame.
    pub fn encode_request(request: &RelayRequest) -> Result<Vec<u8>, RelayCodecError> {
        let mut payload = Vec::new();
        let kind = match request {
            RelayRequest::Health => 0,
            RelayRequest::Open {
                code,
                expires_at,
                creator_blob,
                slot_capability,
                creator_token,
                ticket,
            } => {
                put_code(code, &mut payload)?;
                payload.extend_from_slice(&expires_at.to_unix_millis().to_be_bytes());
                put_blob(creator_blob, &mut payload)?;
                payload.extend_from_slice(slot_capability.0.as_bytes());
                payload.extend_from_slice(creator_token.0.as_bytes());
                payload.extend_from_slice(&ticket.0);
                1
            }
            RelayRequest::Join { code, joiner_blob, joiner_token, ticket } => {
                put_code(code, &mut payload)?;
                put_blob(joiner_blob, &mut payload)?;
                payload.extend_from_slice(joiner_token.0.as_bytes());
                payload.push(u8::from(ticket.is_some()));
                if let Some(ticket) = ticket {
                    payload.extend_from_slice(&ticket.0);
                }
                2
            }
            RelayRequest::Push { slot_id, token, blob } => {
                payload.extend_from_slice(slot_id.0.as_bytes());
                payload.extend_from_slice(token.0.as_bytes());
                put_blob(blob, &mut payload)?;
                3
            }
            RelayRequest::Poll { slot_id, token } => {
                payload.extend_from_slice(slot_id.0.as_bytes());
                payload.extend_from_slice(token.0.as_bytes());
                4
            }
            RelayRequest::Close { slot_id, capability } => {
                payload.extend_from_slice(slot_id.0.as_bytes());
                payload.extend_from_slice(capability.0.as_bytes());
                5
            }
        };
        encode_frame(REQUEST_DIRECTION, kind, &payload)
    }

    /// Decodes one exact request frame and rejects trailing bytes.
    pub fn decode_request(bytes: &[u8]) -> Result<RelayRequest, RelayCodecError> {
        let (direction, kind, payload) = decode_frame(bytes)?;
        if direction != REQUEST_DIRECTION {
            return Err(RelayCodecError::InvalidDirection);
        }
        let mut cursor = Cursor::new(payload);
        let request = match kind {
            0 => RelayRequest::Health,
            1 => RelayRequest::Open {
                code: cursor.code()?,
                expires_at: cursor.timestamp()?,
                creator_blob: cursor.blob()?,
                slot_capability: RelaySlotCapability(cursor.id()?),
                creator_token: RelaySideToken(cursor.id()?),
                ticket: RelayJoinTicket(cursor.fixed16()?),
            },
            2 => RelayRequest::Join {
                code: cursor.code()?,
                joiner_blob: cursor.blob()?,
                joiner_token: RelaySideToken(cursor.id()?),
                ticket: match cursor.byte()? {
                    0 => None,
                    1 => Some(RelayJoinTicket(cursor.fixed16()?)),
                    _ => return Err(RelayCodecError::InvalidField),
                },
            },
            3 => RelayRequest::Push {
                slot_id: RelaySlotId(cursor.id()?),
                token: RelaySideToken(cursor.id()?),
                blob: cursor.blob()?,
            },
            4 => RelayRequest::Poll {
                slot_id: RelaySlotId(cursor.id()?),
                token: RelaySideToken(cursor.id()?),
            },
            5 => RelayRequest::Close {
                slot_id: RelaySlotId(cursor.id()?),
                capability: RelaySlotCapability(cursor.id()?),
            },
            value => return Err(RelayCodecError::UnknownKind(value)),
        };
        cursor.finish()?;
        Ok(request)
    }

    /// Encodes one exact response frame.
    pub fn encode_response(response: &RelayResponse) -> Result<Vec<u8>, RelayCodecError> {
        let mut payload = Vec::new();
        let kind = match response {
            RelayResponse::Healthy => 7,
            RelayResponse::Opened { slot_id, expires_at } => {
                payload.extend_from_slice(slot_id.0.as_bytes());
                payload.extend_from_slice(&expires_at.to_unix_millis().to_be_bytes());
                1
            }
            RelayResponse::Joined { slot_id, expires_at, creator_blob } => {
                payload.extend_from_slice(slot_id.0.as_bytes());
                payload.extend_from_slice(&expires_at.to_unix_millis().to_be_bytes());
                put_blob(creator_blob, &mut payload)?;
                2
            }
            RelayResponse::Accepted => 3,
            RelayResponse::Blobs(blobs) => {
                if blobs.len() > MAX_RELAY_BATCH_BLOBS {
                    return Err(RelayCodecError::TooManyBlobs { actual: blobs.len() });
                }
                let count =
                    u16::try_from(blobs.len()).map_err(|_| RelayCodecError::InvalidField)?;
                payload.extend_from_slice(&count.to_be_bytes());
                for blob in blobs {
                    put_blob(blob, &mut payload)?;
                }
                4
            }
            RelayResponse::Closed => 5,
            RelayResponse::Error(error) => {
                encode_protocol_error(error, &mut payload)?;
                6
            }
        };
        encode_frame(RESPONSE_DIRECTION, kind, &payload)
    }

    /// Decodes one exact response frame and rejects trailing bytes.
    pub fn decode_response(bytes: &[u8]) -> Result<RelayResponse, RelayCodecError> {
        let (direction, kind, payload) = decode_frame(bytes)?;
        if direction != RESPONSE_DIRECTION {
            return Err(RelayCodecError::InvalidDirection);
        }
        let mut cursor = Cursor::new(payload);
        let response = match kind {
            7 => RelayResponse::Healthy,
            1 => RelayResponse::Opened {
                slot_id: RelaySlotId(cursor.id()?),
                expires_at: cursor.timestamp()?,
            },
            2 => RelayResponse::Joined {
                slot_id: RelaySlotId(cursor.id()?),
                expires_at: cursor.timestamp()?,
                creator_blob: cursor.blob()?,
            },
            3 => RelayResponse::Accepted,
            4 => {
                let count = usize::from(cursor.u16()?);
                if count > MAX_RELAY_BATCH_BLOBS {
                    return Err(RelayCodecError::TooManyBlobs { actual: count });
                }
                let mut blobs = Vec::with_capacity(count);
                for _ in 0..count {
                    blobs.push(cursor.blob()?);
                }
                RelayResponse::Blobs(blobs)
            }
            5 => RelayResponse::Closed,
            6 => RelayResponse::Error(cursor.protocol_error()?),
            value => return Err(RelayCodecError::UnknownKind(value)),
        };
        cursor.finish()?;
        Ok(response)
    }

    /// Reads and validates the complete frame length from a fixed relay header.
    pub fn frame_len_from_header(
        header: &[u8; RELAY_HEADER_LEN],
    ) -> Result<usize, RelayCodecError> {
        validate_header_prefix(header)?;
        let payload_len =
            u32::from_be_bytes(header[8..12].try_into().map_err(|_| RelayCodecError::Truncated)?);
        let payload_len =
            usize::try_from(payload_len).map_err(|_| RelayCodecError::InvalidField)?;
        let frame_len =
            RELAY_HEADER_LEN.checked_add(payload_len).ok_or(RelayCodecError::InvalidField)?;
        if frame_len > MAX_RELAY_FRAME_LEN {
            return Err(RelayCodecError::PayloadTooLarge { actual: payload_len });
        }
        Ok(frame_len)
    }
}

/// Validates an opaque blob before relay storage or forwarding.
pub fn validate_blob(blob: &[u8]) -> Result<(), RelayProtocolError> {
    if blob.len() > MAX_RELAY_BLOB_LEN {
        Err(RelayProtocolError::BlobTooLarge { actual: blob.len() })
    } else {
        Ok(())
    }
}

fn encode_frame(direction: u8, kind: u8, payload: &[u8]) -> Result<Vec<u8>, RelayCodecError> {
    let payload_len = u32::try_from(payload.len())
        .map_err(|_| RelayCodecError::PayloadTooLarge { actual: payload.len() })?;
    let frame_len =
        RELAY_HEADER_LEN.checked_add(payload.len()).ok_or(RelayCodecError::InvalidField)?;
    if frame_len > MAX_RELAY_FRAME_LEN {
        return Err(RelayCodecError::PayloadTooLarge { actual: payload.len() });
    }
    let mut output = Vec::with_capacity(frame_len);
    output.extend_from_slice(RELAY_MAGIC);
    output.extend_from_slice(&RelayProtocolVersion::V3.0.to_be_bytes());
    output.push(direction);
    output.push(kind);
    output.extend_from_slice(&payload_len.to_be_bytes());
    output.extend_from_slice(payload);
    Ok(output)
}

fn decode_frame(bytes: &[u8]) -> Result<(u8, u8, &[u8]), RelayCodecError> {
    if bytes.len() < RELAY_HEADER_LEN {
        return Err(RelayCodecError::Truncated);
    }
    let header: &[u8; RELAY_HEADER_LEN] =
        bytes[..RELAY_HEADER_LEN].try_into().map_err(|_| RelayCodecError::Truncated)?;
    let frame_len = RelayCodec::frame_len_from_header(header)?;
    if bytes.len() < frame_len {
        return Err(RelayCodecError::Truncated);
    }
    if bytes.len() != frame_len {
        return Err(RelayCodecError::TrailingBytes);
    }
    Ok((header[6], header[7], &bytes[RELAY_HEADER_LEN..]))
}

fn validate_header_prefix(header: &[u8; RELAY_HEADER_LEN]) -> Result<(), RelayCodecError> {
    if &header[..4] != RELAY_MAGIC {
        return Err(RelayCodecError::InvalidMagic);
    }
    let version =
        u16::from_be_bytes(header[4..6].try_into().map_err(|_| RelayCodecError::Truncated)?);
    if version != RelayProtocolVersion::V3.0 {
        return Err(RelayCodecError::UnsupportedVersion(version));
    }
    if !matches!(header[6], REQUEST_DIRECTION | RESPONSE_DIRECTION) {
        return Err(RelayCodecError::InvalidDirection);
    }
    Ok(())
}

fn put_code(code: &RelayCode, output: &mut Vec<u8>) -> Result<(), RelayCodecError> {
    let bytes = code.as_str().as_bytes();
    let length = u8::try_from(bytes.len()).map_err(|_| RelayCodecError::InvalidField)?;
    output.push(length);
    output.extend_from_slice(bytes);
    Ok(())
}

fn put_blob(blob: &[u8], output: &mut Vec<u8>) -> Result<(), RelayCodecError> {
    if blob.len() > MAX_RELAY_BLOB_LEN {
        return Err(RelayCodecError::PayloadTooLarge { actual: blob.len() });
    }
    let length = u32::try_from(blob.len())
        .map_err(|_| RelayCodecError::PayloadTooLarge { actual: blob.len() })?;
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(blob);
    Ok(())
}

fn encode_protocol_error(
    error: &RelayProtocolError,
    output: &mut Vec<u8>,
) -> Result<(), RelayCodecError> {
    let (code, actual) = match error {
        RelayProtocolError::InvalidCode => (1, 0),
        RelayProtocolError::BlobTooLarge { actual } => {
            let actual = u32::try_from(*actual).map_err(|_| RelayCodecError::InvalidField)?;
            (2, actual)
        }
        RelayProtocolError::SlotNotFound => (3, 0),
        RelayProtocolError::SlotExpired => (4, 0),
        RelayProtocolError::SlotAlreadyJoined => (5, 0),
        RelayProtocolError::Unauthorized => (6, 0),
        RelayProtocolError::QueueFull => (7, 0),
        RelayProtocolError::InvalidOperation => (8, 0),
    };
    output.push(code);
    output.extend_from_slice(&actual.to_be_bytes());
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

    fn take(&mut self, length: usize) -> Result<&'a [u8], RelayCodecError> {
        let end = self.offset.checked_add(length).ok_or(RelayCodecError::InvalidField)?;
        let value = self.bytes.get(self.offset..end).ok_or(RelayCodecError::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    fn u8(&mut self) -> Result<u8, RelayCodecError> {
        Ok(self.take(1)?[0])
    }

    fn byte(&mut self) -> Result<u8, RelayCodecError> {
        self.u8()
    }

    fn fixed16(&mut self) -> Result<[u8; 16], RelayCodecError> {
        self.take(16)?.try_into().map_err(|_| RelayCodecError::Truncated)
    }

    fn u16(&mut self) -> Result<u16, RelayCodecError> {
        Ok(u16::from_be_bytes(self.take(2)?.try_into().map_err(|_| RelayCodecError::Truncated)?))
    }

    fn u32(&mut self) -> Result<u32, RelayCodecError> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().map_err(|_| RelayCodecError::Truncated)?))
    }

    fn i64(&mut self) -> Result<i64, RelayCodecError> {
        Ok(i64::from_be_bytes(self.take(8)?.try_into().map_err(|_| RelayCodecError::Truncated)?))
    }

    fn id(&mut self) -> Result<OpaqueId, RelayCodecError> {
        Ok(OpaqueId::from_bytes(self.take(16)?.try_into().map_err(|_| RelayCodecError::Truncated)?))
    }

    fn timestamp(&mut self) -> Result<Timestamp, RelayCodecError> {
        Timestamp::from_unix_millis(self.i64()?).map_err(|_| RelayCodecError::InvalidField)
    }

    fn code(&mut self) -> Result<RelayCode, RelayCodecError> {
        let length = usize::from(self.u8()?);
        let value =
            core::str::from_utf8(self.take(length)?).map_err(|_| RelayCodecError::InvalidField)?;
        RelayCode::new(value).map_err(|_| RelayCodecError::InvalidField)
    }

    fn blob(&mut self) -> Result<Vec<u8>, RelayCodecError> {
        let length = usize::try_from(self.u32()?).map_err(|_| RelayCodecError::InvalidField)?;
        if length > MAX_RELAY_BLOB_LEN {
            return Err(RelayCodecError::PayloadTooLarge { actual: length });
        }
        Ok(self.take(length)?.to_vec())
    }

    fn protocol_error(&mut self) -> Result<RelayProtocolError, RelayCodecError> {
        let code = self.u8()?;
        let actual = usize::try_from(self.u32()?).map_err(|_| RelayCodecError::InvalidField)?;
        match code {
            1 => Ok(RelayProtocolError::InvalidCode),
            2 => Ok(RelayProtocolError::BlobTooLarge { actual }),
            3 => Ok(RelayProtocolError::SlotNotFound),
            4 => Ok(RelayProtocolError::SlotExpired),
            5 => Ok(RelayProtocolError::SlotAlreadyJoined),
            6 => Ok(RelayProtocolError::Unauthorized),
            7 => Ok(RelayProtocolError::QueueFull),
            8 => Ok(RelayProtocolError::InvalidOperation),
            _ => Err(RelayCodecError::InvalidField),
        }
    }

    fn finish(&self) -> Result<(), RelayCodecError> {
        if self.offset == self.bytes.len() { Ok(()) } else { Err(RelayCodecError::TrailingBytes) }
    }
}

#[cfg(test)]
mod tests {
    use torca_foundation::{OpaqueId, Timestamp};

    use super::{
        RelayCode, RelayCodec, RelayJoinTicket, RelayRequest, RelayResponse, RelaySideToken,
        RelaySlotCapability, RelaySlotId,
    };

    #[test]
    fn request_round_trips_exactly() {
        let request = RelayRequest::Open {
            code: RelayCode::new("T0RCA2").expect("code"),
            expires_at: Timestamp::from_unix_millis(123).expect("timestamp"),
            creator_blob: vec![1, 2, 3],
            slot_capability: RelaySlotCapability(OpaqueId::from_u128(7)),
            creator_token: RelaySideToken(OpaqueId::from_u128(8)),
            ticket: RelayJoinTicket([9; 16]),
        };
        let encoded = RelayCodec::encode_request(&request).expect("encode");
        assert_eq!(RelayCodec::decode_request(&encoded).expect("decode"), request);
    }

    #[test]
    fn health_check_round_trips_exactly() {
        let encoded = RelayCodec::encode_request(&RelayRequest::Health).expect("encode");
        assert_eq!(RelayCodec::decode_request(&encoded).expect("decode"), RelayRequest::Health);
        let encoded = RelayCodec::encode_response(&RelayResponse::Healthy).expect("encode");
        assert_eq!(RelayCodec::decode_response(&encoded).expect("decode"), RelayResponse::Healthy);
    }

    #[test]
    fn response_batch_round_trips_exactly() {
        let response = RelayResponse::Blobs(vec![vec![1], vec![2, 3]]);
        let encoded = RelayCodec::encode_response(&response).expect("encode");
        assert_eq!(RelayCodec::decode_response(&encoded).expect("decode"), response);
    }

    #[test]
    fn joined_response_preserves_creator_blob() {
        let response = RelayResponse::Joined {
            slot_id: RelaySlotId(OpaqueId::from_u128(11)),
            expires_at: Timestamp::from_unix_millis(1_700_000_000_000).expect("timestamp"),
            creator_blob: vec![9; 32],
        };
        let encoded = RelayCodec::encode_response(&response).expect("encode");
        assert_eq!(RelayCodec::decode_response(&encoded).expect("decode"), response);
    }
}
