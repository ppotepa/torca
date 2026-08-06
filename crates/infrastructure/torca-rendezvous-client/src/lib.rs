//! Transport-agnostic rendezvous client for the ephemeral pairing relay.
//!
//! This crate owns connection recovery and relay request/response validation. It deliberately does
//! not own pairing state transitions or long-term message delivery.

use core::fmt;
use std::time::Duration;

use torca_relay_protocol::{
    RelayCode, RelayProtocolError, RelayRequest, RelayResponse, RelaySide, RelaySlotId,
};

/// Redaction-safe class of relay transport failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelayTransportFailureKind {
    /// The relay connection is currently unavailable.
    Unavailable,
    /// The operation exceeded its transport deadline.
    Timeout,
    /// The connection closed unexpectedly.
    Disconnected,
    /// The remote response could not be decoded by the transport implementation.
    InvalidResponse,
}

/// Transport failure together with whether the request may already have reached the relay.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RelayTransportError {
    /// Stable non-sensitive failure class.
    pub kind: RelayTransportFailureKind,
    /// `true` when replaying the request could duplicate a completed relay operation.
    pub request_was_sent: bool,
}

impl fmt::Display for RelayTransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}", self.kind)
    }
}
impl std::error::Error for RelayTransportError {}

/// Synchronous request/response transport used by the rendezvous client.
///
/// Concrete HTTP/WebSocket/Tor transports may live outside this crate. The important contract is
/// that `request_was_sent` is conservative: when the implementation cannot prove a request was not
/// transmitted, it must return `true`.
pub trait RelayTransport {
    /// Establishes or re-establishes the configured relay connection.
    fn reconnect(&mut self) -> Result<(), RelayTransportError>;

    /// Exchanges exactly one relay request.
    fn exchange(
        &mut self,
        request: &RelayRequest,
        timeout: Duration,
    ) -> Result<RelayResponse, RelayTransportError>;
}

/// Rendezvous client failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RendezvousClientError {
    /// Relay rejected the operation according to the versioned protocol.
    Protocol(RelayProtocolError),
    /// Transport failed before the request was known to be transmitted.
    Transport(RelayTransportError),
    /// The transport failed after the request may have reached the relay.
    ///
    /// The client deliberately does not replay such requests because relay `Open`, `Join`, `Push`,
    /// `Poll`, and `Close` are not all safely idempotent.
    OutcomeUnknown(RelayTransportFailureKind),
    /// A successful transport response did not match the requested operation.
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

/// Connection-recovering client for one configured ephemeral relay.
pub struct RendezvousClient<T> {
    transport: T,
    timeout: Duration,
}

impl<T> RendezvousClient<T> {
    /// Creates a relay client with one per-operation transport deadline.
    pub const fn new(transport: T, timeout: Duration) -> Self {
        Self { transport, timeout }
    }

    /// Returns the configured transport deadline.
    pub const fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Consumes the client and returns its transport.
    pub fn into_transport(self) -> T {
        self.transport
    }
}

impl<T: RelayTransport> RendezvousClient<T> {
    /// Opens a short-lived creator slot.
    pub fn open(
        &mut self,
        code: RelayCode,
        expires_at: torca_foundation::Timestamp,
        creator_blob: Vec<u8>,
    ) -> Result<RelaySlotId, RendezvousClientError> {
        let response = self.exchange(RelayRequest::Open { code, expires_at, creator_blob })?;
        match response {
            RelayResponse::Opened { slot_id } => Ok(slot_id),
            _ => Err(RendezvousClientError::UnexpectedResponse),
        }
    }

    /// Joins a creator slot and returns its slot ID plus creator proposal blob.
    pub fn join(
        &mut self,
        code: RelayCode,
        joiner_blob: Vec<u8>,
    ) -> Result<(RelaySlotId, Vec<u8>), RendezvousClientError> {
        let response = self.exchange(RelayRequest::Join { code, joiner_blob })?;
        match response {
            RelayResponse::Joined { slot_id, creator_blob } => Ok((slot_id, creator_blob)),
            _ => Err(RendezvousClientError::UnexpectedResponse),
        }
    }

    /// Publishes one opaque pairing blob to the opposite side.
    pub fn push(
        &mut self,
        slot_id: RelaySlotId,
        side: RelaySide,
        blob: Vec<u8>,
    ) -> Result<(), RendezvousClientError> {
        let response = self.exchange(RelayRequest::Push { slot_id, side, blob })?;
        match response {
            RelayResponse::Accepted => Ok(()),
            _ => Err(RendezvousClientError::UnexpectedResponse),
        }
    }

    /// Receives currently queued pairing blobs for one side.
    pub fn poll(
        &mut self,
        slot_id: RelaySlotId,
        side: RelaySide,
    ) -> Result<Vec<Vec<u8>>, RendezvousClientError> {
        let response = self.exchange(RelayRequest::Poll { slot_id, side })?;
        match response {
            RelayResponse::Blobs(blobs) => Ok(blobs),
            _ => Err(RendezvousClientError::UnexpectedResponse),
        }
    }

    /// Closes an ephemeral slot.
    pub fn close(&mut self, slot_id: RelaySlotId) -> Result<(), RendezvousClientError> {
        let response = self.exchange(RelayRequest::Close { slot_id })?;
        match response {
            RelayResponse::Closed => Ok(()),
            _ => Err(RendezvousClientError::UnexpectedResponse),
        }
    }

    fn exchange(&mut self, request: RelayRequest) -> Result<RelayResponse, RendezvousClientError> {
        match self.transport.exchange(&request, self.timeout) {
            Ok(response) => Ok(response),
            Err(error) if error.request_was_sent => {
                // Recover the connection for the next explicit operation but never replay an
                // operation whose outcome may already have been committed by the relay.
                let _ = self.transport.reconnect();
                Err(RendezvousClientError::OutcomeUnknown(error.kind))
            }
            Err(first_error) => {
                self.transport
                    .reconnect()
                    .map_err(RendezvousClientError::Transport)?;
                self.transport.exchange(&request, self.timeout).map_err(|error| {
                    if error.request_was_sent {
                        RendezvousClientError::OutcomeUnknown(error.kind)
                    } else {
                        RendezvousClientError::Transport(first_error)
                    }
                })
            }
        }
    }
}

/// Deterministic scripted transport useful for application tests and previews.
#[derive(Clone, Debug, Default)]
pub struct ScriptedRelayTransport {
    connected: bool,
    responses: std::collections::VecDeque<Result<RelayResponse, RelayTransportError>>,
    requests: Vec<RelayRequest>,
}

impl ScriptedRelayTransport {
    /// Queues one response returned by the next exchange.
    pub fn push_response(&mut self, response: Result<RelayResponse, RelayTransportError>) {
        self.responses.push_back(response);
    }

    /// Returns requests observed by the fake transport.
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
