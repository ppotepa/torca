//! Versioned opaque rendezvous-relay protocol.

use core::fmt;

use torca_foundation::{OpaqueId, Timestamp};

/// Maximum opaque blob size relayed for one operation.
pub const MAX_RELAY_BLOB_LEN: usize = 64 * 1024;

/// Protocol version.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RelayProtocolVersion(pub u16);
impl RelayProtocolVersion { /// Initial protocol version.
    pub const V1: Self = Self(1); }

/// Relay slot ID.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RelaySlotId(pub OpaqueId);

/// Validated rendezvous code independent from the pairing domain model.
#[must_use]
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RelayCode(String);
impl RelayCode {
    /// Creates an uppercase alphanumeric code.
    pub fn new(value: impl Into<String>) -> Result<Self, RelayProtocolError> {
        let value = value.into().to_ascii_uppercase();
        if !(6..=16).contains(&value.len()) || !value.bytes().all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit()) { return Err(RelayProtocolError::InvalidCode); }
        Ok(Self(value))
    }
    /// Returns text.
    pub fn as_str(&self) -> &str { &self.0 }
}

/// Opaque relay request.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RelayRequest {
    /// Opens a short-lived slot.
    Open { code: RelayCode, expires_at: Timestamp, creator_blob: Vec<u8> },
    /// Joins a slot.
    Join { code: RelayCode, joiner_blob: Vec<u8> },
    /// Pushes an opaque blob from one side.
    Push { slot_id: RelaySlotId, side: RelaySide, blob: Vec<u8> },
    /// Polls queued blobs for one side.
    Poll { slot_id: RelaySlotId, side: RelaySide },
    /// Closes a slot.
    Close { slot_id: RelaySlotId },
}

/// Side of the rendezvous slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelaySide { Creator, Joiner }

/// Opaque relay response.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RelayResponse { Opened { slot_id: RelaySlotId }, Joined { slot_id: RelaySlotId, creator_blob: Vec<u8> }, Accepted, Blobs(Vec<Vec<u8>>), Closed }

/// Relay protocol error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RelayProtocolError { InvalidCode, BlobTooLarge { actual: usize }, SlotNotFound, SlotExpired, SlotAlreadyJoined, QueueFull, InvalidOperation }
impl fmt::Display for RelayProtocolError { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{self:?}") } }
impl std::error::Error for RelayProtocolError {}

/// Validates an opaque blob before relay storage or forwarding.
pub fn validate_blob(blob: &[u8]) -> Result<(), RelayProtocolError> {
    if blob.len() > MAX_RELAY_BLOB_LEN { Err(RelayProtocolError::BlobTooLarge { actual: blob.len() }) } else { Ok(()) }
}
