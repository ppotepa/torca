use crate::{
    DecodeError, EncodeError, FrameMetadata, ProtocolFamily, VersionSupport, WireFrame, WireHeader,
    WireLimits, WIRE_HEADER_LEN,
};

/// Result of attempting to decode one frame from a byte slice.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodeOutcome {
    /// More bytes are required before a complete frame can be decoded.
    Incomplete {
        /// Minimum additional byte count required for the current frame.
        minimum_additional: usize,
    },
    /// One complete frame was decoded.
    Complete {
        /// Decoded frame.
        frame: WireFrame,
        /// Number of bytes consumed from the input.
        consumed: usize,
    },
}

/// Configured generic frame encoder and decoder for one protocol family.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WireCodec {
    family: ProtocolFamily,
    supported: VersionSupport,
    limits: WireLimits,
}

impl WireCodec {
    /// Creates a codec for one protocol family and version range.
    pub const fn new(
        family: ProtocolFamily,
        supported: VersionSupport,
        limits: WireLimits,
    ) -> Self {
        Self {
            family,
            supported,
            limits,
        }
    }

    /// Returns the configured protocol family.
    pub const fn family(self) -> ProtocolFamily {
        self.family
    }

    /// Returns the configured compatibility range.
    pub const fn supported_versions(self) -> VersionSupport {
        self.supported
    }

    /// Returns strict frame limits.
    pub const fn limits(self) -> WireLimits {
        self.limits
    }

    /// Encodes one complete generic frame.
    ///
    /// # Errors
    ///
    /// Returns [`EncodeError`] when the version is unsupported, the payload exceeds configured
    /// limits or the frame length cannot be represented safely.
    pub fn encode(
        self,
        metadata: FrameMetadata,
        payload: &[u8],
    ) -> Result<Vec<u8>, EncodeError> {
        if !self.supported.supports(metadata.version()) {
            return Err(EncodeError::UnsupportedProtocolVersion {
                received: metadata.version(),
                supported: self.supported,
            });
        }
        if payload.len() > self.limits.max_payload_len() {
            return Err(EncodeError::PayloadTooLarge {
                actual: payload.len(),
                maximum: self.limits.max_payload_len(),
            });
        }
        let frame_len = WIRE_HEADER_LEN
            .checked_add(payload.len())
            .ok_or(EncodeError::FrameLengthOverflow)?;
        let header = WireHeader::new(self.family, metadata, payload.len()).encode()?;
        let mut encoded = Vec::with_capacity(frame_len);
        encoded.extend_from_slice(&header);
        encoded.extend_from_slice(payload);
        Ok(encoded)
    }

    /// Attempts to decode one frame from the beginning of `input`.
    ///
    /// The function does not treat incomplete input as an error and never allocates until the
    /// fixed header and declared payload length have passed validation.
    ///
    /// # Errors
    ///
    /// Returns [`DecodeError`] for malformed headers, incompatible versions, unexpected protocol
    /// families or declared lengths above the configured limit.
    pub fn decode(self, input: &[u8]) -> Result<DecodeOutcome, DecodeError> {
        if input.len() < WIRE_HEADER_LEN {
            return Ok(DecodeOutcome::Incomplete {
                minimum_additional: WIRE_HEADER_LEN - input.len(),
            });
        }
        let header_bytes: &[u8; WIRE_HEADER_LEN] = input[..WIRE_HEADER_LEN]
            .try_into()
            .map_err(|_| DecodeError::LengthConversion)?;
        let header = WireHeader::decode(
            header_bytes,
            self.family,
            self.supported,
            self.limits,
        )?;
        let frame_len = header.frame_len();
        if input.len() < frame_len {
            return Ok(DecodeOutcome::Incomplete {
                minimum_additional: frame_len - input.len(),
            });
        }
        let payload = input[WIRE_HEADER_LEN..frame_len].to_vec();
        Ok(DecodeOutcome::Complete {
            frame: WireFrame::new(header, payload),
            consumed: frame_len,
        })
    }

    /// Decodes exactly one complete frame and rejects trailing bytes.
    ///
    /// # Errors
    ///
    /// Returns [`DecodeError`] when input is malformed, incomplete or contains bytes after the
    /// decoded frame.
    pub fn decode_exact(self, input: &[u8]) -> Result<WireFrame, DecodeError> {
        match self.decode(input)? {
            DecodeOutcome::Incomplete { .. } => {
                let required = required_frame_len(self, input)?;
                Err(DecodeError::UnexpectedEnd {
                    required,
                    actual: input.len(),
                })
            }
            DecodeOutcome::Complete { frame, consumed } if consumed == input.len() => Ok(frame),
            DecodeOutcome::Complete { consumed, .. } => Err(DecodeError::TrailingBytes {
                count: input.len() - consumed,
            }),
        }
    }
}

fn required_frame_len(codec: WireCodec, input: &[u8]) -> Result<usize, DecodeError> {
    if input.len() < WIRE_HEADER_LEN {
        return Ok(WIRE_HEADER_LEN);
    }
    let header_bytes: &[u8; WIRE_HEADER_LEN] = input[..WIRE_HEADER_LEN]
        .try_into()
        .map_err(|_| DecodeError::LengthConversion)?;
    WireHeader::decode(
        header_bytes,
        codec.family,
        codec.supported,
        codec.limits,
    )
    .map(WireHeader::frame_len)
}

/// Incremental decoder that buffers at most one validated frame at a time.
#[derive(Debug)]
pub struct FrameDecoder {
    codec: WireCodec,
    buffer: Vec<u8>,
    expected_frame_len: Option<usize>,
}

impl FrameDecoder {
    /// Creates an incremental decoder.
    pub const fn new(codec: WireCodec) -> Self {
        Self {
            codec,
            buffer: Vec::new(),
            expected_frame_len: None,
        }
    }

    /// Pushes an arbitrary byte chunk and returns every complete frame found in order.
    ///
    /// The internal buffer never grows beyond one configured maximum frame. Concatenated frames
    /// are emitted independently, and partial headers or payloads remain buffered for the next
    /// call.
    ///
    /// # Errors
    ///
    /// Returns [`DecodeError`] on the first malformed frame. The decoder resets itself after an
    /// error so the caller can decide whether to continue with a new transport segment.
    pub fn push(&mut self, mut input: &[u8]) -> Result<Vec<WireFrame>, DecodeError> {
        let mut frames = Vec::new();
        while !input.is_empty() {
            let target_len = match self.expected_frame_len {
                Some(length) => length,
                None => WIRE_HEADER_LEN,
            };
            let missing = target_len - self.buffer.len();
            let take = missing.min(input.len());
            self.buffer.extend_from_slice(&input[..take]);
            input = &input[take..];

            if self.buffer.len() < target_len {
                continue;
            }

            if self.expected_frame_len.is_none() {
                match self.codec.decode(&self.buffer) {
                    Ok(DecodeOutcome::Incomplete {
                        minimum_additional,
                    }) => {
                        let expected = self
                            .buffer
                            .len()
                            .checked_add(minimum_additional)
                            .ok_or(DecodeError::LengthConversion)?;
                        self.expected_frame_len = Some(expected);
                        self.buffer.reserve(expected - self.buffer.len());
                    }
                    Ok(DecodeOutcome::Complete { frame, .. }) => {
                        frames.push(frame);
                        self.buffer.clear();
                    }
                    Err(error) => {
                        self.reset();
                        return Err(error);
                    }
                }
                continue;
            }

            match self.codec.decode_exact(&self.buffer) {
                Ok(frame) => {
                    frames.push(frame);
                    self.buffer.clear();
                    self.expected_frame_len = None;
                }
                Err(error) => {
                    self.reset();
                    return Err(error);
                }
            }
        }
        Ok(frames)
    }

    /// Returns the number of bytes currently buffered for an incomplete frame.
    pub fn pending_bytes(&self) -> usize {
        self.buffer.len()
    }

    /// Returns whether no partial frame is buffered.
    pub fn is_idle(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Discards any partial frame state.
    pub fn reset(&mut self) {
        self.buffer.clear();
        self.expected_frame_len = None;
    }
}
