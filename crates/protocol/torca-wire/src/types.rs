use core::{fmt, num::NonZeroU16, str::FromStr};

use torca_foundation::{CorrelationId, OpaqueId, ParseOpaqueIdError};

/// Non-zero identifier of a protocol family.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProtocolFamily(NonZeroU16);

impl ProtocolFamily {
    /// Creates a protocol family identifier.
    pub const fn new(value: u16) -> Option<Self> {
        match NonZeroU16::new(value) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    /// Returns the numeric protocol family identifier.
    pub const fn get(self) -> u16 {
        self.0.get()
    }
}

impl fmt::Display for ProtocolFamily {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, formatter)
    }
}

/// A version within a protocol family.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProtocolVersion {
    major: u16,
    minor: u16,
}

impl ProtocolVersion {
    /// Creates a version. Major version zero is reserved and rejected.
    pub const fn new(major: u16, minor: u16) -> Option<Self> {
        if major == 0 { None } else { Some(Self { major, minor }) }
    }

    /// Returns the major version.
    pub const fn major(self) -> u16 {
        self.major
    }

    /// Returns the minor version.
    pub const fn minor(self) -> u16 {
        self.minor
    }
}

impl fmt::Display for ProtocolVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}", self.major, self.minor)
    }
}

/// Supported version range for one protocol family.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VersionSupport {
    major: u16,
    max_minor: u16,
}

impl VersionSupport {
    /// Creates support for one major version and all minor versions up to `max_minor`.
    pub const fn new(major: u16, max_minor: u16) -> Option<Self> {
        if major == 0 { None } else { Some(Self { major, max_minor }) }
    }

    /// Returns whether the supplied version is compatible.
    pub const fn supports(self, version: ProtocolVersion) -> bool {
        version.major == self.major && version.minor <= self.max_minor
    }

    /// Returns the supported major version.
    pub const fn major(self) -> u16 {
        self.major
    }

    /// Returns the highest supported minor version.
    pub const fn max_minor(self) -> u16 {
        self.max_minor
    }
}

/// Non-zero message-kind identifier scoped to a protocol family.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MessageKind(NonZeroU16);

impl MessageKind {
    /// Creates a message-kind identifier.
    pub const fn new(value: u16) -> Option<Self> {
        match NonZeroU16::new(value) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    /// Returns the numeric message-kind identifier.
    pub const fn get(self) -> u16 {
        self.0.get()
    }
}

impl fmt::Display for MessageKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, formatter)
    }
}

/// Flags carried by a generic wire header.
#[must_use]
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct WireFlags(u8);

impl WireFlags {
    /// No flags are set.
    pub const NONE: Self = Self(0);

    /// The receiver must reject the frame when it does not understand the message kind.
    pub const REQUIRED_KIND: Self = Self(0b0000_0001);

    const KNOWN_MASK: u8 = Self::REQUIRED_KIND.0;

    /// Creates flags when all bits are known to this header version.
    pub const fn from_bits(bits: u8) -> Option<Self> {
        if bits & !Self::KNOWN_MASK == 0 { Some(Self(bits)) } else { None }
    }

    /// Returns the raw flag bits.
    pub const fn bits(self) -> u8 {
        self.0
    }

    /// Returns whether every flag in `other` is set.
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

/// Stable identifier of a wire envelope.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EnvelopeId(OpaqueId);

impl EnvelopeId {
    /// Creates an envelope identifier from a shared opaque value.
    pub const fn from_opaque(value: OpaqueId) -> Self {
        Self(value)
    }

    /// Creates an envelope identifier from raw bytes.
    pub const fn from_bytes(bytes: [u8; OpaqueId::BYTE_LEN]) -> Self {
        Self(OpaqueId::from_bytes(bytes))
    }

    /// Creates an envelope identifier from an unsigned 128-bit integer.
    pub const fn from_u128(value: u128) -> Self {
        Self(OpaqueId::from_u128(value))
    }

    /// Returns the shared opaque value.
    pub const fn to_opaque(self) -> OpaqueId {
        self.0
    }
}

impl fmt::Display for EnvelopeId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, formatter)
    }
}

impl FromStr for EnvelopeId {
    type Err = ParseOpaqueIdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        value.parse::<OpaqueId>().map(Self)
    }
}

/// Metadata needed to encode one generic frame.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameMetadata {
    version: ProtocolVersion,
    message_kind: MessageKind,
    flags: WireFlags,
    envelope_id: EnvelopeId,
    correlation_id: CorrelationId,
}

impl FrameMetadata {
    /// Creates frame metadata.
    pub const fn new(
        version: ProtocolVersion,
        message_kind: MessageKind,
        flags: WireFlags,
        envelope_id: EnvelopeId,
        correlation_id: CorrelationId,
    ) -> Self {
        Self { version, message_kind, flags, envelope_id, correlation_id }
    }

    /// Returns the protocol version.
    pub const fn version(self) -> ProtocolVersion {
        self.version
    }

    /// Returns the message kind.
    pub const fn message_kind(self) -> MessageKind {
        self.message_kind
    }

    /// Returns the header flags.
    pub const fn flags(self) -> WireFlags {
        self.flags
    }

    /// Returns the envelope identifier.
    pub const fn envelope_id(self) -> EnvelopeId {
        self.envelope_id
    }

    /// Returns the workflow correlation identifier.
    pub const fn correlation_id(self) -> CorrelationId {
        self.correlation_id
    }
}
