//! Transport-agnostic rendezvous client for the ephemeral pairing relay.
//!
//! This crate owns connection recovery and relay request/response validation. It deliberately does
//! not own pairing state transitions or long-term message delivery.

mod pairing;
mod stream;
mod tcp;
mod tor;

use core::fmt;
use std::time::Duration;

use torca_relay_protocol::{
    RelayCode, RelayProtocolError, RelayRequest, RelayResponse, RelaySideToken,
    RelaySlotCapability, RelaySlotId,
};

pub use tcp::TcpRelayTransport;
pub use tor::TorRelayTransport;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelayTransportFailureKind {
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

pub trait RelayTransport {
    fn reconnect(&mut self) -> Result<(), RelayTransportError>;
    fn exchange(
        &mut self,
        request: &RelayRequest,
        timeout: Duration,
    ) -> Result<RelayResponse, RelayTransportError>;
}

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

pub struct RendezvousClient<T> {
    transport: T,
    timeout: Duration,
}
impl<T> RendezvousClient<T> {
    pub const fn new(transport: T, timeout: Duration) -> Self {
        Self { transport, timeout }
    }
    pub const fn timeout(&self) -> Duration {
        self.timeout
    }
    pub fn into_transport(self) -> T {
        self.transport
    }
}

impl<T: RelayTransport> RendezvousClient<T> {
    pub fn open(
        &mut self,
        code: RelayCode,
        expires_at: torca_foundation::Timestamp,
        creator_blob: Vec<u8>,
        slot_capability: RelaySlotCapability,
        creator_token: RelaySideToken,
    ) -> Result<RelaySlotId, RendezvousClientError> {
        let response = self.exchange(RelayRequest::Open {
            code,
            expires_at,
            creator_blob,
            slot_capability,
            creator_token,
        })?;
        match response {
            RelayResponse::Opened { slot_id } => Ok(slot_id),
            _ => Err(RendezvousClientError::UnexpectedResponse),
        }
    }

    pub fn join(
        &mut self,
        code: RelayCode,
        joiner_blob: Vec<u8>,
        joiner_token: RelaySideToken,
    ) -> Result<(RelaySlotId, Vec<u8>), RendezvousClientError> {
        let response = self.exchange(RelayRequest::Join { code, joiner_blob, joiner_token })?;
        match response {
            RelayResponse::Joined { slot_id, creator_blob } => Ok((slot_id, creator_blob)),
            _ => Err(RendezvousClientError::UnexpectedResponse),
        }
    }

    pub fn push(
        &mut self,
        slot_id: RelaySlotId,
        token: RelaySideToken,
        blob: Vec<u8>,
    ) -> Result<(), RendezvousClientError> {
        let response = self.exchange(RelayRequest::Push { slot_id, token, blob })?;
        match response {
            RelayResponse::Accepted => Ok(()),
            _ => Err(RendezvousClientError::UnexpectedResponse),
        }
    }

    pub fn poll(
        &mut self,
        slot_id: RelaySlotId,
        token: RelaySideToken,
    ) -> Result<Vec<Vec<u8>>, RendezvousClientError> {
        let response = self.exchange(RelayRequest::Poll { slot_id, token })?;
        match response {
            RelayResponse::Blobs(blobs) => Ok(blobs),
            _ => Err(RendezvousClientError::UnexpectedResponse),
        }
    }

    pub fn close(
        &mut self,
        slot_id: RelaySlotId,
        capability: RelaySlotCapability,
    ) -> Result<(), RendezvousClientError> {
        let response = self.exchange(RelayRequest::Close { slot_id, capability })?;
        match response {
            RelayResponse::Closed => Ok(()),
            _ => Err(RendezvousClientError::UnexpectedResponse),
        }
    }

    fn exchange(&mut self, request: RelayRequest) -> Result<RelayResponse, RendezvousClientError> {
        match self.transport.exchange(&request, self.timeout) {
            Ok(response) => checked_response(response),
            Err(error) if error.request_was_sent => {
                let _ = self.transport.reconnect();
                Err(RendezvousClientError::OutcomeUnknown(error.kind))
            }
            Err(first_error) => {
                self.transport.reconnect().map_err(RendezvousClientError::Transport)?;
                let response =
                    self.transport.exchange(&request, self.timeout).map_err(|error| {
                        if error.request_was_sent {
                            RendezvousClientError::OutcomeUnknown(error.kind)
                        } else {
                            RendezvousClientError::Transport(first_error)
                        }
                    })?;
                checked_response(response)
            }
        }
    }
}

fn checked_response(response: RelayResponse) -> Result<RelayResponse, RendezvousClientError> {
    match response {
        RelayResponse::Error(error) => Err(RendezvousClientError::Protocol(error)),
        response => Ok(response),
    }
}

#[derive(Clone, Debug, Default)]
pub struct ScriptedRelayTransport {
    connected: bool,
    responses: std::collections::VecDeque<Result<RelayResponse, RelayTransportError>>,
    requests: Vec<RelayRequest>,
}
impl ScriptedRelayTransport {
    pub fn push_response(&mut self, response: Result<RelayResponse, RelayTransportError>) {
        self.responses.push_back(response);
    }
    pub fn requests(&self) -> &[RelayRequest] {
        &self.requests
    }
}
impl RelayTransport for ScriptedRelayTransport {
    fn reconnect(&mut self) -> Result<(), RelayTransportError> {
        self.connected = true;
        Ok(())
    }
    fn exchange(
        &mut self,
        request: &RelayRequest,
        _timeout: Duration,
    ) -> Result<RelayResponse, RelayTransportError> {
        if !self.connected {
            return Err(RelayTransportError {
                kind: RelayTransportFailureKind::Disconnected,
                request_was_sent: false,
            });
        }
        self.requests.push(request.clone());
        self.responses.pop_front().unwrap_or_else(|| {
            Err(RelayTransportError {
                kind: RelayTransportFailureKind::Unavailable,
                request_was_sent: false,
            })
        })
    }
}
