//! Transport-agnostic rendezvous client for the ephemeral pairing relay.
//!
//! This crate owns connection recovery and relay request/response validation. It deliberately does
//! not own pairing state transitions or long-term message delivery.

mod error;
mod pairing;
mod scripted;
mod stream;
mod tcp;

use std::net::TcpStream;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};
use torca_connectivity::{
    ConnectivityObserver, OperationPhase, TransportDirection, TransportLayer, TransportOperation,
};
use torca_foundation::{OpaqueId, Timestamp};

use torca_relay_protocol::{
    RelayCode, RelayDelivery, RelayMessageId, RelayOperationId, RelayRequest, RelayResponse,
    RelaySequence, RelaySideToken, RelaySlotCapability, RelaySlotId,
};

pub use error::{RelayTransportError, RelayTransportFailureKind, RendezvousClientError};
pub use scripted::ScriptedRelayTransport;
pub use tcp::TcpRelayTransport;

/// Provider-neutral request/response transport for the short-lived pairing
/// service. The historical `RelayTransport` name remains an alias for old
/// Tor adapters.
pub trait PairingServiceTransport {
    fn invalidate(&mut self);
    fn reconnect(&mut self) -> Result<(), RelayTransportError>;
    fn exchange(
        &mut self,
        request: &RelayRequest,
        timeout: Duration,
    ) -> Result<RelayResponse, RelayTransportError>;
}

pub use PairingServiceTransport as RelayTransport;

/// Exchanges one framed relay request over a connected byte stream. Provider
/// adapters decide how the stream is opened; the shared client owns framing
/// and protocol validation.
pub fn exchange_tcp_stream(
    stream: &mut TcpStream,
    request: &RelayRequest,
    timeout: Duration,
) -> Result<RelayResponse, RelayTransportError> {
    stream::exchange_stream(stream, request, timeout)
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

impl<T: PairingServiceTransport> RendezvousClient<T> {
    pub fn network_changed(&mut self) {
        self.transport.invalidate();
    }

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
            operation_id: RelayOperationId(slot_capability.0),
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
            operation_id: RelayOperationId(joiner_token.0),
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
        message_id: OpaqueId,
        slot_id: RelaySlotId,
        token: RelaySideToken,
        blob: Vec<u8>,
    ) -> Result<(), RendezvousClientError> {
        let response = self.exchange(RelayRequest::Push {
            operation_id: RelayOperationId(message_id),
            message_id: RelayMessageId(message_id),
            slot_id,
            token,
            blob,
        })?;
        match response {
            RelayResponse::Accepted => Ok(()),
            _ => Err(RendezvousClientError::UnexpectedResponse),
        }
    }

    pub fn poll(
        &mut self,
        slot_id: RelaySlotId,
        token: RelaySideToken,
        after: RelaySequence,
    ) -> Result<Vec<RelayDelivery>, RendezvousClientError> {
        let response = self.exchange(RelayRequest::Poll { slot_id, token, after })?;
        match response {
            RelayResponse::Deliveries(deliveries) => Ok(deliveries),
            _ => Err(RendezvousClientError::UnexpectedResponse),
        }
    }

    pub fn ack(
        &mut self,
        slot_id: RelaySlotId,
        token: RelaySideToken,
        up_to: RelaySequence,
    ) -> Result<(), RendezvousClientError> {
        let response = self.exchange(RelayRequest::Ack { slot_id, token, up_to })?;
        match response {
            RelayResponse::Acked(acked) if acked == up_to => Ok(()),
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
            Err(first_error) if !matches!(request, RelayRequest::Close { .. }) => {
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
            Err(error) if error.request_was_sent => {
                let _ = self.transport.reconnect();
                Err(RendezvousClientError::OutcomeUnknown(error.kind))
            }
            Err(_first_error) => {
                self.transport.reconnect().map_err(RendezvousClientError::Transport)?;
                self.transport
                    .exchange(&request, self.timeout)
                    .map_err(RendezvousClientError::Transport)
                    .and_then(checked_response)
            }
        };
        match result {
            Ok(response) => {
                // TX is recorded when the request starts. Complete the
                // round-trip without a direction, then emit RX only after a
                // response was decoded. This keeps the LED channels causal
                // and prevents a successful response from masquerading as a
                // second TX event.
                self.observe(None, OperationPhase::Completed);
                self.observe(Some(TransportDirection::Rx), OperationPhase::Completed);
                Ok(response)
            }
            Err(error) => {
                self.observe(Some(TransportDirection::Tx), OperationPhase::Failed);
                Err(error)
            }
        }
    }

    fn observe(&self, direction: Option<TransportDirection>, phase: OperationPhase) {
        let Some(observer) = &self.connectivity else {
            return;
        };
        let Ok(duration) = SystemTime::now().duration_since(UNIX_EPOCH) else {
            return;
        };
        let Ok(at) =
            Timestamp::from_unix_millis(i64::try_from(duration.as_millis()).unwrap_or(i64::MAX))
        else {
            return;
        };
        observer.record(
            TransportLayer::PairingService,
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

fn checked_response(response: RelayResponse) -> Result<RelayResponse, RendezvousClientError> {
    match response {
        RelayResponse::Error(error) => Err(RendezvousClientError::Protocol(error)),
        response => Ok(response),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use torca_foundation::OpaqueId;
    use torca_relay_protocol::{RelaySideToken, RelaySlotId};

    #[test]
    fn retries_an_idempotent_request_after_a_lost_response() {
        let mut transport = ScriptedRelayTransport::default();
        transport.reconnect().expect("initial connection");
        // The relay may have committed the first request and the return path
        // may then disappear. The caller receives a replay of exactly the
        // same request; relay operation IDs make mutation requests safe and
        // Poll itself is non-destructive.
        transport.push_response(Err(RelayTransportError {
            kind: RelayTransportFailureKind::Disconnected,
            request_was_sent: true,
        }));
        transport.push_response(Ok(RelayResponse::Deliveries(Vec::new())));
        let mut client = RendezvousClient::new(transport, Duration::from_millis(25));
        client
            .poll(
                RelaySlotId(OpaqueId::from_u128(1)),
                RelaySideToken(OpaqueId::from_u128(2)),
                RelaySequence(0),
            )
            .expect("replayed poll");
        assert_eq!(client.transport.requests().len(), 2);
        assert_eq!(client.transport.requests()[0], client.transport.requests()[1]);
    }

    #[test]
    fn close_is_not_replayed_after_an_unknown_outcome() {
        let mut transport = ScriptedRelayTransport::default();
        transport.reconnect().expect("initial connection");
        transport.push_response(Err(RelayTransportError {
            kind: RelayTransportFailureKind::Disconnected,
            request_was_sent: true,
        }));
        let mut client = RendezvousClient::new(transport, Duration::from_millis(25));
        let result = client.close(
            RelaySlotId(OpaqueId::from_u128(1)),
            torca_relay_protocol::RelaySlotCapability(OpaqueId::from_u128(3)),
        );
        assert!(matches!(result, Err(RendezvousClientError::OutcomeUnknown(_))));
        assert_eq!(client.transport.requests().len(), 1);
    }
}
