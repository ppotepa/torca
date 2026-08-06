use torca_foundation::{CorrelationId, OpaqueId};

use crate::{
    DecodeError, EnvelopeId, FrameMetadata, MessageKind, ProtocolFamily, ProtocolVersion,
    VersionSupport, WireFlags, WireLimits, WIRE_HEADER_LEN,
};

pub(crate) const MAGIC: [u8; 4] = *b"TRCA";
pub(crate) const HEADER_VERSION: u8 = 1;

/// Decoded fixed-size generic wire header.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WireHeader {
    family: ProtocolFamily,
    metadata: FrameMetadata,
    payload_len: usize,
}

impl WireHeader {
    pub(crate) const fn new(
        family: ProtocolFamily,
        metadata: FrameMetadata,
        payload_len: usize,
    ) -> Self {
        Self {
            family,
            metadata,
            payload_len,
        }
    }

    /// Returns the protocol family.
    pub const fn family(self) -> ProtocolFamily {
        self.family
    }

    /// Returns generic frame metadata.
    pub const fn metadata(self) -> FrameMetadata {
        self.metadata
    }

    /// Returns the declared payload length.
    pub const fn payload_len(self) -> usize {
        self.payload_len
    }

    /// Returns the complete encoded frame length.
    pub const fn frame_len(self) -> usize {
        WIRE_HEADER_LEN + self.payload_len
    }

    pub(crate) fn encode(self) -> Result<[u8; WIRE_HEADER_LEN], crate::EncodeError> {
        let payload_len = u32::try_from(self.payload_len)
            .map_err(|_| crate::EncodeError::FrameLengthOverflow)?;
        let mut bytes = [0_u8; WIRE_HEADER_LEN];
        bytes[0..4].copy_from_slice(&MAGIC);
        bytes[4] = HEADER_VERSION;
        bytes[5] = self.metadata.flags().bits();
        bytes[6..8].copy_from_slice(&self.family.get().to_be_bytes());
        bytes[8..10].copy_from_slice(&self.metadata.version().major().to_be_bytes());
        bytes[10..12].copy_from_slice(&self.metadata.version().minor().to_be_bytes());
        bytes[12..14].copy_from_slice(&self.metadata.message_kind().get().to_be_bytes());
        bytes[14..16].copy_from_slice(&0_u16.to_be_bytes());
        bytes[16..20].copy_from_slice(&payload_len.to_be_bytes());
        bytes[20..36].copy_from_slice(self.metadata.envelope_id().to_opaque().as_bytes());
        bytes[36..52].copy_from_slice(self.metadata.correlation_id().to_opaque().as_bytes());
        Ok(bytes)
    }

    pub(crate) fn decode(
        input: &[u8; WIRE_HEADER_LEN],
        expected_family: ProtocolFamily,
        supported: VersionSupport,
        limits: WireLimits,
    ) -> Result<Self, DecodeError> {
        let actual_magic = [input[0], input[1], input[2], input[3]];
        if actual_magic != MAGIC {
            return Err(DecodeError::InvalidMagic {
                actual: actual_magic,
            });
        }
        if input[4] != HEADER_VERSION {
            return Err(DecodeError::UnsupportedHeaderVersion { actual: input[4] });
        }
        let flags = WireFlags::from_bits(input[5])
            .ok_or(DecodeError::InvalidFlags { actual: input[5] })?;
        let family_raw = u16::from_be_bytes([input[6], input[7]]);
        let family = ProtocolFamily::new(family_raw).ok_or(DecodeError::InvalidProtocolFamily)?;
        if family != expected_family {
            return Err(DecodeError::UnexpectedProtocolFamily {
                expected: expected_family,
                actual: family,
            });
        }
        let major = u16::from_be_bytes([input[8], input[9]]);
        let minor = u16::from_be_bytes([input[10], input[11]]);
        let version = ProtocolVersion::new(major, minor)
            .ok_or(DecodeError::InvalidProtocolVersion { major, minor })?;
        if !supported.supports(version) {
            return Err(DecodeError::UnsupportedProtocolVersion {
                received: version,
                supported,
            });
        }
        let message_kind_raw = u16::from_be_bytes([input[12], input[13]]);
        let message_kind =
            MessageKind::new(message_kind_raw).ok_or(DecodeError::InvalidMessageKind)?;
        let reserved = u16::from_be_bytes([input[14], input[15]]);
        if reserved != 0 {
            return Err(DecodeError::ReservedBitsSet { actual: reserved });
        }
        let payload_len_u32 = u32::from_be_bytes([input[16], input[17], input[18], input[19]]);
        let payload_len =
            usize::try_from(payload_len_u32).map_err(|_| DecodeError::LengthConversion)?;
        if payload_len > limits.max_payload_len() {
            return Err(DecodeError::PayloadTooLarge {
                actual: payload_len,
                maximum: limits.max_payload_len(),
            });
        }

        let mut envelope_bytes = [0_u8; OpaqueId::BYTE_LEN];
        envelope_bytes.copy_from_slice(&input[20..36]);
        let mut correlation_bytes = [0_u8; OpaqueId::BYTE_LEN];
        correlation_bytes.copy_from_slice(&input[36..52]);
        let metadata = FrameMetadata::new(
            version,
            message_kind,
            flags,
            EnvelopeId::from_bytes(envelope_bytes),
            CorrelationId::from_opaque(OpaqueId::from_bytes(correlation_bytes)),
        );

        Ok(Self::new(family, metadata, payload_len))
    }
}
