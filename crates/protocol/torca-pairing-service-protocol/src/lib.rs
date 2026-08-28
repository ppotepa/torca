//! Versioned opaque rendezvous-relay protocol and strict binary wire codec.

use core::fmt;

use torca_foundation::{OpaqueId, Timestamp};

/// Maximum opaque blob size relayed for one operation.
pub const MAX_PAIRING_SERVICE_BLOB_LEN: usize = 64 * 1024;
/// Maximum number of blobs returned by one poll response.
pub const MAX_PAIRING_SERVICE_BATCH_BLOBS: usize = 32;
/// Fixed relay frame header length.
pub const PAIRING_SERVICE_HEADER_LEN: usize = 12;
/// Maximum encoded frame length, including a full bounded poll batch.
pub const MAX_PAIRING_SERVICE_FRAME_LEN: usize = PAIRING_SERVICE_HEADER_LEN
    + 4
    + MAX_PAIRING_SERVICE_BATCH_BLOBS * (8 + 16 + 4 + MAX_PAIRING_SERVICE_BLOB_LEN);

const PAIRING_SERVICE_MAGIC: &[u8; 4] = b"TCRL";
const REQUEST_DIRECTION: u8 = 1;
const RESPONSE_DIRECTION: u8 = 2;
const CROCKFORD_BASE32: &str = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const MAX_PAIRING_SERVICE_INFO_TEXT_LEN: usize = 128;

/// Protocol version.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PairingServiceProtocolVersion(pub u16);
impl PairingServiceProtocolVersion {
    /// Idempotent, acknowledged rendezvous protocol.
    pub const V4: Self = Self(4);
}

/// Public, non-sensitive identity of the running relay artifact.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairingServiceInfo {
    pub product_version: String,
    pub build_id: String,
    pub source_commit: String,
    pub protocol_version: u16,
}

impl PairingServiceInfo {
    pub fn new(
        product_version: impl Into<String>,
        build_id: impl Into<String>,
        source_commit: impl Into<String>,
    ) -> Result<Self, PairingServiceCodecError> {
        let value = Self {
            product_version: product_version.into(),
            build_id: build_id.into(),
            source_commit: source_commit.into(),
            protocol_version: PairingServiceProtocolVersion::V4.0,
        };
        for text in [&value.product_version, &value.build_id, &value.source_commit] {
            if text.is_empty() || text.len() > MAX_PAIRING_SERVICE_INFO_TEXT_LEN {
                return Err(PairingServiceCodecError::InvalidField);
            }
        }
        Ok(value)
    }
}

/// Stable idempotency key for one relay mutation.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PairingServiceOperationId(pub OpaqueId);

/// Stable identifier for one opaque queued message.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PairingServiceMessageId(pub OpaqueId);

/// Monotonic sequence within one authenticated side queue.
#[must_use]
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct PairingServiceSequence(pub u64);

/// Non-destructively polled relay delivery.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairingServiceDelivery {
    pub sequence: PairingServiceSequence,
    pub message_id: PairingServiceMessageId,
    pub blob: Vec<u8>,
}

/// Relay slot ID. It is an address, not an authorization secret.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PairingServiceSlotId(pub OpaqueId);

/// Opaque capability that authorizes destructive slot administration.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PairingServiceSlotCapability(pub OpaqueId);

/// Opaque per-side capability used for push/poll operations.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PairingServiceSideToken(pub OpaqueId);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PairingServiceJoinTicket(pub [u8; 16]);

/// Validated rendezvous code independent from the pairing domain model.
#[must_use]
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PairingServiceCode(String);
impl PairingServiceCode {
    /// Creates the exact six-character Crockford Base32 invitation code.
    pub fn new(value: impl Into<String>) -> Result<Self, PairingServiceProtocolError> {
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
            return Err(PairingServiceProtocolError::InvalidCode);
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
pub enum PairingServiceRequest {
    /// Unauthenticated protocol-level health check.
    Health,
    /// Returns the version and build identity of the responding relay.
    Info,
    /// Opens a short-lived slot. All capabilities are generated client-side with a CSPRNG.
    Open {
        operation_id: PairingServiceOperationId,
        code: PairingServiceCode,
        expires_at: Timestamp,
        creator_blob: Vec<u8>,
        slot_capability: PairingServiceSlotCapability,
        creator_token: PairingServiceSideToken,
        ticket: PairingServiceJoinTicket,
    },
    /// Joins a slot and installs the joiner's client-generated side token.
    Join {
        operation_id: PairingServiceOperationId,
        code: PairingServiceCode,
        joiner_blob: Vec<u8>,
        joiner_token: PairingServiceSideToken,
        ticket: Option<PairingServiceJoinTicket>,
    },
    /// Pushes an opaque blob to the opposite side. Side is inferred from the capability.
    Push {
        operation_id: PairingServiceOperationId,
        message_id: PairingServiceMessageId,
        slot_id: PairingServiceSlotId,
        token: PairingServiceSideToken,
        blob: Vec<u8>,
    },
    /// Non-destructively polls messages newer than the acknowledged cursor.
    Poll {
        slot_id: PairingServiceSlotId,
        token: PairingServiceSideToken,
        after: PairingServiceSequence,
    },
    /// Acknowledges durable processing and removes messages through the cursor.
    Ack {
        slot_id: PairingServiceSlotId,
        token: PairingServiceSideToken,
        up_to: PairingServiceSequence,
    },
    /// Closes a slot using its separate administrative capability.
    Close { slot_id: PairingServiceSlotId, capability: PairingServiceSlotCapability },
}

/// Side of the rendezvous slot after capability authentication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PairingServiceSide {
    Creator,
    Joiner,
}

/// Opaque relay response.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PairingServiceResponse {
    Healthy,
    Info(PairingServiceInfo),
    Opened {
        slot_id: PairingServiceSlotId,
        /// Relay-clock deadline. Clients project this value instead of trusting
        /// their own wall clock for invitation expiry.
        expires_at: Timestamp,
    },
    Joined {
        slot_id: PairingServiceSlotId,
        /// Relay-clock deadline shared with the joining client.
        expires_at: Timestamp,
        creator_blob: Vec<u8>,
    },
    Accepted,
    Deliveries(Vec<PairingServiceDelivery>),
    Acked(PairingServiceSequence),
    Closed,
    /// Application-level relay rejection. Transport succeeded and callers must not retry blindly.
    Error(PairingServiceProtocolError),
}

/// Relay protocol error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PairingServiceProtocolError {
    InvalidCode,
    BlobTooLarge { actual: usize },
    SlotNotFound,
    SlotExpired,
    SlotAlreadyJoined,
    Unauthorized,
    QueueFull,
    InvalidOperation,
}
impl fmt::Display for PairingServiceProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for PairingServiceProtocolError {}

/// Strict frame codec failure. These errors describe malformed transport bytes rather than a
/// semantically rejected relay operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PairingServiceCodecError {
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
impl fmt::Display for PairingServiceCodecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for PairingServiceCodecError {}

/// Deterministic binary codec used on every relay network transport.
pub struct PairingServiceCodec;
impl PairingServiceCodec {
    /// Encodes one exact request frame.
    pub fn encode_request(
        request: &PairingServiceRequest,
    ) -> Result<Vec<u8>, PairingServiceCodecError> {
        let mut payload = Vec::new();
        let kind = match request {
            PairingServiceRequest::Health => 0,
            PairingServiceRequest::Info => 7,
            PairingServiceRequest::Open {
                operation_id,
                code,
                expires_at,
                creator_blob,
                slot_capability,
                creator_token,
                ticket,
            } => {
                payload.extend_from_slice(operation_id.0.as_bytes());
                put_code(code, &mut payload)?;
                payload.extend_from_slice(&expires_at.to_unix_millis().to_be_bytes());
                put_blob(creator_blob, &mut payload)?;
                payload.extend_from_slice(slot_capability.0.as_bytes());
                payload.extend_from_slice(creator_token.0.as_bytes());
                payload.extend_from_slice(&ticket.0);
                1
            }
            PairingServiceRequest::Join {
                operation_id,
                code,
                joiner_blob,
                joiner_token,
                ticket,
            } => {
                payload.extend_from_slice(operation_id.0.as_bytes());
                put_code(code, &mut payload)?;
                put_blob(joiner_blob, &mut payload)?;
                payload.extend_from_slice(joiner_token.0.as_bytes());
                payload.push(u8::from(ticket.is_some()));
                if let Some(ticket) = ticket {
                    payload.extend_from_slice(&ticket.0);
                }
                2
            }
            PairingServiceRequest::Push { operation_id, message_id, slot_id, token, blob } => {
                payload.extend_from_slice(operation_id.0.as_bytes());
                payload.extend_from_slice(message_id.0.as_bytes());
                payload.extend_from_slice(slot_id.0.as_bytes());
                payload.extend_from_slice(token.0.as_bytes());
                put_blob(blob, &mut payload)?;
                3
            }
            PairingServiceRequest::Poll { slot_id, token, after } => {
                payload.extend_from_slice(slot_id.0.as_bytes());
                payload.extend_from_slice(token.0.as_bytes());
                payload.extend_from_slice(&after.0.to_be_bytes());
                4
            }
            PairingServiceRequest::Close { slot_id, capability } => {
                payload.extend_from_slice(slot_id.0.as_bytes());
                payload.extend_from_slice(capability.0.as_bytes());
                5
            }
            PairingServiceRequest::Ack { slot_id, token, up_to } => {
                payload.extend_from_slice(slot_id.0.as_bytes());
                payload.extend_from_slice(token.0.as_bytes());
                payload.extend_from_slice(&up_to.0.to_be_bytes());
                6
            }
        };
        encode_frame(REQUEST_DIRECTION, kind, &payload)
    }

    /// Decodes one exact request frame and rejects trailing bytes.
    pub fn decode_request(bytes: &[u8]) -> Result<PairingServiceRequest, PairingServiceCodecError> {
        let (direction, kind, payload) = decode_frame(bytes)?;
        if direction != REQUEST_DIRECTION {
            return Err(PairingServiceCodecError::InvalidDirection);
        }
        let mut cursor = Cursor::new(payload);
        let request = match kind {
            0 => PairingServiceRequest::Health,
            1 => PairingServiceRequest::Open {
                operation_id: PairingServiceOperationId(cursor.id()?),
                code: cursor.code()?,
                expires_at: cursor.timestamp()?,
                creator_blob: cursor.blob()?,
                slot_capability: PairingServiceSlotCapability(cursor.id()?),
                creator_token: PairingServiceSideToken(cursor.id()?),
                ticket: PairingServiceJoinTicket(cursor.fixed16()?),
            },
            2 => PairingServiceRequest::Join {
                operation_id: PairingServiceOperationId(cursor.id()?),
                code: cursor.code()?,
                joiner_blob: cursor.blob()?,
                joiner_token: PairingServiceSideToken(cursor.id()?),
                ticket: match cursor.byte()? {
                    0 => None,
                    1 => Some(PairingServiceJoinTicket(cursor.fixed16()?)),
                    _ => return Err(PairingServiceCodecError::InvalidField),
                },
            },
            3 => PairingServiceRequest::Push {
                operation_id: PairingServiceOperationId(cursor.id()?),
                message_id: PairingServiceMessageId(cursor.id()?),
                slot_id: PairingServiceSlotId(cursor.id()?),
                token: PairingServiceSideToken(cursor.id()?),
                blob: cursor.blob()?,
            },
            4 => PairingServiceRequest::Poll {
                slot_id: PairingServiceSlotId(cursor.id()?),
                token: PairingServiceSideToken(cursor.id()?),
                after: PairingServiceSequence(cursor.u64()?),
            },
            5 => PairingServiceRequest::Close {
                slot_id: PairingServiceSlotId(cursor.id()?),
                capability: PairingServiceSlotCapability(cursor.id()?),
            },
            6 => PairingServiceRequest::Ack {
                slot_id: PairingServiceSlotId(cursor.id()?),
                token: PairingServiceSideToken(cursor.id()?),
                up_to: PairingServiceSequence(cursor.u64()?),
            },
            7 => PairingServiceRequest::Info,
            value => return Err(PairingServiceCodecError::UnknownKind(value)),
        };
        cursor.finish()?;
        Ok(request)
    }

    /// Encodes one exact response frame.
    pub fn encode_response(
        response: &PairingServiceResponse,
    ) -> Result<Vec<u8>, PairingServiceCodecError> {
        let mut payload = Vec::new();
        let kind = match response {
            PairingServiceResponse::Healthy => 7,
            PairingServiceResponse::Info(info) => {
                put_text(&info.product_version, &mut payload)?;
                put_text(&info.build_id, &mut payload)?;
                put_text(&info.source_commit, &mut payload)?;
                payload.extend_from_slice(&info.protocol_version.to_be_bytes());
                9
            }
            PairingServiceResponse::Opened { slot_id, expires_at } => {
                payload.extend_from_slice(slot_id.0.as_bytes());
                payload.extend_from_slice(&expires_at.to_unix_millis().to_be_bytes());
                1
            }
            PairingServiceResponse::Joined { slot_id, expires_at, creator_blob } => {
                payload.extend_from_slice(slot_id.0.as_bytes());
                payload.extend_from_slice(&expires_at.to_unix_millis().to_be_bytes());
                put_blob(creator_blob, &mut payload)?;
                2
            }
            PairingServiceResponse::Accepted => 3,
            PairingServiceResponse::Deliveries(deliveries) => {
                if deliveries.len() > MAX_PAIRING_SERVICE_BATCH_BLOBS {
                    return Err(PairingServiceCodecError::TooManyBlobs {
                        actual: deliveries.len(),
                    });
                }
                let count = u16::try_from(deliveries.len())
                    .map_err(|_| PairingServiceCodecError::InvalidField)?;
                payload.extend_from_slice(&count.to_be_bytes());
                for delivery in deliveries {
                    payload.extend_from_slice(&delivery.sequence.0.to_be_bytes());
                    payload.extend_from_slice(delivery.message_id.0.as_bytes());
                    put_blob(&delivery.blob, &mut payload)?;
                }
                4
            }
            PairingServiceResponse::Acked(sequence) => {
                payload.extend_from_slice(&sequence.0.to_be_bytes());
                8
            }
            PairingServiceResponse::Closed => 5,
            PairingServiceResponse::Error(error) => {
                encode_protocol_error(error, &mut payload)?;
                6
            }
        };
        encode_frame(RESPONSE_DIRECTION, kind, &payload)
    }

    /// Decodes one exact response frame and rejects trailing bytes.
    pub fn decode_response(
        bytes: &[u8],
    ) -> Result<PairingServiceResponse, PairingServiceCodecError> {
        let (direction, kind, payload) = decode_frame(bytes)?;
        if direction != RESPONSE_DIRECTION {
            return Err(PairingServiceCodecError::InvalidDirection);
        }
        let mut cursor = Cursor::new(payload);
        let response = match kind {
            7 => PairingServiceResponse::Healthy,
            1 => PairingServiceResponse::Opened {
                slot_id: PairingServiceSlotId(cursor.id()?),
                expires_at: cursor.timestamp()?,
            },
            2 => PairingServiceResponse::Joined {
                slot_id: PairingServiceSlotId(cursor.id()?),
                expires_at: cursor.timestamp()?,
                creator_blob: cursor.blob()?,
            },
            3 => PairingServiceResponse::Accepted,
            4 => {
                let count = usize::from(cursor.u16()?);
                if count > MAX_PAIRING_SERVICE_BATCH_BLOBS {
                    return Err(PairingServiceCodecError::TooManyBlobs { actual: count });
                }
                let mut deliveries = Vec::with_capacity(count);
                for _ in 0..count {
                    deliveries.push(PairingServiceDelivery {
                        sequence: PairingServiceSequence(cursor.u64()?),
                        message_id: PairingServiceMessageId(cursor.id()?),
                        blob: cursor.blob()?,
                    });
                }
                PairingServiceResponse::Deliveries(deliveries)
            }
            5 => PairingServiceResponse::Closed,
            6 => PairingServiceResponse::Error(cursor.protocol_error()?),
            8 => PairingServiceResponse::Acked(PairingServiceSequence(cursor.u64()?)),
            9 => PairingServiceResponse::Info(PairingServiceInfo {
                product_version: cursor.text()?,
                build_id: cursor.text()?,
                source_commit: cursor.text()?,
                protocol_version: cursor.u16()?,
            }),
            value => return Err(PairingServiceCodecError::UnknownKind(value)),
        };
        cursor.finish()?;
        Ok(response)
    }

    /// Reads and validates the complete frame length from a fixed relay header.
    pub fn frame_len_from_header(
        header: &[u8; PAIRING_SERVICE_HEADER_LEN],
    ) -> Result<usize, PairingServiceCodecError> {
        validate_header_prefix(header)?;
        let payload_len = u32::from_be_bytes(
            header[8..12].try_into().map_err(|_| PairingServiceCodecError::Truncated)?,
        );
        let payload_len =
            usize::try_from(payload_len).map_err(|_| PairingServiceCodecError::InvalidField)?;
        let frame_len = PAIRING_SERVICE_HEADER_LEN
            .checked_add(payload_len)
            .ok_or(PairingServiceCodecError::InvalidField)?;
        if frame_len > MAX_PAIRING_SERVICE_FRAME_LEN {
            return Err(PairingServiceCodecError::PayloadTooLarge { actual: payload_len });
        }
        Ok(frame_len)
    }
}

/// Validates an opaque blob before relay storage or forwarding.
pub fn validate_blob(blob: &[u8]) -> Result<(), PairingServiceProtocolError> {
    if blob.len() > MAX_PAIRING_SERVICE_BLOB_LEN {
        Err(PairingServiceProtocolError::BlobTooLarge { actual: blob.len() })
    } else {
        Ok(())
    }
}

fn encode_frame(
    direction: u8,
    kind: u8,
    payload: &[u8],
) -> Result<Vec<u8>, PairingServiceCodecError> {
    let payload_len = u32::try_from(payload.len())
        .map_err(|_| PairingServiceCodecError::PayloadTooLarge { actual: payload.len() })?;
    let frame_len = PAIRING_SERVICE_HEADER_LEN
        .checked_add(payload.len())
        .ok_or(PairingServiceCodecError::InvalidField)?;
    if frame_len > MAX_PAIRING_SERVICE_FRAME_LEN {
        return Err(PairingServiceCodecError::PayloadTooLarge { actual: payload.len() });
    }
    let mut output = Vec::with_capacity(frame_len);
    output.extend_from_slice(PAIRING_SERVICE_MAGIC);
    output.extend_from_slice(&PairingServiceProtocolVersion::V4.0.to_be_bytes());
    output.push(direction);
    output.push(kind);
    output.extend_from_slice(&payload_len.to_be_bytes());
    output.extend_from_slice(payload);
    Ok(output)
}

fn decode_frame(bytes: &[u8]) -> Result<(u8, u8, &[u8]), PairingServiceCodecError> {
    if bytes.len() < PAIRING_SERVICE_HEADER_LEN {
        return Err(PairingServiceCodecError::Truncated);
    }
    let header: &[u8; PAIRING_SERVICE_HEADER_LEN] = bytes[..PAIRING_SERVICE_HEADER_LEN]
        .try_into()
        .map_err(|_| PairingServiceCodecError::Truncated)?;
    let frame_len = PairingServiceCodec::frame_len_from_header(header)?;
    if bytes.len() < frame_len {
        return Err(PairingServiceCodecError::Truncated);
    }
    if bytes.len() != frame_len {
        return Err(PairingServiceCodecError::TrailingBytes);
    }
    Ok((header[6], header[7], &bytes[PAIRING_SERVICE_HEADER_LEN..]))
}

fn validate_header_prefix(
    header: &[u8; PAIRING_SERVICE_HEADER_LEN],
) -> Result<(), PairingServiceCodecError> {
    if &header[..4] != PAIRING_SERVICE_MAGIC {
        return Err(PairingServiceCodecError::InvalidMagic);
    }
    let version = u16::from_be_bytes(
        header[4..6].try_into().map_err(|_| PairingServiceCodecError::Truncated)?,
    );
    if version != PairingServiceProtocolVersion::V4.0 {
        return Err(PairingServiceCodecError::UnsupportedVersion(version));
    }
    if !matches!(header[6], REQUEST_DIRECTION | RESPONSE_DIRECTION) {
        return Err(PairingServiceCodecError::InvalidDirection);
    }
    Ok(())
}

fn put_code(
    code: &PairingServiceCode,
    output: &mut Vec<u8>,
) -> Result<(), PairingServiceCodecError> {
    let bytes = code.as_str().as_bytes();
    let length = u8::try_from(bytes.len()).map_err(|_| PairingServiceCodecError::InvalidField)?;
    output.push(length);
    output.extend_from_slice(bytes);
    Ok(())
}

fn put_blob(blob: &[u8], output: &mut Vec<u8>) -> Result<(), PairingServiceCodecError> {
    if blob.len() > MAX_PAIRING_SERVICE_BLOB_LEN {
        return Err(PairingServiceCodecError::PayloadTooLarge { actual: blob.len() });
    }
    let length = u32::try_from(blob.len())
        .map_err(|_| PairingServiceCodecError::PayloadTooLarge { actual: blob.len() })?;
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(blob);
    Ok(())
}

fn put_text(value: &str, output: &mut Vec<u8>) -> Result<(), PairingServiceCodecError> {
    if value.is_empty() || value.len() > MAX_PAIRING_SERVICE_INFO_TEXT_LEN {
        return Err(PairingServiceCodecError::InvalidField);
    }
    let length = u8::try_from(value.len()).map_err(|_| PairingServiceCodecError::InvalidField)?;
    output.push(length);
    output.extend_from_slice(value.as_bytes());
    Ok(())
}

fn encode_protocol_error(
    error: &PairingServiceProtocolError,
    output: &mut Vec<u8>,
) -> Result<(), PairingServiceCodecError> {
    let (code, actual) = match error {
        PairingServiceProtocolError::InvalidCode => (1, 0),
        PairingServiceProtocolError::BlobTooLarge { actual } => {
            let actual =
                u32::try_from(*actual).map_err(|_| PairingServiceCodecError::InvalidField)?;
            (2, actual)
        }
        PairingServiceProtocolError::SlotNotFound => (3, 0),
        PairingServiceProtocolError::SlotExpired => (4, 0),
        PairingServiceProtocolError::SlotAlreadyJoined => (5, 0),
        PairingServiceProtocolError::Unauthorized => (6, 0),
        PairingServiceProtocolError::QueueFull => (7, 0),
        PairingServiceProtocolError::InvalidOperation => (8, 0),
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

    fn take(&mut self, length: usize) -> Result<&'a [u8], PairingServiceCodecError> {
        let end = self.offset.checked_add(length).ok_or(PairingServiceCodecError::InvalidField)?;
        let value = self.bytes.get(self.offset..end).ok_or(PairingServiceCodecError::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    fn u8(&mut self) -> Result<u8, PairingServiceCodecError> {
        Ok(self.take(1)?[0])
    }

    fn byte(&mut self) -> Result<u8, PairingServiceCodecError> {
        self.u8()
    }

    fn fixed16(&mut self) -> Result<[u8; 16], PairingServiceCodecError> {
        self.take(16)?.try_into().map_err(|_| PairingServiceCodecError::Truncated)
    }

    fn u16(&mut self) -> Result<u16, PairingServiceCodecError> {
        Ok(u16::from_be_bytes(
            self.take(2)?.try_into().map_err(|_| PairingServiceCodecError::Truncated)?,
        ))
    }

    fn u32(&mut self) -> Result<u32, PairingServiceCodecError> {
        Ok(u32::from_be_bytes(
            self.take(4)?.try_into().map_err(|_| PairingServiceCodecError::Truncated)?,
        ))
    }

    fn u64(&mut self) -> Result<u64, PairingServiceCodecError> {
        Ok(u64::from_be_bytes(
            self.take(8)?.try_into().map_err(|_| PairingServiceCodecError::Truncated)?,
        ))
    }

    fn i64(&mut self) -> Result<i64, PairingServiceCodecError> {
        Ok(i64::from_be_bytes(
            self.take(8)?.try_into().map_err(|_| PairingServiceCodecError::Truncated)?,
        ))
    }

    fn id(&mut self) -> Result<OpaqueId, PairingServiceCodecError> {
        Ok(OpaqueId::from_bytes(
            self.take(16)?.try_into().map_err(|_| PairingServiceCodecError::Truncated)?,
        ))
    }

    fn timestamp(&mut self) -> Result<Timestamp, PairingServiceCodecError> {
        Timestamp::from_unix_millis(self.i64()?).map_err(|_| PairingServiceCodecError::InvalidField)
    }

    fn code(&mut self) -> Result<PairingServiceCode, PairingServiceCodecError> {
        let length = usize::from(self.u8()?);
        let value = core::str::from_utf8(self.take(length)?)
            .map_err(|_| PairingServiceCodecError::InvalidField)?;
        PairingServiceCode::new(value).map_err(|_| PairingServiceCodecError::InvalidField)
    }

    fn blob(&mut self) -> Result<Vec<u8>, PairingServiceCodecError> {
        let length =
            usize::try_from(self.u32()?).map_err(|_| PairingServiceCodecError::InvalidField)?;
        if length > MAX_PAIRING_SERVICE_BLOB_LEN {
            return Err(PairingServiceCodecError::PayloadTooLarge { actual: length });
        }
        Ok(self.take(length)?.to_vec())
    }

    fn text(&mut self) -> Result<String, PairingServiceCodecError> {
        let length = usize::from(self.u8()?);
        if length == 0 || length > MAX_PAIRING_SERVICE_INFO_TEXT_LEN {
            return Err(PairingServiceCodecError::InvalidField);
        }
        let value = core::str::from_utf8(self.take(length)?)
            .map_err(|_| PairingServiceCodecError::InvalidField)?;
        Ok(value.to_owned())
    }

    fn protocol_error(&mut self) -> Result<PairingServiceProtocolError, PairingServiceCodecError> {
        let code = self.u8()?;
        let actual =
            usize::try_from(self.u32()?).map_err(|_| PairingServiceCodecError::InvalidField)?;
        match code {
            1 => Ok(PairingServiceProtocolError::InvalidCode),
            2 => Ok(PairingServiceProtocolError::BlobTooLarge { actual }),
            3 => Ok(PairingServiceProtocolError::SlotNotFound),
            4 => Ok(PairingServiceProtocolError::SlotExpired),
            5 => Ok(PairingServiceProtocolError::SlotAlreadyJoined),
            6 => Ok(PairingServiceProtocolError::Unauthorized),
            7 => Ok(PairingServiceProtocolError::QueueFull),
            8 => Ok(PairingServiceProtocolError::InvalidOperation),
            _ => Err(PairingServiceCodecError::InvalidField),
        }
    }

    fn finish(&self) -> Result<(), PairingServiceCodecError> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(PairingServiceCodecError::TrailingBytes)
        }
    }
}

#[cfg(test)]
mod tests {
    use torca_foundation::{OpaqueId, Timestamp};

    use super::{
        PairingServiceCode, PairingServiceCodec, PairingServiceDelivery, PairingServiceInfo,
        PairingServiceJoinTicket, PairingServiceMessageId, PairingServiceOperationId,
        PairingServiceRequest, PairingServiceResponse, PairingServiceSequence,
        PairingServiceSideToken, PairingServiceSlotCapability, PairingServiceSlotId,
    };

    #[test]
    fn request_round_trips_exactly() {
        let request = PairingServiceRequest::Open {
            operation_id: PairingServiceOperationId(OpaqueId::from_u128(6)),
            code: PairingServiceCode::new("T0RCA2").expect("code"),
            expires_at: Timestamp::from_unix_millis(123).expect("timestamp"),
            creator_blob: vec![1, 2, 3],
            slot_capability: PairingServiceSlotCapability(OpaqueId::from_u128(7)),
            creator_token: PairingServiceSideToken(OpaqueId::from_u128(8)),
            ticket: PairingServiceJoinTicket([9; 16]),
        };
        let encoded = PairingServiceCodec::encode_request(&request).expect("encode");
        assert_eq!(PairingServiceCodec::decode_request(&encoded).expect("decode"), request);
    }

    #[test]
    fn health_check_round_trips_exactly() {
        let encoded =
            PairingServiceCodec::encode_request(&PairingServiceRequest::Health).expect("encode");
        assert_eq!(
            PairingServiceCodec::decode_request(&encoded).expect("decode"),
            PairingServiceRequest::Health
        );
        let encoded =
            PairingServiceCodec::encode_response(&PairingServiceResponse::Healthy).expect("encode");
        assert_eq!(
            PairingServiceCodec::decode_response(&encoded).expect("decode"),
            PairingServiceResponse::Healthy
        );
    }

    #[test]
    fn relay_info_round_trips_exactly() {
        let request = PairingServiceCodec::encode_request(&PairingServiceRequest::Info)
            .expect("encode request");
        assert_eq!(
            PairingServiceCodec::decode_request(&request).expect("decode request"),
            PairingServiceRequest::Info
        );
        let response = PairingServiceResponse::Info(
            PairingServiceInfo::new("0.3.0", "ABC123", "deadbeef").expect("valid info"),
        );
        let encoded = PairingServiceCodec::encode_response(&response).expect("encode response");
        assert_eq!(
            PairingServiceCodec::decode_response(&encoded).expect("decode response"),
            response
        );
    }

    #[test]
    fn response_batch_round_trips_exactly() {
        let response = PairingServiceResponse::Deliveries(vec![
            PairingServiceDelivery {
                sequence: PairingServiceSequence(1),
                message_id: PairingServiceMessageId(OpaqueId::from_u128(20)),
                blob: vec![1],
            },
            PairingServiceDelivery {
                sequence: PairingServiceSequence(2),
                message_id: PairingServiceMessageId(OpaqueId::from_u128(21)),
                blob: vec![2, 3],
            },
        ]);
        let encoded = PairingServiceCodec::encode_response(&response).expect("encode");
        assert_eq!(PairingServiceCodec::decode_response(&encoded).expect("decode"), response);
    }

    #[test]
    fn joined_response_preserves_creator_blob() {
        let response = PairingServiceResponse::Joined {
            slot_id: PairingServiceSlotId(OpaqueId::from_u128(11)),
            expires_at: Timestamp::from_unix_millis(1_700_000_000_000).expect("timestamp"),
            creator_blob: vec![9; 32],
        };
        let encoded = PairingServiceCodec::encode_response(&response).expect("encode");
        assert_eq!(PairingServiceCodec::decode_response(&encoded).expect("decode"), response);
    }
}
