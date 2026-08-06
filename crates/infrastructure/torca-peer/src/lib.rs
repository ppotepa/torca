//! Transport-independent authenticated peer-session lifecycle.

use core::fmt;
use std::collections::{BTreeMap, VecDeque};
use torca_foundation::{OpaqueId, Timestamp};
use torca_peer_protocol::{AckStatus, HandshakeAck, HandshakeHello, HandshakePolicy, HandshakeVerifier, PeerCodec, PeerMessage, PeerProtocolError};
#[derive(Clone, Copy, Debug, Eq, PartialEq)] pub enum PeerSessionState { Disconnected, Connecting, Handshaking, Ready, Reconnecting, Closed, Failed }
#[must_use] #[derive(Clone, Debug, Eq, PartialEq)] pub struct PendingEnvelope { pub envelope_id: OpaqueId, pub message_kind: u16, pub ciphertext: Vec<u8>, pub attempts: u32 }
#[derive(Clone, Debug, Eq, PartialEq)] pub struct PeerTransportError(pub String); impl fmt::Display for PeerTransportError { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(&self.0) } } impl std::error::Error for PeerTransportError {}
pub trait PeerTransport { fn connect(&mut self) -> Result<(), PeerTransportError>; fn send(&mut self, payload: Vec<u8>) -> Result<(), PeerTransportError>; fn try_receive(&mut self) -> Result<Option<Vec<u8>>, PeerTransportError>; fn close(&mut self) -> Result<(), PeerTransportError>; }
#[derive(Clone, Debug, Eq, PartialEq)] pub enum PeerSessionError { Transport(PeerTransportError), Protocol(PeerProtocolError), InvalidState, ChallengeMismatch, Rejected }
impl fmt::Display for PeerSessionError { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{self:?}") } } impl std::error::Error for PeerSessionError {} impl From<PeerTransportError> for PeerSessionError { fn from(value: PeerTransportError) -> Self { Self::Transport(value) } } impl From<PeerProtocolError> for PeerSessionError { fn from(value: PeerProtocolError) -> Self { Self::Protocol(value) } }
pub struct PeerSession<T, V> { state: PeerSessionState, transport: T, verifier: V, policy: HandshakePolicy, local_challenge: Option<(OpaqueId, [u8; 32])>, remote_challenge: Option<(OpaqueId, [u8; 32])>, pending: BTreeMap<OpaqueId, PendingEnvelope> }
impl<T: PeerTransport, V: HandshakeVerifier> PeerSession<T, V> {
    pub const fn new(transport: T, verifier: V, policy: HandshakePolicy) -> Self { Self { state: PeerSessionState::Disconnected, transport, verifier, policy, local_challenge: None, remote_challenge: None, pending: BTreeMap::new() } }
    pub const fn state(&self) -> PeerSessionState { self.state }
    pub fn connect(&mut self, hello: HandshakeHello) -> Result<(), PeerSessionError> { if !matches!(self.state, PeerSessionState::Disconnected | PeerSessionState::Reconnecting) { return Err(PeerSessionError::InvalidState); } self.state = PeerSessionState::Connecting; self.transport.connect()?; self.state = PeerSessionState::Handshaking; self.local_challenge = Some((hello.session_id, hello.nonce)); self.transport.send(PeerCodec::encode(&PeerMessage::Hello(hello))?)?; Ok(()) }
    pub fn send_handshake_ack(&mut self, ack: HandshakeAck) -> Result<(), PeerSessionError> { let expected = self.remote_challenge.ok_or(PeerSessionError::InvalidState)?; if expected != (ack.session_id, ack.nonce) { return Err(PeerSessionError::ChallengeMismatch); } self.transport.send(PeerCodec::encode(&PeerMessage::HelloAck(ack))?)?; self.remote_challenge = None; self.state = PeerSessionState::Ready; Ok(()) }
    pub fn receive(&mut self, payload: &[u8], now: Timestamp) -> Result<Option<PeerMessage>, PeerSessionError> { let message = PeerCodec::decode(payload)?; match &message {
        PeerMessage::Hello(hello) => { if matches!(self.state, PeerSessionState::Ready | PeerSessionState::Closed | PeerSessionState::Failed) { return Err(PeerSessionError::InvalidState); } self.policy.validate_hello(hello, now, &self.verifier)?; self.remote_challenge = Some((hello.session_id, hello.nonce)); self.state = PeerSessionState::Handshaking; }
        PeerMessage::HelloAck(ack) => { let (session, nonce) = self.local_challenge.ok_or(PeerSessionError::InvalidState)?; self.policy.validate_ack(ack, session, nonce, &self.verifier)?; self.local_challenge = None; self.state = PeerSessionState::Ready; }
        PeerMessage::Ack { envelope_id, status } => { if *status == AckStatus::Rejected { return Err(PeerSessionError::Rejected); } self.pending.remove(envelope_id); }
        PeerMessage::Ping(value) => { self.transport.send(PeerCodec::encode(&PeerMessage::Pong(*value))?)?; }
        PeerMessage::Pong(_) => {}
        PeerMessage::Data { .. } => { if self.state != PeerSessionState::Ready { return Err(PeerSessionError::InvalidState); } }
    } Ok(Some(message)) }
    pub fn send_data(&mut self, envelope_id: OpaqueId, message_kind: u16, ciphertext: Vec<u8>) -> Result<(), PeerSessionError> { if self.state != PeerSessionState::Ready { return Err(PeerSessionError::InvalidState); } let record = PendingEnvelope { envelope_id, message_kind, ciphertext, attempts: 1 }; self.transport.send(PeerCodec::encode(&PeerMessage::Data { envelope_id, message_kind, ciphertext: record.ciphertext.clone() })?)?; self.pending.insert(envelope_id, record); Ok(()) }
    pub fn resend_pending(&mut self) -> Result<usize, PeerSessionError> { if self.state != PeerSessionState::Ready { return Err(PeerSessionError::InvalidState); } for record in self.pending.values_mut() { record.attempts = record.attempts.saturating_add(1); self.transport.send(PeerCodec::encode(&PeerMessage::Data { envelope_id: record.envelope_id, message_kind: record.message_kind, ciphertext: record.ciphertext.clone() })?)?; } Ok(self.pending.len()) }
    pub fn disconnected(&mut self) { if self.state != PeerSessionState::Closed { self.state = PeerSessionState::Reconnecting; self.local_challenge = None; self.remote_challenge = None; } }
    pub fn close(&mut self) -> Result<(), PeerSessionError> { self.transport.close()?; self.state = PeerSessionState::Closed; self.local_challenge = None; self.remote_challenge = None; Ok(()) }
    pub fn pending_count(&self) -> usize { self.pending.len() }
}
#[derive(Clone, Debug, Default)] pub struct MemoryPeerTransport { connected: bool, incoming: VecDeque<Vec<u8>>, outgoing: VecDeque<Vec<u8>> }
impl MemoryPeerTransport { pub fn push_incoming(&mut self, payload: Vec<u8>) { self.incoming.push_back(payload); } pub fn pop_outgoing(&mut self) -> Option<Vec<u8>> { self.outgoing.pop_front() } }
impl PeerTransport for MemoryPeerTransport { fn connect(&mut self) -> Result<(), PeerTransportError> { self.connected = true; Ok(()) } fn send(&mut self, payload: Vec<u8>) -> Result<(), PeerTransportError> { if !self.connected { return Err(PeerTransportError("not connected".into())); } self.outgoing.push_back(payload); Ok(()) } fn try_receive(&mut self) -> Result<Option<Vec<u8>>, PeerTransportError> { Ok(self.incoming.pop_front()) } fn close(&mut self) -> Result<(), PeerTransportError> { self.connected = false; Ok(()) } }
