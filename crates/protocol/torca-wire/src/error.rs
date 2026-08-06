use core::fmt;

use crate::{ProtocolFamily, ProtocolVersion, VersionSupport};

/// Failure while encoding a generic wire frame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EncodeError {
    /// The frame version is outside the configured compatibility range.
    UnsupportedProtocolVersion {
        /// Version requested by the caller.
        received: ProtocolVersion,
        /// Compatibility range configured on the codec.
        supported: VersionSupport,
    },
    /// The payload exceeds the configured maximum.
    PayloadTooLarge {
        /// Actual payload length.
        actual: usize,
        /// Configured maximum payload length.
        maximum: usize,
    },
    /// The complete frame length overflowed `usize`.
    FrameLengthOverflow,
}

impl fmt::Display for EncodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedProtocolVersion { received, supported } => write!(
                formatter,
                "protocol version {received} is unsupported; expected major {} and minor at most {}",
                supported.major(),
                supported.max_minor()
            ),
            Self::PayloadTooLarge { actual, maximum } => {
                write!(formatter, "payload length {actual} exceeds limit {maximum}")
            }
            Self::FrameLengthOverflow => formatter.write_str("complete frame length overflowed"),
        }
    }
}

impl std::error::Error for EncodeError {}

/// Failure while decoding a generic wire frame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodeError {
    /// Header magic does not identify a Torca frame.
    InvalidMagic {
        /// Four bytes found at the beginning of the header.
        actual: [u8; 4],
    },
    /// Header encoding version is not understood.
    UnsupportedHeaderVersion {
        /// Header version found in the frame.
        actual: u8,
    },
    /// Header contains flag bits unknown to this header version.
    InvalidFlags {
        /// Raw flag bits found in the header.
        actual: u8,
    },
    /// Reserved header bytes are not zero.
    ReservedBitsSet {
        /// Raw reserved value.
        actual: u16,
    },
    /// Protocol family zero is invalid.
    InvalidProtocolFamily,
    /// Message kind zero is invalid.
    InvalidMessageKind,
    /// Protocol version uses reserved major version zero.
    InvalidProtocolVersion {
        /// Raw major version.
        major: u16,
        /// Raw minor version.
        minor: u16,
    },
    /// Frame belongs to another protocol family.
    UnexpectedProtocolFamily {
        /// Protocol family configured on the codec.
        expected: ProtocolFamily,
        /// Protocol family found in the frame.
        actual: ProtocolFamily,
    },
    /// Frame protocol version is outside the configured compatibility range.
    UnsupportedProtocolVersion {
        /// Version found in the frame.
        received: ProtocolVersion,
        /// Compatibility range configured on the codec.
        supported: VersionSupport,
    },
    /// Payload length exceeds the configured maximum.
    PayloadTooLarge {
        /// Payload length declared by the header.
        actual: usize,
        /// Configured maximum payload length.
        maximum: usize,
    },
    /// Input ended before the complete frame was available.
    UnexpectedEnd {
        /// Complete frame length required by the header.
        required: usize,
        /// Number of bytes supplied.
        actual: usize,
    },
    /// Exact decoding received bytes after the complete frame.
    TrailingBytes {
        /// Number of bytes after the complete frame.
        count: usize,
    },
    /// A platform conversion could not represent a validated frame length.
    LengthConversion,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidMagic { actual } => write!(formatter, "invalid frame magic {actual:?}"),
            Self::UnsupportedHeaderVersion { actual } => {
                write!(formatter, "unsupported wire header version {actual}")
            }
            Self::InvalidFlags { actual } => {
                write!(formatter, "invalid wire flag bits 0x{actual:02x}")
            }
            Self::ReservedBitsSet { actual } => {
                write!(formatter, "reserved header bits are set: 0x{actual:04x}")
            }
            Self::InvalidProtocolFamily => formatter.write_str("protocol family zero is invalid"),
            Self::InvalidMessageKind => formatter.write_str("message kind zero is invalid"),
            Self::InvalidProtocolVersion { major, minor } => {
                write!(formatter, "invalid protocol version {major}.{minor}")
            }
            Self::UnexpectedProtocolFamily { expected, actual } => {
                write!(formatter, "unexpected protocol family {actual}; expected {expected}")
            }
            Self::UnsupportedProtocolVersion { received, supported } => write!(
                formatter,
                "protocol version {received} is unsupported; expected major {} and minor at most {}",
                supported.major(),
                supported.max_minor()
            ),
            Self::PayloadTooLarge { actual, maximum } => {
                write!(formatter, "declared payload length {actual} exceeds limit {maximum}")
            }
            Self::UnexpectedEnd { required, actual } => {
                write!(formatter, "incomplete frame: required {required} bytes, received {actual}")
            }
            Self::TrailingBytes { count } => {
                write!(formatter, "exact decoder received {count} trailing bytes")
            }
            Self::LengthConversion => formatter.write_str("frame length conversion failed"),
        }
    }
}

impl std::error::Error for DecodeError {}
