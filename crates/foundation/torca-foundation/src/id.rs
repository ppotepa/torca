use core::{fmt, str::FromStr};

/// A dependency-free 128-bit identifier represented canonically as 32 hexadecimal characters.
#[must_use]
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct OpaqueId([u8; 16]);

impl OpaqueId {
    /// Number of bytes stored by an identifier.
    pub const BYTE_LEN: usize = 16;

    /// Number of hexadecimal characters in the canonical representation.
    pub const ENCODED_LEN: usize = Self::BYTE_LEN * 2;

    /// The all-zero identifier.
    pub const NIL: Self = Self([0; Self::BYTE_LEN]);

    /// Creates an identifier from raw bytes.
    pub const fn from_bytes(bytes: [u8; Self::BYTE_LEN]) -> Self {
        Self(bytes)
    }

    /// Creates an identifier from an unsigned 128-bit integer using big-endian byte order.
    pub const fn from_u128(value: u128) -> Self {
        Self(value.to_be_bytes())
    }

    /// Returns the raw bytes.
    pub const fn as_bytes(&self) -> &[u8; Self::BYTE_LEN] {
        &self.0
    }

    /// Consumes the identifier and returns its raw bytes.
    pub const fn into_bytes(self) -> [u8; Self::BYTE_LEN] {
        self.0
    }

    /// Returns the identifier as an unsigned 128-bit integer using big-endian byte order.
    pub const fn to_u128(self) -> u128 {
        u128::from_be_bytes(self.0)
    }

    /// Returns whether the identifier contains only zero bytes.
    pub const fn is_nil(&self) -> bool {
        self.to_u128() == 0
    }
}

impl fmt::Display for OpaqueId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::LowerHex::fmt(self, formatter)
    }
}

impl fmt::Debug for OpaqueId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "OpaqueId({self})")
    }
}

impl fmt::LowerHex for OpaqueId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl FromStr for OpaqueId {
    type Err = ParseOpaqueIdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let encoded = value.as_bytes();
        if encoded.len() != Self::ENCODED_LEN {
            return Err(ParseOpaqueIdError::InvalidLength {
                actual: encoded.len(),
            });
        }

        let mut bytes = [0_u8; Self::BYTE_LEN];
        for (byte_index, pair) in encoded.chunks_exact(2).enumerate() {
            let high_index = byte_index * 2;
            let low_index = high_index + 1;
            let high = decode_nibble(pair[0]).ok_or(ParseOpaqueIdError::InvalidCharacter {
                index: high_index,
                value: pair[0],
            })?;
            let low = decode_nibble(pair[1]).ok_or(ParseOpaqueIdError::InvalidCharacter {
                index: low_index,
                value: pair[1],
            })?;
            bytes[byte_index] = (high << 4) | low;
        }

        Ok(Self(bytes))
    }
}

const fn decode_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

/// Error returned when a textual identifier is not valid canonical hexadecimal data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParseOpaqueIdError {
    /// The input did not contain exactly 32 hexadecimal characters.
    InvalidLength {
        /// Actual number of bytes in the input.
        actual: usize,
    },
    /// The input contained a non-hexadecimal byte.
    InvalidCharacter {
        /// Zero-based byte position in the input.
        index: usize,
        /// Invalid byte value.
        value: u8,
    },
}

impl fmt::Display for ParseOpaqueIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLength { actual } => write!(
                formatter,
                "identifier must contain {} hexadecimal characters, got {actual}",
                OpaqueId::ENCODED_LEN
            ),
            Self::InvalidCharacter { index, value } => write!(
                formatter,
                "identifier contains invalid byte 0x{value:02x} at position {index}"
            ),
        }
    }
}

impl std::error::Error for ParseOpaqueIdError {}

macro_rules! identifier_type {
    ($name:ident, $description:literal) => {
        #[doc = $description]
        #[must_use]
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(OpaqueId);

        impl $name {
            /// Creates the typed identifier from a shared opaque identifier.
            pub const fn from_opaque(value: OpaqueId) -> Self {
                Self(value)
            }

            /// Creates the typed identifier from raw bytes.
            pub const fn from_bytes(bytes: [u8; OpaqueId::BYTE_LEN]) -> Self {
                Self(OpaqueId::from_bytes(bytes))
            }

            /// Creates the typed identifier from an unsigned 128-bit integer.
            pub const fn from_u128(value: u128) -> Self {
                Self(OpaqueId::from_u128(value))
            }

            /// Returns the shared opaque representation.
            pub const fn to_opaque(self) -> OpaqueId {
                self.0
            }

            /// Returns whether the identifier contains only zero bytes.
            pub const fn is_nil(&self) -> bool {
                self.0.is_nil()
            }
        }

        impl From<OpaqueId> for $name {
            fn from(value: OpaqueId) -> Self {
                Self(value)
            }
        }

        impl From<$name> for OpaqueId {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, formatter)
            }
        }

        impl FromStr for $name {
            type Err = ParseOpaqueIdError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                value.parse::<OpaqueId>().map(Self)
            }
        }
    };
}

identifier_type!(CommandId, "Stable idempotency identifier of an application command.");
identifier_type!(CorrelationId, "Identifier shared by operations belonging to one logical workflow.");
identifier_type!(CausationId, "Identifier of the command or event that directly caused another operation.");
identifier_type!(EventId, "Unique identifier of an immutable domain event occurrence.");

impl From<CommandId> for CorrelationId {
    fn from(value: CommandId) -> Self {
        Self::from_opaque(value.to_opaque())
    }
}

impl From<CommandId> for CausationId {
    fn from(value: CommandId) -> Self {
        Self::from_opaque(value.to_opaque())
    }
}

impl From<EventId> for CausationId {
    fn from(value: EventId) -> Self {
        Self::from_opaque(value.to_opaque())
    }
}

#[cfg(test)]
mod tests {
    use super::{CommandId, OpaqueId, ParseOpaqueIdError};

    #[test]
    fn opaque_identifier_round_trips_through_hexadecimal_text() {
        let identifier = OpaqueId::from_u128(0x0123_4567_89ab_cdef_fedc_ba98_7654_3210);
        let encoded = identifier.to_string();

        assert_eq!(encoded, "0123456789abcdeffedcba9876543210");
        assert_eq!(encoded.parse::<OpaqueId>(), Ok(identifier));
        assert_eq!(encoded.parse::<CommandId>(), Ok(CommandId::from(identifier)));
    }

    #[test]
    fn parser_reports_the_invalid_character_position() {
        let error = "0123456789abcdeffedcba987654321z"
            .parse::<OpaqueId>()
            .expect_err("invalid hexadecimal input must fail");

        assert_eq!(
            error,
            ParseOpaqueIdError::InvalidCharacter {
                index: 31,
                value: b'z'
            }
        );
    }
}
