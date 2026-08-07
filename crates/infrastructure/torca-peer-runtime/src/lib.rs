use core::fmt;
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::time::Duration;

use torca_contacts::{
    Contact, ContactError, ContactId, ContactRepository, PeerCredentialRepository,
};
use torca_crypto::{CryptoProvider, Ed25519HandshakeVerifier, RustCryptoProvider};
use torca_foundation::{OpaqueId, Timestamp};
use torca_peer::{PeerSession, PeerSessionError, PeerSessionState, PeerTransport};
use torca_peer_protocol::{
    HandshakePolicy, HandshakeSigner, PeerCodec, PeerMessage, build_handshake_ack,
    build_handshake_hello,
};
use torca_transport_tor::{
    IncomingPeerTransport, PeerListener, Socks5Connector, TorError, TorPeerTransport,
    TOR_PEER_VIRTUAL_PORT,
};

const MAX_CLOCK_SKEW_MS: i64 = 2 * 60 * 1000;
const MAX_PENDING_INCOMING: usize = 64;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PeerRuntimeError {
    Listener,
    Repository,
    Protocol,
    Unauthorized,
    DuplicateConnection,
    PendingLimit,
    Randomness,
    ContactNotFound,
}
impl fmt::Display for PeerRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for PeerRuntimeError {}

#[must_use]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PeerAcceptReport {
    pub accepted: usize,
    pub authenticated: usize,
    pub rejected: usize,
}

#[must_use]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PeerPollReport {
    pub outgoing_ready: usize,
    pub disconnected: usize,
}

type IncomingSession = PeerSession<IncomingPeerTransport, Ed25519HandshakeVerifier>;
type OutgoingSession = PeerSession<TorPeerTransport, Ed25519HandshakeVerifier>;

/// Authenticated owner of inbound and outbound Tor peer sessions.
pub struct PeerRuntime<S, K> {
    listener: PeerListener,
    relationships: S,
    signer: K,
    local_identity_id: OpaqueId,
    socks_address: SocketAddr,
    connect_timeout: Duration,
    random: RustCryptoProvider,
    pending: Vec<IncomingPeerTransport>,
    incoming: BTreeMap<ContactId, IncomingSession>,
    outgoing: BTreeMap<ContactId, OutgoingSession>,
}

impl<S, K> PeerRuntime<S, K>
where
    S: ContactRepository + PeerCredentialRepository,
    K: HandshakeSigner,
{
    pub const fn new(
        listener: PeerListener,
        relationships: S,
        signer: K,
        local_identity_id: OpaqueId,
        socks_address: SocketAddr,
        connect_timeout: Duration,
    ) -> Self {
        Self {
            listener,
            relationships,
            signer,
            local_identity_id,
            socks_address,
            connect_timeout,
            random: RustCryptoProvider,
            pending: Vec::new(),
            incoming: BTreeMap::new(),
            outgoing: BTreeMap::new(),
        }
    }

    pub fn run_accept_once(&mut self, now: Timestamp) -> Result<PeerAcceptReport, PeerRuntimeError> {
        let mut report = PeerAcceptReport::default();
        while self.pending.len() < MAX_PENDING_INCOMING {
            match self.listener.try_accept_transport().map_err(map_tor)? {
                Some(transport) => {
                    self.pending.push(transport);
                    report.accepted += 1;
                }
                None => break,
            }
        }
        if self.pending.len() >= MAX_PENDING_INCOMING
            && self.listener.try_accept_transport().map_err(map_tor)?.is_some()
        {
            report.rejected += 1;
        }

        let mut index = 0;
        while index < self.pending.len() {
            match self.try_authenticate(index, now) {
                Ok(AuthOutcome::Waiting) => index += 1,
                Ok(AuthOutcome::Authenticated) => report.authenticated += 1,
                Err(_) => {
                    let mut rejected = self.pending.swap_remove(index);
                    let _ = rejected.close();
                    report.rejected += 1;
                }
            }
        }
        Ok(report)
    }

    /// Starts one authenticated outgoing onion-service session when no outgoing attempt exists.
    pub fn connect_contact(
        &mut self,
        contact_id: ContactId,
        now: Timestamp,
    ) -> Result<(), PeerRuntimeError> {
        if self.outgoing.contains_key(&contact_id) {
            return Err(PeerRuntimeError::DuplicateConnection);
        }
        let contact = self
            .relationships
            .get(contact_id)
            .map_err(map_contact)?
            .ok_or(PeerRuntimeError::ContactNotFound)?;
        let verifier = verifier_for(&contact)?;
        let policy = HandshakePolicy {
            expected_identity: contact.remote_identity().identity_id().to_opaque(),
            expected_capability: contact.route().capability_id(),
            max_clock_skew_ms: MAX_CLOCK_SKEW_MS,
        };
        let connector = Socks5Connector::new(self.socks_address, self.connect_timeout);
        let transport = TorPeerTransport::new(
            connector,
            contact.route().onion_address(),
            TOR_PEER_VIRTUAL_PORT,
        );
        let mut session = PeerSession::new(transport, verifier, policy);
        let session_id = self.random_id()?;
        let nonce = self.random_nonce()?;
        let hello = build_handshake_hello(
            session_id,
            self.local_identity_id,
            contact.route().capability_id(),
            now,
            nonce,
            &self.signer,
        )
        .map_err(|_| PeerRuntimeError::Protocol)?;
        session.connect(hello).map_err(map_session)?;
        self.outgoing.insert(contact_id, session);
        Ok(())
    }

    /// Advances outgoing handshakes and liveness without blocking the runtime supervisor.
    pub fn poll_outgoing(&mut self, now: Timestamp) -> PeerPollReport {
        let ids: Vec<_> = self.outgoing.keys().copied().collect();
        let mut report = PeerPollReport::default();
        for contact_id in ids {
            let Some(session) = self.outgoing.get_mut(&contact_id) else {
                continue;
            };
            let was_ready = session.state() == PeerSessionState::Ready;
            match session.poll(now) {
                Ok(_) => {
                    if !was_ready && session.state() == PeerSessionState::Ready {
                        report.outgoing_ready += 1;
                    }
                }
                Err(_) => {
                    report.disconnected += 1;
                }
            }
        }
        report
    }

    pub fn incoming_session_mut(&mut self, contact_id: ContactId) -> Option<&mut IncomingSession> {
        self.incoming.get_mut(&contact_id)
    }

    pub fn outgoing_session_mut(&mut self, contact_id: ContactId) -> Option<&mut OutgoingSession> {
        self.outgoing.get_mut(&contact_id)
    }

    pub fn has_ready_incoming(&self, contact_id: ContactId) -> bool {
        self.incoming
            .get(&contact_id)
            .is_some_and(|session| session.state() == PeerSessionState::Ready)
    }

    pub fn has_ready_outgoing(&self, contact_id: ContactId) -> bool {
        self.outgoing
            .get(&contact_id)
            .is_some_and(|session| session.state() == PeerSessionState::Ready)
    }

    pub fn into_parts(self) -> (PeerListener, S, K) {
        (self.listener, self.relationships, self.signer)
    }

    fn try_authenticate(
        &mut self,
        index: usize,
        now: Timestamp,
    ) -> Result<AuthOutcome, PeerRuntimeError> {
        let payload = match self.pending[index].try_receive() {
            Ok(Some(payload)) => payload,
            Ok(None) => return Ok(AuthOutcome::Waiting),
            Err(_) => return Err(PeerRuntimeError::Protocol),
        };
        let hello = match PeerCodec::decode(&payload).map_err(|_| PeerRuntimeError::Protocol)? {
            PeerMessage::Hello(hello) => hello,
            _ => return Err(PeerRuntimeError::Protocol),
        };
        let contact = self.contact_for_identity(hello.identity_id)?;
        let credential = self
            .relationships
            .credential_for_contact(contact.id())
            .map_err(map_contact)?
            .ok_or(PeerRuntimeError::Unauthorized)?;
        if hello.capability_id != credential.local_capability_id() {
            return Err(PeerRuntimeError::Unauthorized);
        }
        if self.incoming.contains_key(&contact.id()) {
            return Err(PeerRuntimeError::DuplicateConnection);
        }

        let verifier = verifier_for(&contact)?;
        let policy = HandshakePolicy {
            expected_identity: contact.remote_identity().identity_id().to_opaque(),
            expected_capability: credential.local_capability_id(),
            max_clock_skew_ms: MAX_CLOCK_SKEW_MS,
        };
        let transport = self.pending.swap_remove(index);
        let mut session = PeerSession::new(transport, verifier, policy);
        session.receive(&payload, now).map_err(map_session)?;
        let ack = build_handshake_ack(hello.session_id, hello.nonce, &self.signer)
            .map_err(|_| PeerRuntimeError::Protocol)?;
        session.send_handshake_ack(ack).map_err(map_session)?;
        self.incoming.insert(contact.id(), session);
        Ok(AuthOutcome::Authenticated)
    }

    fn contact_for_identity(&self, identity_id: OpaqueId) -> Result<Contact, PeerRuntimeError> {
        self.relationships
            .list()
            .map_err(map_contact)?
            .into_iter()
            .find(|contact| contact.remote_identity().identity_id().to_opaque() == identity_id)
            .ok_or(PeerRuntimeError::Unauthorized)
    }

    fn random_id(&mut self) -> Result<OpaqueId, PeerRuntimeError> {
        for _ in 0..8 {
            let mut bytes = [0_u8; 16];
            self.random
                .fill_random(&mut bytes)
                .map_err(|_| PeerRuntimeError::Randomness)?;
            let id = OpaqueId::from_bytes(bytes);
            if !id.is_nil() {
                return Ok(id);
            }
        }
        Err(PeerRuntimeError::Randomness)
    }

    fn random_nonce(&mut self) -> Result<[u8; 32], PeerRuntimeError> {
        let mut nonce = [0_u8; 32];
        self.random
            .fill_random(&mut nonce)
            .map_err(|_| PeerRuntimeError::Randomness)?;
        Ok(nonce)
    }
}

enum AuthOutcome {
    Waiting,
    Authenticated,
}

fn verifier_for(contact: &Contact) -> Result<Ed25519HandshakeVerifier, PeerRuntimeError> {
    let public: [u8; 32] = contact
        .remote_identity()
        .key()
        .public_key()
        .try_into()
        .map_err(|_| PeerRuntimeError::Unauthorized)?;
    Ok(Ed25519HandshakeVerifier::from_bytes(public))
}

fn map_contact(_: ContactError) -> PeerRuntimeError {
    PeerRuntimeError::Repository
}
fn map_session(_: PeerSessionError) -> PeerRuntimeError {
    PeerRuntimeError::Protocol
}
fn map_tor(_: TorError) -> PeerRuntimeError {
    PeerRuntimeError::Listener
}
