/// Encoded byte length of every version-1 generic wire header.
pub const WIRE_HEADER_LEN: usize = 52;

/// Default maximum payload length: four mebibytes.
pub const DEFAULT_MAX_PAYLOAD_LEN: usize = 4 * 1024 * 1024;

/// Hard safety ceiling accepted by the generic codec: 256 mebibytes.
pub const HARD_MAX_PAYLOAD_LEN: usize = 256 * 1024 * 1024;

/// Strict allocation and frame-size limits used by a codec.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WireLimits {
    max_payload_len: usize,
}

impl WireLimits {
    /// Default limits for generic Torca frames.
    pub const DEFAULT: Self = Self { max_payload_len: DEFAULT_MAX_PAYLOAD_LEN };

    /// Creates limits when the payload bound is non-zero and representable by the header.
    pub const fn new(max_payload_len: usize) -> Option<Self> {
        if max_payload_len == 0 || max_payload_len > HARD_MAX_PAYLOAD_LEN {
            None
        } else {
            Some(Self { max_payload_len })
        }
    }

    /// Returns the maximum payload length.
    pub const fn max_payload_len(self) -> usize {
        self.max_payload_len
    }

    /// Returns the maximum complete frame length.
    pub const fn max_frame_len(self) -> usize {
        WIRE_HEADER_LEN + self.max_payload_len
    }
}

impl Default for WireLimits {
    fn default() -> Self {
        Self::DEFAULT
    }
}
