//! Transport-independent authenticated peer-session lifecycle.
//!
//! The session owns authentication, protocol acknowledgements and ephemeral in-flight IDs. Durable
//! retry payloads belong to the message outbox, never to this transport session.

use core::fmt;
use std::collections::{BTreeSet, VecDeque};

use torca_foundation::{OpaqueId, Timestamp};
use torca_peer_protocol::{
    AckStatus, HandshakeAck, HandshakeHello, HandshakePolicy, HandshakeVerifier, PeerCodec,
    PeerMessage, PeerProtocolError,
};

/// Peer connection lifecycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PeerSessionState {
    Disconnected,
    Connecting,
    Handshaking,
    Ready,
    Reconnecting,
    Closed,
    Failed,
}

/// Redaction-safe concrete transport failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PeerTransportError(pub String);
impl fmt::Display for PeerTransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}
impl std::error::Error for PeerTransportError {}

/// Byte transport used by an authenticated peer session.
pub trait PeerTransport {
    fn connect(&mut self) -> Result<(), PeerTransportError>;
    fn send(&mut self, payload: Vec<u8>) -> Result<(), PeerTransportError>;
    fn try_receive(&mut self) -> Result<Option<Vec<u8>>, PeerTransportError>;
    fn close(&mut self) -> Result<(), PeerTransportError>;
}

/// Peer-session failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PeerSessionError {
    Transport(PeerTransportError),
    Protocol(PeerProtocolError),
    InvalidState,
    ChallengeMismatch,
    DuplicateEnvelope,
    Rejected,
}
impl fmt::Display for PeerSessionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for PeerSessionError {}
impl From<PeerTransportError> for PeerSessionError {
    fn from(value: PeerTransportError) -> Self {
        Self::Transport(value)
    }
}
impl From<PeerProtocolError> for PeerSessionError {
    fn from(value: PeerProtocolError) -> Self {
        Self::Protocol(value)
    }
}

/// One authenticated peer session.
pub struct PeerSession<T, V> {
    state: PeerSessionState,
    transport: T,
    verifier: V,
    policy: HandshakePolicy,
    local_challenge: Option<(OpaqueId, [u8; 32])>,
    remote_challenge: Option<(OpaqueId, [u8; 32])>,
    in_flight: BTreeSet<OpaqueId>,
}

impl<T: PeerTransport, V: HandshakeVerifier> PeerSession<T, V> {
    /// Creates a disconnected session.
    pub const fn new(transport: T, verifier: V, policy: HandshakePolicy) -> Self {
        Self {
            state: PeerSessionState::Disconnected,
            transport,
            verifier,
            policy,
            local_challenge: None,
            remote_challenge: None,
            in_flight: BTreeSet::new(),
        }
    }

    /// Returns current lifecycle state.
    pub const fn state(&self) -> PeerSessionState {
        self.state
    }

    /// Opens the transport and sends the authenticated initiator hello.
    pub fn connect(&mut self, hello: HandshakeHello) -> Result<(), PeerSessionError> {
        if !matches!(self.state, PeerSessionState::Disconnected | PeerSessionState::Reconnecting) {
            return Err(PeerSessionError::InvalidState);
        }
        self.state = PeerSessionState::Connecting;
        if let Err(error) = self.transport.connect() {
            self.transition_to_reconnecting();
            return Err(error.into());
        }

        let challenge = (hello.session_id, hello.nonce);
        let payload = PeerCodec::encode(&PeerMessage::Hello(hello))?;
        self.state = PeerSessionState::Handshaking;
        self.local_challenge = Some(challenge);
        if let Err(error) = self.transport.send(payload) {
            self.transition_to_reconnecting();
            return Err(error.into());
        }
        Ok(())
    }

    /// Sends the responder acknowledgement after a validated hello.
    pub fn send_handshake_ack(&mut self, ack: HandshakeAck) -> Result<(), PeerSessionError> {
        if self.state != PeerSessionState::Handshaking {
            return Err(PeerSessionError::InvalidState);
        }
        let expected = self.remote_challenge.ok_or(PeerSessionError::InvalidState)?;
        if expected != (ack.session_id, ack.nonce) {
            return Err(PeerSessionError::ChallengeMismatch);
        }
        let payload = PeerCodec::encode(&PeerMessage::HelloAck(ack))?;
        if let Err(error) = self.transport.send(payload) {
            self.transition_to_reconnecting();
            return Err(error.into());
        }
        self.remote_challenge = None;
        self.state = PeerSessionState::Ready;
        Ok(())
    }

    /// Decodes and applies one peer-protocol payload.
    pub fn receive(
        &mut self,
        payload: &[u8],
        now: Timestamp,
    ) -> Result<Option<PeerMessage>, PeerSessionError> {
        let message = PeerCodec::decode(payload)?;
        match &message {
            PeerMessage::Hello(hello) => {
                if matches!(
                    self.state,
                    PeerSessionState::Ready | PeerSessionState::Closed | PeerSessionState::Failed
                ) {
                    return Err(PeerSessionError::InvalidState);
                }
                if let Err(error) = self.policy.validate_hello(hello, now, &self.verifier) {
                    self.state = PeerSessionState::Failed;
                    return Err(error.into());
                }
                self.remote_challenge = Some((hello.session_id, hello.nonce));
                self.state = PeerSessionState::Handshaking;
            }
            PeerMessage::HelloAck(ack) => {
                let (session, nonce) =
                    self.local_challenge.ok_or(PeerSessionError::InvalidState)?;
                if let Err(error) = self.policy.validate_ack(ack, session, nonce, &self.verifier) {
                    self.state = PeerSessionState::Failed;
                    return Err(error.into());
                }
                self.local_challenge = None;
                self.state = PeerSessionState::Ready;
            }
            PeerMessage::Ack { envelope_id, status } => {
                self.in_flight.remove(envelope_id);
                if *status == AckStatus::Rejected {
                    return Err(PeerSessionError::Rejected);
                }
                // Accepted and Duplicate are both successful protocol acknowledgements. Durable
                // delivery code can complete the same outbox record idempotently.
            }
            PeerMessage::Ping(value) => {
                let response = PeerCodec::encode(&PeerMessage::Pong(*value))?;
                if let Err(error) = self.transport.send(response) {
                    self.transition_to_reconnecting();
                    return Err(error.into());
                }
            }
            PeerMessage::Pong(_) => {}
            PeerMessage::Data { .. } => {
                if self.state != PeerSessionState::Ready {
                    return Err(PeerSessionError::InvalidState);
                }
            }
        }
        Ok(Some(message))
    }

    /// Polls the concrete transport once and applies a payload when available.
    pub fn poll(&mut self, now: Timestamp) -> Result<Option<PeerMessage>, PeerSessionError> {
        let payload = match self.transport.try_receive() {
            Ok(payload) => payload,
            Err(error) => {
                self.transition_to_reconnecting();
                return Err(error.into());
            }
        };
        match payload {
            Some(payload) => self.receive(&payload, now),
            None => Ok(None),
        }
    }

    /// Sends one encrypted application envelope.
    ///
    /// Only the envelope ID is retained until an ACK arrives. Ciphertext and retry scheduling stay
    /// in the durable outbox so reconnecting or recreating a session cannot become a second queue.
    pub fn send_data(
        &mut self,
        envelope_id: OpaqueId,
        message_kind: u16,
        ciphertext: Vec<u8>,
    ) -> Result<(), PeerSessionError> {
        if self.state != PeerSessionState::Ready {
            return Err(PeerSessionError::InvalidState);
        }
        if self.in_flight.contains(&envelope_id) {
            return Err(PeerSessionError::DuplicateEnvelope);
        }
        let payload =
            PeerCodec::encode(&PeerMessage::Data { envelope_id, message_kind, ciphertext })?;
        if let Err(error) = self.transport.send(payload) {
            self.transition_to_reconnecting();
            return Err(error.into());
        }
        self.in_flight.insert(envelope_id);
        Ok(())
    }

    /// Sends a protocol acknowledgement for an inbound envelope.
    pub fn send_ack(
        &mut self,
        envelope_id: OpaqueId,
        status: AckStatus,
    ) -> Result<(), PeerSessionError> {
        if self.state != PeerSessionState::Ready {
            return Err(PeerSessionError::InvalidState);
        }
        let payload = PeerCodec::encode(&PeerMessage::Ack { envelope_id, status })?;
        if let Err(error) = self.transport.send(payload) {
            self.transition_to_reconnecting();
            return Err(error.into());
        }
        Ok(())
    }

    /// Marks an externally observed connection loss and discards ephemeral in-flight tracking.
    pub fn disconnected(&mut self) {
        self.transition_to_reconnecting();
    }

    /// Closes the transport and permanently ends this session instance.
    pub fn close(&mut self) -> Result<(), PeerSessionError> {
        let result = self.transport.close();
        self.state = PeerSessionState::Closed;
        self.local_challenge = None;
        self.remote_challenge = None;
        self.in_flight.clear();
        result.map_err(Into::into)
    }

    /// Returns the number of envelopes sent on this session but not yet acknowledged.
    pub fn in_flight_count(&self) -> usize {
        self.in_flight.len()
    }

    fn transition_to_reconnecting(&mut self) {
        if self.state != PeerSessionState::Closed {
            self.state = PeerSessionState::Reconnecting;
            self.local_challenge = None;
            self.remote_challenge = None;
            self.in_flight.clear();
        }
    }
}

/// Deterministic transport used by unit tests and previews.
#[derive(Clone, Debug, Default)]
pub struct MemoryPeerTransport {
    connected: bool,
    incoming: VecDeque<Vec<u8>>,
    outgoing: VecDeque<Vec<u8>>,
}
impl MemoryPeerTransport {
    pub fn push_incoming(&mut self, payload: Vec<u8>) {
        self.incoming.push_back(payload);
    }
    pub fn pop_outgoing(&mut self) -> Option<Vec<u8>> {
        self.outgoing.pop_front()
    }
}
impl PeerTransport for MemoryPeerTransport {
    fn connect(&mut self) -> Result<(), PeerTransportError> {
        self.connected = true;
        Ok(())
    }
    fn send(&mut self, payload: Vec<u8>) -> Result<(), PeerTransportError> {
        if !self.connected {
            return Err(PeerTransportError("not connected".into()));
        }
        self.outgoing.push_back(payload);
        Ok(())
    }
    fn try_receive(&mut self) -> Result<Option<Vec<u8>>, PeerTransportError> {
        if !self.connected {
            return Err(PeerTransportError("not connected".into()));
        }
        Ok(self.incoming.pop_front())
    }
    fn close(&mut self) -> Result<(), PeerTransportError> {
        self.connected = false;
        Ok(())
    }
}
