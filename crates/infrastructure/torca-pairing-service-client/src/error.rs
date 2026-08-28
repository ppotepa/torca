use core::fmt;

use torca_pairing_service_protocol::PairingServiceProtocolError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PairingServiceTransportFailureKind {
    Busy,
    Unavailable,
    Timeout,
    Disconnected,
    InvalidResponse,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PairingServiceTransportError {
    pub kind: PairingServiceTransportFailureKind,
    pub request_was_sent: bool,
}

impl fmt::Display for PairingServiceTransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}", self.kind)
    }
}

impl std::error::Error for PairingServiceTransportError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RendezvousClientError {
    Protocol(PairingServiceProtocolError),
    Transport(PairingServiceTransportError),
    OutcomeUnknown(PairingServiceTransportFailureKind),
    UnexpectedResponse,
}

impl fmt::Display for RendezvousClientError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for RendezvousClientError {}

impl From<PairingServiceProtocolError> for RendezvousClientError {
    fn from(value: PairingServiceProtocolError) -> Self {
        Self::Protocol(value)
    }
}
