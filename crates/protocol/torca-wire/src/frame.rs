use crate::{FrameMetadata, ProtocolFamily, WireHeader};

/// Decoded generic frame with an owned bounded payload.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WireFrame {
    header: WireHeader,
    payload: Vec<u8>,
}

impl WireFrame {
    pub(crate) fn new(header: WireHeader, payload: Vec<u8>) -> Self {
        debug_assert_eq!(header.payload_len(), payload.len());
        Self { header, payload }
    }

    /// Returns the protocol family.
    pub const fn family(&self) -> ProtocolFamily {
        self.header.family()
    }

    /// Returns generic frame metadata.
    pub const fn metadata(&self) -> FrameMetadata {
        self.header.metadata()
    }

    /// Returns the decoded header.
    pub const fn header(&self) -> WireHeader {
        self.header
    }

    /// Returns the payload bytes.
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    /// Consumes the frame and returns the payload bytes.
    pub fn into_payload(self) -> Vec<u8> {
        self.payload
    }
}
