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

use torca_pairing_service_protocol::{
    PairingServiceCode, PairingServiceDelivery, PairingServiceMessageId, PairingServiceOperationId,
    PairingServiceRequest, PairingServiceResponse, PairingServiceSequence, PairingServiceSideToken,
    PairingServiceSlotCapability, PairingServiceSlotId,
};

pub use error::{
    PairingServiceTransportError, PairingServiceTransportFailureKind, RendezvousClientError,
};
pub use scripted::ScriptedPairingServiceTransport;
pub use tcp::TcpPairingServiceTransport;

/// Provider-neutral request/response transport for the short-lived pairing
/// service. The historical `PairingServiceTransport` name remains an alias for old
/// Tor adapters.
pub trait PairingServiceTransport {
    fn invalidate(&mut self);
    fn reconnect(&mut self) -> Result<(), PairingServiceTransportError>;
    fn exchange(
        &mut self,
        request: &PairingServiceRequest,
        timeout: Duration,
    ) -> Result<PairingServiceResponse, PairingServiceTransportError>;
}

/// Neutral client-facing name for the pairing-service transport.
pub use PairingServiceTransport as PairingServiceClient;

/// Exchanges one framed relay request over a connected byte stream. Provider
/// adapters decide how the stream is opened; the shared client owns framing
/// and protocol validation.
pub fn exchange_tcp_stream(
    stream: &mut TcpStream,
    request: &PairingServiceRequest,
    timeout: Duration,
) -> Result<PairingServiceResponse, PairingServiceTransportError> {
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
        code: PairingServiceCode,
        expires_at: torca_foundation::Timestamp,
        creator_blob: Vec<u8>,
        slot_capability: PairingServiceSlotCapability,
        creator_token: PairingServiceSideToken,
        ticket: [u8; 16],
    ) -> Result<(PairingServiceSlotId, torca_foundation::Timestamp), RendezvousClientError> {
        let response = self.exchange(PairingServiceRequest::Open {
            operation_id: PairingServiceOperationId(slot_capability.0),
            code,
            expires_at,
            creator_blob,
            slot_capability,
            creator_token,
            ticket: torca_pairing_service_protocol::PairingServiceJoinTicket(ticket),
        })?;
        match response {
            PairingServiceResponse::Opened { slot_id, expires_at } => Ok((slot_id, expires_at)),
            _ => Err(RendezvousClientError::UnexpectedResponse),
        }
    }

    pub fn join(
        &mut self,
        code: PairingServiceCode,
        joiner_blob: Vec<u8>,
        joiner_token: PairingServiceSideToken,
        ticket: Option<[u8; 16]>,
    ) -> Result<(PairingServiceSlotId, torca_foundation::Timestamp, Vec<u8>), RendezvousClientError>
    {
        let response = self.exchange(PairingServiceRequest::Join {
            operation_id: PairingServiceOperationId(joiner_token.0),
            code,
            joiner_blob,
            joiner_token,
            ticket: ticket.map(torca_pairing_service_protocol::PairingServiceJoinTicket),
        })?;
        match response {
            PairingServiceResponse::Joined { slot_id, expires_at, creator_blob } => {
                Ok((slot_id, expires_at, creator_blob))
            }
            _ => Err(RendezvousClientError::UnexpectedResponse),
        }
    }

    pub fn push(
        &mut self,
        message_id: OpaqueId,
        slot_id: PairingServiceSlotId,
        token: PairingServiceSideToken,
        blob: Vec<u8>,
    ) -> Result<(), RendezvousClientError> {
        let response = self.exchange(PairingServiceRequest::Push {
            operation_id: PairingServiceOperationId(message_id),
            message_id: PairingServiceMessageId(message_id),
            slot_id,
            token,
            blob,
        })?;
        match response {
            PairingServiceResponse::Accepted => Ok(()),
            _ => Err(RendezvousClientError::UnexpectedResponse),
        }
    }

    pub fn poll(
        &mut self,
        slot_id: PairingServiceSlotId,
        token: PairingServiceSideToken,
        after: PairingServiceSequence,
    ) -> Result<Vec<PairingServiceDelivery>, RendezvousClientError> {
        let response = self.exchange(PairingServiceRequest::Poll { slot_id, token, after })?;
        match response {
            PairingServiceResponse::Deliveries(deliveries) => Ok(deliveries),
            _ => Err(RendezvousClientError::UnexpectedResponse),
        }
    }

    pub fn ack(
        &mut self,
        slot_id: PairingServiceSlotId,
        token: PairingServiceSideToken,
        up_to: PairingServiceSequence,
    ) -> Result<(), RendezvousClientError> {
        let response = self.exchange(PairingServiceRequest::Ack { slot_id, token, up_to })?;
        match response {
            PairingServiceResponse::Acked(acked) if acked == up_to => Ok(()),
            _ => Err(RendezvousClientError::UnexpectedResponse),
        }
    }

    pub fn close(
        &mut self,
        slot_id: PairingServiceSlotId,
        capability: PairingServiceSlotCapability,
    ) -> Result<(), RendezvousClientError> {
        let response = self.exchange(PairingServiceRequest::Close { slot_id, capability })?;
        match response {
            PairingServiceResponse::Closed => Ok(()),
            _ => Err(RendezvousClientError::UnexpectedResponse),
        }
    }

    fn exchange(
        &mut self,
        request: PairingServiceRequest,
    ) -> Result<PairingServiceResponse, RendezvousClientError> {
        self.observe(Some(TransportDirection::Tx), OperationPhase::Started);
        let result = match self.transport.exchange(&request, self.timeout) {
            Ok(response) => checked_response(response),
            Err(first_error) if !matches!(request, PairingServiceRequest::Close { .. }) => {
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

fn checked_response(
    response: PairingServiceResponse,
) -> Result<PairingServiceResponse, RendezvousClientError> {
    match response {
        PairingServiceResponse::Error(error) => Err(RendezvousClientError::Protocol(error)),
        response => Ok(response),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use torca_foundation::OpaqueId;
    use torca_pairing_service_protocol::{PairingServiceSideToken, PairingServiceSlotId};

    #[test]
    fn retries_an_idempotent_request_after_a_lost_response() {
        let mut transport = ScriptedPairingServiceTransport::default();
        transport.reconnect().expect("initial connection");
        // The relay may have committed the first request and the return path
        // may then disappear. The caller receives a replay of exactly the
        // same request; relay operation IDs make mutation requests safe and
        // Poll itself is non-destructive.
        transport.push_response(Err(PairingServiceTransportError {
            kind: PairingServiceTransportFailureKind::Disconnected,
            request_was_sent: true,
        }));
        transport.push_response(Ok(PairingServiceResponse::Deliveries(Vec::new())));
        let mut client = RendezvousClient::new(transport, Duration::from_millis(25));
        client
            .poll(
                PairingServiceSlotId(OpaqueId::from_u128(1)),
                PairingServiceSideToken(OpaqueId::from_u128(2)),
                PairingServiceSequence(0),
            )
            .expect("replayed poll");
        assert_eq!(client.transport.requests().len(), 2);
        assert_eq!(client.transport.requests()[0], client.transport.requests()[1]);
    }

    #[test]
    fn close_is_not_replayed_after_an_unknown_outcome() {
        let mut transport = ScriptedPairingServiceTransport::default();
        transport.reconnect().expect("initial connection");
        transport.push_response(Err(PairingServiceTransportError {
            kind: PairingServiceTransportFailureKind::Disconnected,
            request_was_sent: true,
        }));
        let mut client = RendezvousClient::new(transport, Duration::from_millis(25));
        let result = client.close(
            PairingServiceSlotId(OpaqueId::from_u128(1)),
            torca_pairing_service_protocol::PairingServiceSlotCapability(OpaqueId::from_u128(3)),
        );
        assert!(matches!(result, Err(RendezvousClientError::OutcomeUnknown(_))));
        assert_eq!(client.transport.requests().len(), 1);
    }
}
