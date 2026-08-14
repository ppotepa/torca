//! Versioned, bounded wire vocabulary for Radio Mode.

use core::fmt;
use torca_foundation::OpaqueId;

pub const RADIO_PROTOCOL_VERSION: u16 = 2;
pub const RADIO_FRAME_INTERVAL_MS: u32 = 20;
pub const RADIO_SAMPLE_RATE_HZ: u32 = 8_000;
pub const RADIO_SAMPLES_PER_FRAME: usize = 160;
pub const MAX_RADIO_AUDIO_PAYLOAD: usize = 256;
pub const MAX_RADIO_MEDIA_FRAME: usize = 512;
pub const MAX_RADIO_BURST_FRAMES: usize = 500;
pub const RADIO_MEDIA_PROOF_LEN: usize = 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SessionCloseReason {
    Disabled = 1,
    Replaced = 2,
    PeerUnavailable = 3,
    ProtocolError = 4,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum FloorDeniedReason {
    ChannelBusy = 1,
    SessionNotReady = 2,
    ConsentRequired = 3,
    Cancelled = 4,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum BurstEndReason {
    Released = 1,
    LimitReached = 2,
    Backgrounded = 3,
    SessionInterrupted = 4,
    AudioUnavailable = 5,
    NetworkTooSlow = 6,
}

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RadioControlFrame {
    StateSync { boot_epoch: [u8; 16], revision: u64, enabled: bool, changed_at_ms: i64 },
    SessionOpen { session_id: OpaqueId, media_token: [u8; 32], coordinator_identity: OpaqueId },
    SessionClose { session_id: OpaqueId, reason: SessionCloseReason },
}

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RadioMediaFrame {
    Hello {
        protocol_version: u16,
        session_id: OpaqueId,
        nonce: [u8; 24],
        proof: [u8; RADIO_MEDIA_PROOF_LEN],
    },
    FloorRequest {
        request_id: OpaqueId,
    },
    FloorGrant {
        request_id: OpaqueId,
        burst_id: OpaqueId,
        max_duration_ms: u32,
    },
    FloorDenied {
        request_id: OpaqueId,
        reason: FloorDeniedReason,
    },
    /// Announces that a granted floor has actually started producing media.
    /// This is deliberately separate from FloorGrant: a grant is a response
    /// to a request, while a burst start is an event for the receiver.
    BurstStart {
        request_id: OpaqueId,
        burst_id: OpaqueId,
        max_duration_ms: u32,
    },
    Audio {
        burst_id: OpaqueId,
        sequence: u32,
        ciphertext: Vec<u8>,
    },
    EndBurst {
        burst_id: OpaqueId,
        final_sequence_exclusive: u32,
        reason: BurstEndReason,
    },
    BurstAck {
        burst_id: OpaqueId,
        /// Exact audio sequence received and authenticated by the peer.
        /// ACKs are intentionally not cumulative: an out-of-order frame must
        /// never cause an earlier missing frame to be discarded by the sender.
        sequence: u32,
    },
    KeepAlive {
        sequence: u64,
    },
    Close {
        reason: SessionCloseReason,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RadioProtocolError {
    Truncated,
    Malformed,
    UnsupportedVersion(u16),
    PayloadTooLarge { actual: usize, maximum: usize },
    InvalidBurstLimit,
    InvalidSequence,
    TrailingBytes,
}

impl fmt::Display for RadioProtocolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for RadioProtocolError {}

pub struct RadioControlCodec;

impl RadioControlCodec {
    pub fn encode(frame: &RadioControlFrame) -> Vec<u8> {
        let mut output = Vec::with_capacity(80);
        output.extend_from_slice(&RADIO_PROTOCOL_VERSION.to_be_bytes());
        match frame {
            RadioControlFrame::StateSync { boot_epoch, revision, enabled, changed_at_ms } => {
                output.push(1);
                output.extend_from_slice(boot_epoch);
                output.extend_from_slice(&revision.to_be_bytes());
                output.push(u8::from(*enabled));
                output.extend_from_slice(&changed_at_ms.to_be_bytes());
            }
            RadioControlFrame::SessionOpen { session_id, media_token, coordinator_identity } => {
                output.push(2);
                output.extend_from_slice(session_id.as_bytes());
                output.extend_from_slice(media_token);
                output.extend_from_slice(coordinator_identity.as_bytes());
            }
            RadioControlFrame::SessionClose { session_id, reason } => {
                output.push(4);
                output.extend_from_slice(session_id.as_bytes());
                output.push(*reason as u8);
            }
        }
        output
    }

    pub fn decode(input: &[u8]) -> Result<RadioControlFrame, RadioProtocolError> {
        let mut cursor = Cursor::new(input);
        ensure_version(cursor.u16()?)?;
        let frame = match cursor.u8()? {
            1 => RadioControlFrame::StateSync {
                boot_epoch: cursor.array_16()?,
                revision: cursor.u64()?,
                enabled: decode_bool(cursor.u8()?)?,
                changed_at_ms: cursor.i64()?,
            },
            2 => RadioControlFrame::SessionOpen {
                session_id: cursor.id()?,
                media_token: cursor.array_32()?,
                coordinator_identity: cursor.id()?,
            },
            4 => RadioControlFrame::SessionClose {
                session_id: cursor.id()?,
                reason: session_close_reason(cursor.u8()?)?,
            },
            _ => return Err(RadioProtocolError::Malformed),
        };
        cursor.finish()?;
        Ok(frame)
    }
}

pub struct RadioMediaCodec;

impl RadioMediaCodec {
    pub fn encode(frame: &RadioMediaFrame) -> Result<Vec<u8>, RadioProtocolError> {
        let mut output = Vec::with_capacity(MAX_RADIO_MEDIA_FRAME);
        output.extend_from_slice(&RADIO_PROTOCOL_VERSION.to_be_bytes());
        match frame {
            RadioMediaFrame::Hello { protocol_version, session_id, nonce, proof } => {
                ensure_version(*protocol_version)?;
                output.push(1);
                output.extend_from_slice(&protocol_version.to_be_bytes());
                output.extend_from_slice(session_id.as_bytes());
                output.extend_from_slice(nonce);
                output.extend_from_slice(proof);
            }
            RadioMediaFrame::FloorRequest { request_id } => {
                output.push(2);
                output.extend_from_slice(request_id.as_bytes());
            }
            RadioMediaFrame::FloorGrant { request_id, burst_id, max_duration_ms } => {
                if *max_duration_ms == 0 || *max_duration_ms > 10_000 {
                    return Err(RadioProtocolError::InvalidBurstLimit);
                }
                output.push(3);
                output.extend_from_slice(request_id.as_bytes());
                output.extend_from_slice(burst_id.as_bytes());
                output.extend_from_slice(&max_duration_ms.to_be_bytes());
            }
            RadioMediaFrame::FloorDenied { request_id, reason } => {
                output.push(4);
                output.extend_from_slice(request_id.as_bytes());
                output.push(*reason as u8);
            }
            RadioMediaFrame::BurstStart { request_id, burst_id, max_duration_ms } => {
                if *max_duration_ms == 0 || *max_duration_ms > 10_000 {
                    return Err(RadioProtocolError::InvalidBurstLimit);
                }
                output.push(5);
                output.extend_from_slice(request_id.as_bytes());
                output.extend_from_slice(burst_id.as_bytes());
                output.extend_from_slice(&max_duration_ms.to_be_bytes());
            }
            RadioMediaFrame::Audio { burst_id, sequence, ciphertext } => {
                if ciphertext.len() > MAX_RADIO_AUDIO_PAYLOAD {
                    return Err(RadioProtocolError::PayloadTooLarge {
                        actual: ciphertext.len(),
                        maximum: MAX_RADIO_AUDIO_PAYLOAD,
                    });
                }
                output.push(6);
                output.extend_from_slice(burst_id.as_bytes());
                output.extend_from_slice(&sequence.to_be_bytes());
                let length = u16::try_from(ciphertext.len()).map_err(|_| {
                    RadioProtocolError::PayloadTooLarge {
                        actual: ciphertext.len(),
                        maximum: MAX_RADIO_AUDIO_PAYLOAD,
                    }
                })?;
                output.extend_from_slice(&length.to_be_bytes());
                output.extend_from_slice(ciphertext);
            }
            RadioMediaFrame::EndBurst { burst_id, final_sequence_exclusive, reason } => {
                if *final_sequence_exclusive
                    > u32::try_from(MAX_RADIO_BURST_FRAMES).unwrap_or(u32::MAX)
                {
                    return Err(RadioProtocolError::InvalidSequence);
                }
                output.push(7);
                output.extend_from_slice(burst_id.as_bytes());
                output.extend_from_slice(&final_sequence_exclusive.to_be_bytes());
                output.push(*reason as u8);
            }
            RadioMediaFrame::BurstAck { burst_id, sequence } => {
                output.push(8);
                output.extend_from_slice(burst_id.as_bytes());
                output.extend_from_slice(&sequence.to_be_bytes());
            }
            RadioMediaFrame::KeepAlive { sequence } => {
                output.push(9);
                output.extend_from_slice(&sequence.to_be_bytes());
            }
            RadioMediaFrame::Close { reason } => {
                output.push(10);
                output.push(*reason as u8);
            }
        }
        if output.len() > MAX_RADIO_MEDIA_FRAME {
            return Err(RadioProtocolError::PayloadTooLarge {
                actual: output.len(),
                maximum: MAX_RADIO_MEDIA_FRAME,
            });
        }
        Ok(output)
    }

    pub fn decode(input: &[u8]) -> Result<RadioMediaFrame, RadioProtocolError> {
        if input.len() > MAX_RADIO_MEDIA_FRAME {
            return Err(RadioProtocolError::PayloadTooLarge {
                actual: input.len(),
                maximum: MAX_RADIO_MEDIA_FRAME,
            });
        }
        let mut cursor = Cursor::new(input);
        ensure_version(cursor.u16()?)?;
        let frame = match cursor.u8()? {
            1 => {
                let protocol_version = cursor.u16()?;
                ensure_version(protocol_version)?;
                RadioMediaFrame::Hello {
                    protocol_version,
                    session_id: cursor.id()?,
                    nonce: cursor.array_24()?,
                    proof: cursor.array_16()?,
                }
            }
            2 => RadioMediaFrame::FloorRequest { request_id: cursor.id()? },
            3 => {
                let request_id = cursor.id()?;
                let burst_id = cursor.id()?;
                let max_duration_ms = cursor.u32()?;
                if max_duration_ms == 0 || max_duration_ms > 10_000 {
                    return Err(RadioProtocolError::InvalidBurstLimit);
                }
                RadioMediaFrame::FloorGrant { request_id, burst_id, max_duration_ms }
            }
            4 => RadioMediaFrame::FloorDenied {
                request_id: cursor.id()?,
                reason: floor_denied_reason(cursor.u8()?)?,
            },
            5 => {
                let request_id = cursor.id()?;
                let burst_id = cursor.id()?;
                let max_duration_ms = cursor.u32()?;
                if max_duration_ms == 0 || max_duration_ms > 10_000 {
                    return Err(RadioProtocolError::InvalidBurstLimit);
                }
                RadioMediaFrame::BurstStart { request_id, burst_id, max_duration_ms }
            }
            6 => {
                let burst_id = cursor.id()?;
                let sequence = cursor.u32()?;
                let length = usize::from(cursor.u16()?);
                if length > MAX_RADIO_AUDIO_PAYLOAD {
                    return Err(RadioProtocolError::PayloadTooLarge {
                        actual: length,
                        maximum: MAX_RADIO_AUDIO_PAYLOAD,
                    });
                }
                RadioMediaFrame::Audio {
                    burst_id,
                    sequence,
                    ciphertext: cursor.take(length)?.to_vec(),
                }
            }
            7 => {
                let burst_id = cursor.id()?;
                let final_sequence_exclusive = cursor.u32()?;
                if final_sequence_exclusive
                    > u32::try_from(MAX_RADIO_BURST_FRAMES).unwrap_or(u32::MAX)
                {
                    return Err(RadioProtocolError::InvalidSequence);
                }
                RadioMediaFrame::EndBurst {
                    burst_id,
                    final_sequence_exclusive,
                    reason: burst_end_reason(cursor.u8()?)?,
                }
            }
            8 => RadioMediaFrame::BurstAck { burst_id: cursor.id()?, sequence: cursor.u32()? },
            9 => RadioMediaFrame::KeepAlive { sequence: cursor.u64()? },
            10 => RadioMediaFrame::Close { reason: session_close_reason(cursor.u8()?)? },
            _ => return Err(RadioProtocolError::Malformed),
        };
        cursor.finish()?;
        Ok(frame)
    }

    /// Adds a bounded big-endian length prefix for one TCP media frame.
    pub fn encode_framed(frame: &RadioMediaFrame) -> Result<Vec<u8>, RadioProtocolError> {
        let payload = Self::encode(frame)?;
        let mut framed = Vec::with_capacity(4 + payload.len());
        framed.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        framed.extend_from_slice(&payload);
        Ok(framed)
    }
}

fn ensure_version(version: u16) -> Result<(), RadioProtocolError> {
    if version == RADIO_PROTOCOL_VERSION {
        Ok(())
    } else {
        Err(RadioProtocolError::UnsupportedVersion(version))
    }
}

fn decode_bool(value: u8) -> Result<bool, RadioProtocolError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(RadioProtocolError::Malformed),
    }
}

fn session_close_reason(value: u8) -> Result<SessionCloseReason, RadioProtocolError> {
    match value {
        1 => Ok(SessionCloseReason::Disabled),
        2 => Ok(SessionCloseReason::Replaced),
        3 => Ok(SessionCloseReason::PeerUnavailable),
        4 => Ok(SessionCloseReason::ProtocolError),
        _ => Err(RadioProtocolError::Malformed),
    }
}

fn floor_denied_reason(value: u8) -> Result<FloorDeniedReason, RadioProtocolError> {
    match value {
        1 => Ok(FloorDeniedReason::ChannelBusy),
        2 => Ok(FloorDeniedReason::SessionNotReady),
        3 => Ok(FloorDeniedReason::ConsentRequired),
        4 => Ok(FloorDeniedReason::Cancelled),
        _ => Err(RadioProtocolError::Malformed),
    }
}

fn burst_end_reason(value: u8) -> Result<BurstEndReason, RadioProtocolError> {
    match value {
        1 => Ok(BurstEndReason::Released),
        2 => Ok(BurstEndReason::LimitReached),
        3 => Ok(BurstEndReason::Backgrounded),
        4 => Ok(BurstEndReason::SessionInterrupted),
        5 => Ok(BurstEndReason::AudioUnavailable),
        6 => Ok(BurstEndReason::NetworkTooSlow),
        _ => Err(RadioProtocolError::Malformed),
    }
}

struct Cursor<'a> {
    input: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    const fn new(input: &'a [u8]) -> Self {
        Self { input, offset: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], RadioProtocolError> {
        let end = self.offset.checked_add(length).ok_or(RadioProtocolError::Malformed)?;
        let value = self.input.get(self.offset..end).ok_or(RadioProtocolError::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    fn finish(&self) -> Result<(), RadioProtocolError> {
        if self.offset == self.input.len() {
            Ok(())
        } else {
            Err(RadioProtocolError::TrailingBytes)
        }
    }

    fn u8(&mut self) -> Result<u8, RadioProtocolError> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, RadioProtocolError> {
        Ok(u16::from_be_bytes(self.take(2)?.try_into().map_err(|_| RadioProtocolError::Truncated)?))
    }

    fn u32(&mut self) -> Result<u32, RadioProtocolError> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().map_err(|_| RadioProtocolError::Truncated)?))
    }

    fn u64(&mut self) -> Result<u64, RadioProtocolError> {
        Ok(u64::from_be_bytes(self.take(8)?.try_into().map_err(|_| RadioProtocolError::Truncated)?))
    }

    fn i64(&mut self) -> Result<i64, RadioProtocolError> {
        Ok(i64::from_be_bytes(self.take(8)?.try_into().map_err(|_| RadioProtocolError::Truncated)?))
    }

    fn id(&mut self) -> Result<OpaqueId, RadioProtocolError> {
        Ok(OpaqueId::from_bytes(self.array_16()?))
    }

    fn array_16(&mut self) -> Result<[u8; 16], RadioProtocolError> {
        self.take(16)?.try_into().map_err(|_| RadioProtocolError::Truncated)
    }

    fn array_24(&mut self) -> Result<[u8; 24], RadioProtocolError> {
        self.take(24)?.try_into().map_err(|_| RadioProtocolError::Truncated)
    }

    fn array_32(&mut self) -> Result<[u8; 32], RadioProtocolError> {
        self.take(32)?.try_into().map_err(|_| RadioProtocolError::Truncated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_frames_round_trip() {
        let values = [
            RadioControlFrame::StateSync {
                boot_epoch: [1; 16],
                revision: 42,
                enabled: true,
                changed_at_ms: 99,
            },
            RadioControlFrame::SessionOpen {
                session_id: OpaqueId::from_u128(1),
                media_token: [2; 32],
                coordinator_identity: OpaqueId::from_u128(3),
            },
            RadioControlFrame::SessionClose {
                session_id: OpaqueId::from_u128(5),
                reason: SessionCloseReason::Disabled,
            },
        ];
        for value in values {
            let encoded = RadioControlCodec::encode(&value);
            assert_eq!(RadioControlCodec::decode(&encoded), Ok(value));
        }
    }

    #[test]
    fn media_frames_round_trip() {
        let values = [
            RadioMediaFrame::Hello {
                protocol_version: RADIO_PROTOCOL_VERSION,
                session_id: OpaqueId::from_u128(1),
                nonce: [1; 24],
                proof: [2; RADIO_MEDIA_PROOF_LEN],
            },
            RadioMediaFrame::FloorRequest { request_id: OpaqueId::from_u128(2) },
            RadioMediaFrame::FloorGrant {
                request_id: OpaqueId::from_u128(2),
                burst_id: OpaqueId::from_u128(3),
                max_duration_ms: 10_000,
            },
            RadioMediaFrame::FloorDenied {
                request_id: OpaqueId::from_u128(2),
                reason: FloorDeniedReason::ChannelBusy,
            },
            RadioMediaFrame::BurstStart {
                request_id: OpaqueId::from_u128(2),
                burst_id: OpaqueId::from_u128(3),
                max_duration_ms: 10_000,
            },
            RadioMediaFrame::FloorDenied {
                request_id: OpaqueId::from_u128(9),
                reason: FloorDeniedReason::Cancelled,
            },
            RadioMediaFrame::Audio {
                burst_id: OpaqueId::from_u128(3),
                sequence: 4,
                ciphertext: vec![7; 176],
            },
            RadioMediaFrame::EndBurst {
                burst_id: OpaqueId::from_u128(3),
                final_sequence_exclusive: 5,
                reason: BurstEndReason::Released,
            },
            RadioMediaFrame::BurstAck { burst_id: OpaqueId::from_u128(3), sequence: 4 },
            RadioMediaFrame::KeepAlive { sequence: 8 },
            RadioMediaFrame::Close { reason: SessionCloseReason::Replaced },
        ];
        for value in values {
            let encoded = RadioMediaCodec::encode(&value).expect("encode");
            assert_eq!(RadioMediaCodec::decode(&encoded), Ok(value));
        }
    }

    #[test]
    fn decoder_rejects_oversized_audio_without_allocating_it() {
        let frame = RadioMediaFrame::Audio {
            burst_id: OpaqueId::from_u128(1),
            sequence: 1,
            ciphertext: vec![0; MAX_RADIO_AUDIO_PAYLOAD + 1],
        };
        assert_eq!(
            RadioMediaCodec::encode(&frame),
            Err(RadioProtocolError::PayloadTooLarge {
                actual: MAX_RADIO_AUDIO_PAYLOAD + 1,
                maximum: MAX_RADIO_AUDIO_PAYLOAD,
            })
        );
    }

    #[test]
    fn end_burst_sequence_is_bounded() {
        let frame = RadioMediaFrame::EndBurst {
            burst_id: OpaqueId::from_u128(1),
            final_sequence_exclusive: u32::try_from(MAX_RADIO_BURST_FRAMES)
                .expect("constant fits")
                .saturating_add(1),
            reason: BurstEndReason::Released,
        };
        assert_eq!(RadioMediaCodec::encode(&frame), Err(RadioProtocolError::InvalidSequence));
    }

    #[test]
    fn decoder_rejects_unknown_versions_and_trailing_bytes() {
        let mut encoded = RadioControlCodec::encode(&RadioControlFrame::SessionClose {
            session_id: OpaqueId::from_u128(1),
            reason: SessionCloseReason::Replaced,
        });
        encoded[1] = 3;
        assert_eq!(
            RadioControlCodec::decode(&encoded),
            Err(RadioProtocolError::UnsupportedVersion(3))
        );

        let mut encoded =
            RadioMediaCodec::encode(&RadioMediaFrame::KeepAlive { sequence: 1 }).expect("encode");
        encoded.push(0);
        assert_eq!(RadioMediaCodec::decode(&encoded), Err(RadioProtocolError::TrailingBytes));
    }
}
