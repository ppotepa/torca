//! Versioned opaque rendezvous-relay protocol.

use core::fmt;

use torca_foundation::{OpaqueId, Timestamp};

/// Maximum opaque blob size relayed for one operation.
pub const MAX_RELAY_BLOB_LEN: usize = 64 * 1024;

/// Protocol version.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RelayProtocolVersion(pub u16);
impl RelayProtocolVersion {
    /// Capability-authenticated rendezvous protocol.
    pub const V2: Self = Self(2);
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

/// Validated rendezvous code independent from the pairing domain model.
#[must_use]
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RelayCode(String);
impl RelayCode {
    /// Creates an uppercase alphanumeric code.
    pub fn new(value: impl Into<String>) -> Result<Self, RelayProtocolError> {
        let value = value.into().to_ascii_uppercase();
        if !(6..=16).contains(&value.len())
            || !value.bytes().all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
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
    /// Opens a short-lived slot. All capabilities are generated client-side with a CSPRNG.
    Open {
        code: RelayCode,
        expires_at: Timestamp,
        creator_blob: Vec<u8>,
        slot_capability: RelaySlotCapability,
        creator_token: RelaySideToken,
    },
    /// Joins a slot and installs the joiner's client-generated side token.
    Join {
        code: RelayCode,
        joiner_blob: Vec<u8>,
        joiner_token: RelaySideToken,
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
    Opened { slot_id: RelaySlotId },
    Joined { slot_id: RelaySlotId, creator_blob: Vec<u8> },
    Accepted,
    Blobs(Vec<Vec<u8>>),
    Closed,
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

/// Validates an opaque blob before relay storage or forwarding.
pub fn validate_blob(blob: &[u8]) -> Result<(), RelayProtocolError> {
    if blob.len() > MAX_RELAY_BLOB_LEN {
        Err(RelayProtocolError::BlobTooLarge { actual: blob.len() })
    } else {
        Ok(())
    }
}
