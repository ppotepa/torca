use core::fmt;

use torca_relay_protocol::RelayProtocolError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelayTransportFailureKind {
    Busy,
    Unavailable,
    Timeout,
    Disconnected,
    InvalidResponse,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RelayTransportError {
    pub kind: RelayTransportFailureKind,
    pub request_was_sent: bool,
}

impl fmt::Display for RelayTransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}", self.kind)
    }
}

impl std::error::Error for RelayTransportError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RendezvousClientError {
    Protocol(RelayProtocolError),
    Transport(RelayTransportError),
    OutcomeUnknown(RelayTransportFailureKind),
    UnexpectedResponse,
}

impl fmt::Display for RendezvousClientError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for RendezvousClientError {}

impl From<RelayProtocolError> for RendezvousClientError {
    fn from(value: RelayProtocolError) -> Self {
        Self::Protocol(value)
    }
}
