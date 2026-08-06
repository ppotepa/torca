//! Generic, versioned and strictly bounded binary framing for Torca protocols.
//!
//! `torca-wire` owns only common envelope mechanics. Message, pairing, relay and peer payload
//! schemas belong to their dedicated protocol crates.

mod codec;
mod error;
mod frame;
mod header;
mod limits;
mod types;

pub use codec::{DecodeOutcome, FrameDecoder, WireCodec};
pub use error::{DecodeError, EncodeError};
pub use frame::WireFrame;
pub use header::WireHeader;
pub use limits::{DEFAULT_MAX_PAYLOAD_LEN, HARD_MAX_PAYLOAD_LEN, WIRE_HEADER_LEN, WireLimits};
pub use types::{
    EnvelopeId, FrameMetadata, MessageKind, ProtocolFamily, ProtocolVersion, VersionSupport,
    WireFlags,
};

#[cfg(test)]
mod tests {
    use torca_foundation::{CorrelationId, OpaqueId};

    use crate::{
        DecodeError, EncodeError, EnvelopeId, FrameDecoder, FrameMetadata, MessageKind,
        ProtocolFamily, ProtocolVersion, VersionSupport, WIRE_HEADER_LEN, WireCodec, WireFlags,
        WireLimits,
    };

    fn codec() -> WireCodec {
        WireCodec::new(
            ProtocolFamily::new(1).expect("test family is valid"),
            VersionSupport::new(1, 2).expect("test support is valid"),
            WireLimits::new(64).expect("test limit is valid"),
        )
    }

    fn metadata(version: ProtocolVersion, sequence: u128) -> FrameMetadata {
        FrameMetadata::new(
            version,
            MessageKind::new(7).expect("test kind is valid"),
            WireFlags::REQUIRED_KIND,
            EnvelopeId::from_u128(sequence),
            CorrelationId::from_opaque(OpaqueId::from_u128(99)),
        )
    }

    #[test]
    fn frame_encoding_is_deterministic_and_round_trips() {
        let codec = codec();
        let metadata = metadata(ProtocolVersion::new(1, 2).expect("test version is valid"), 5);
        let encoded = codec.encode(metadata, b"abc").expect("frame must encode");

        assert_eq!(encoded.len(), WIRE_HEADER_LEN + 3);
        assert_eq!(&encoded[..4], b"TRCA");
        assert_eq!(encoded[4], 1);
        assert_eq!(encoded[5], WireFlags::REQUIRED_KIND.bits());
        assert_eq!(&encoded[16..20], &3_u32.to_be_bytes());

        let decoded = codec.decode_exact(&encoded).expect("frame must decode");
        assert_eq!(decoded.metadata(), metadata);
        assert_eq!(decoded.payload(), b"abc");
    }

    #[test]
    fn decoder_accepts_supported_older_minor_versions() {
        let codec = codec();
        let metadata = metadata(ProtocolVersion::new(1, 1).expect("test version is valid"), 1);
        let encoded = codec.encode(metadata, b"ok").expect("frame must encode");

        assert_eq!(codec.decode_exact(&encoded).expect("frame must decode").payload(), b"ok");
    }

    #[test]
    fn encoder_rejects_newer_minor_versions() {
        let codec = codec();
        let received = ProtocolVersion::new(1, 3).expect("test version is valid");
        let error = codec
            .encode(metadata(received, 1), b"no")
            .expect_err("newer minor version must be rejected");

        assert_eq!(
            error,
            EncodeError::UnsupportedProtocolVersion {
                received,
                supported: codec.supported_versions(),
            }
        );
    }

    #[test]
    fn oversized_declared_payload_is_rejected_before_payload_arrives() {
        let codec = codec();
        let metadata = metadata(ProtocolVersion::new(1, 0).expect("test version is valid"), 1);
        let mut encoded = codec.encode(metadata, b"small").expect("frame must encode");
        encoded[16..20].copy_from_slice(&65_u32.to_be_bytes());
        encoded.truncate(WIRE_HEADER_LEN);

        assert_eq!(
            codec.decode(&encoded).expect_err("oversized length must fail"),
            DecodeError::PayloadTooLarge { actual: 65, maximum: 64 }
        );
    }

    #[test]
    fn incremental_decoder_handles_partial_and_concatenated_frames() {
        let codec = codec();
        let first = codec
            .encode(metadata(ProtocolVersion::new(1, 0).expect("test version is valid"), 1), b"one")
            .expect("first frame must encode");
        let second = codec
            .encode(metadata(ProtocolVersion::new(1, 0).expect("test version is valid"), 2), b"two")
            .expect("second frame must encode");
        let mut bytes = first;
        bytes.extend_from_slice(&second);
        let mut decoder = FrameDecoder::new(codec);
        let mut frames = Vec::new();

        for byte in bytes {
            frames.extend(decoder.push(&[byte]).expect("stream must decode"));
        }

        assert!(decoder.is_idle());
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].payload(), b"one");
        assert_eq!(frames[1].payload(), b"two");
    }

    #[test]
    fn malformed_magic_resets_incremental_decoder() {
        let mut decoder = FrameDecoder::new(codec());
        let mut invalid = vec![0_u8; WIRE_HEADER_LEN];
        invalid[..4].copy_from_slice(b"NOPE");

        assert!(matches!(decoder.push(&invalid), Err(DecodeError::InvalidMagic { .. })));
        assert!(decoder.is_idle());
    }
}
