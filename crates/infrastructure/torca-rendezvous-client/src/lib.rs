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
use std::time::{SystemTime, UNIX_EPOCH};
use torca_connectivity::{
    ConnectivityObserver, OperationPhase, TransportDirection, TransportLayer, TransportOperation,
};
use torca_foundation::Timestamp;

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
    connectivity: Option<ConnectivityObserver>,
}
impl<T> RendezvousClient<T> {
    pub const fn new(transport: T, timeout: Duration) -> Self {
        Self { transport, timeout, connectivity: None }
    }
    #[must_use]
    pub fn with_connectivity(mut self, connectivity: ConnectivityObserver) -> Self {
        self.connectivity = Some(connectivity);
        self
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
        ticket: [u8; 16],
    ) -> Result<(RelaySlotId, torca_foundation::Timestamp), RendezvousClientError> {
        let response = self.exchange(RelayRequest::Open {
            code,
            expires_at,
            creator_blob,
            slot_capability,
            creator_token,
            ticket: torca_relay_protocol::RelayJoinTicket(ticket),
        })?;
        match response {
            RelayResponse::Opened { slot_id, expires_at } => Ok((slot_id, expires_at)),
            _ => Err(RendezvousClientError::UnexpectedResponse),
        }
    }

    pub fn join(
        &mut self,
        code: RelayCode,
        joiner_blob: Vec<u8>,
        joiner_token: RelaySideToken,
        ticket: Option<[u8; 16]>,
    ) -> Result<(RelaySlotId, torca_foundation::Timestamp, Vec<u8>), RendezvousClientError> {
        let response = self.exchange(RelayRequest::Join {
            code,
            joiner_blob,
            joiner_token,
            ticket: ticket.map(torca_relay_protocol::RelayJoinTicket),
        })?;
        match response {
            RelayResponse::Joined { slot_id, expires_at, creator_blob } => {
                Ok((slot_id, expires_at, creator_blob))
            }
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
        self.observe(Some(TransportDirection::Tx), OperationPhase::Started);
        let result = match self.transport.exchange(&request, self.timeout) {
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
        };
        self.observe(
            Some(TransportDirection::Tx),
            if result.is_ok() { OperationPhase::Completed } else { OperationPhase::Failed },
        );
        if result.is_ok() {
            self.observe(Some(TransportDirection::Rx), OperationPhase::Completed);
        }
        result
    }

    fn observe(&self, direction: Option<TransportDirection>, phase: OperationPhase) {
        let Some(observer) = &self.connectivity else { return };
        let Ok(duration) = SystemTime::now().duration_since(UNIX_EPOCH) else { return };
        let Ok(at) =
            Timestamp::from_unix_millis(i64::try_from(duration.as_millis()).unwrap_or(i64::MAX))
        else {
            return;
        };
        for layer in [TransportLayer::Relay, TransportLayer::Tor] {
            observer.record(
                layer,
                direction,
                TransportOperation::Request,
                phase,
                None,
                at,
                None,
                None,
            );
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
