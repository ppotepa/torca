//! Authenticated Tor peer-link owner for the Torca runtime.
//!
//! One instance owns accepted/outgoing sockets, authenticated sessions, reconnect timing and a
//! bounded queue of encrypted inbound application envelopes. Durable message retry remains outside
//! this crate; this layer never stores plaintext messages or a second outbox.

use core::fmt;
use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use torca_connectivity::{
    ConnectivityObserver, OperationPhase, TransportDirection, TransportLayer, TransportOperation,
};
use torca_contacts::{
    Contact, ContactError, ContactId, ContactRepository, PeerCredentialRepository,
};
use torca_crypto::{CryptoProvider, Ed25519HandshakeVerifier, RustCryptoProvider};
use torca_foundation::{OpaqueId, Timestamp};
use torca_peer::{PeerSession, PeerSessionError, PeerSessionState, PeerTransport};
use torca_peer_protocol::{
    AckStatus, HandshakeAck, HandshakeHello, HandshakePolicy, HandshakeSigner, PeerCodec,
    PeerMessage,
};
use torca_tor::{
    PeerListener, TOR_PEER_VIRTUAL_PORT, TorPeerTransport, TorServiceHandle, TransportError,
};

const MAX_CLOCK_SKEW_MS: i64 = 2 * 60 * 1000;
const MAX_PENDING_INCOMING: usize = 64;
const MAX_INBOUND_EVENTS: usize = 256;
const MAX_PENDING_ACKS: usize = 256;
const RECONNECT_BASE_MS: u64 = 1_000;
const RECONNECT_MAX_MS: u64 = 60_000;
const POLL_SLEEP: Duration = Duration::from_millis(10);
// Never let one attachment chunk monopolize the shared peer lane. ACKs that
// arrive after this cooperative slice are retained in `pending_acks` and the
// durable job retries the same stable frame idempotently. A five-second slice
// allowed a video transfer to freeze text/control synchronization.
const MAX_ACK_WAIT_SLICE: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PeerConnectionState {
    Disconnected,
    Connecting,
    Handshaking,
    Ready,
    Reconnecting,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LinkAck {
    Accepted,
    Duplicate,
}

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InboundPeerEnvelope {
    pub contact_id: ContactId,
    pub envelope_id: OpaqueId,
    pub message_kind: u16,
    pub ciphertext: Vec<u8>,
}

#[must_use]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PeerLinkReport {
    pub accepted: usize,
    pub authenticated: usize,
    pub rejected: usize,
    pub became_ready: usize,
    pub disconnected: usize,
    pub reconnect_started: usize,
    pub inbound_queued: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PeerLinkError {
    Listener,
    Repository,
    Protocol,
    Unauthorized,
    DuplicateConnection,
    Randomness,
    ContactNotFound,
    NotReady,
    AckTimeout,
    AckRejected,
    InboundQueueFull,
    Clock,
}
impl fmt::Display for PeerLinkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for PeerLinkError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ReconnectEntry {
    failures: u32,
    next_attempt_at: Timestamp,
    in_progress: bool,
}

type IncomingSession = PeerSession<TorPeerTransport, Ed25519HandshakeVerifier>;
type OutgoingSession = PeerSession<TorPeerTransport, Ed25519HandshakeVerifier>;

pub struct PeerLink<S, K> {
    listener: PeerListener,
    relationships: S,
    signer: K,
    local_identity_id: OpaqueId,
    tor_client: TorServiceHandle,
    random: RustCryptoProvider,
    pending: Vec<TorPeerTransport>,
    incoming: BTreeMap<ContactId, IncomingSession>,
    outgoing: BTreeMap<ContactId, OutgoingSession>,
    reconnect: BTreeMap<ContactId, ReconnectEntry>,
    pending_acks: BTreeMap<(ContactId, OpaqueId), Result<LinkAck, PeerLinkError>>,
    inbound: VecDeque<InboundPeerEnvelope>,
    connectivity: Option<ConnectivityObserver>,
}

impl<S, K> PeerLink<S, K>
where
    S: ContactRepository + PeerCredentialRepository,
    K: HandshakeSigner,
{
    pub const fn new(
        listener: PeerListener,
        relationships: S,
        signer: K,
        local_identity_id: OpaqueId,
        tor_client: TorServiceHandle,
    ) -> Self {
        Self {
            listener,
            relationships,
            signer,
            local_identity_id,
            tor_client,
            random: RustCryptoProvider,
            pending: Vec::new(),
            incoming: BTreeMap::new(),
            outgoing: BTreeMap::new(),
            reconnect: BTreeMap::new(),
            pending_acks: BTreeMap::new(),
            inbound: VecDeque::new(),
            connectivity: None,
        }
    }

    #[must_use]
    pub fn with_connectivity(mut self, connectivity: ConnectivityObserver) -> Self {
        self.connectivity = Some(connectivity);
        self
    }

    /// Wakes the runtime owner as soon as the listener queues an inbound
    /// stream, avoiding a periodic accept poll.
    pub fn set_waker(&self, waker: Arc<dyn Fn() + Send + Sync>) -> Result<(), PeerLinkError> {
        self.listener.set_waker(waker).map_err(|_| PeerLinkError::Listener)
    }

    pub fn connection_state(&self, contact_id: ContactId) -> PeerConnectionState {
        if let Some(session) = self.outgoing.get(&contact_id) {
            return map_state(session.state());
        }
        if let Some(session) = self.incoming.get(&contact_id) {
            return map_state(session.state());
        }
        PeerConnectionState::Disconnected
    }

    pub fn is_ready(&self, contact_id: ContactId) -> bool {
        self.connection_state(contact_id) == PeerConnectionState::Ready
    }

    /// Ensures at most one connect/handshake is active for the contact. This method never blocks
    /// waiting for Tor or the handshake; callers can observe readiness through `maintenance`.
    pub fn ensure_connected(
        &mut self,
        contact_id: ContactId,
        now: Timestamp,
    ) -> Result<bool, PeerLinkError> {
        if matches!(
            self.connection_state(contact_id),
            PeerConnectionState::Connecting
                | PeerConnectionState::Handshaking
                | PeerConnectionState::Ready
        ) {
            return Ok(false);
        }
        self.remove_non_ready(contact_id);
        self.connect_outgoing(contact_id, now)?;
        Ok(true)
    }

    /// Advances accept/authentication, active sessions and explicitly demanded
    /// reconnect attempts once. A contact existing in the address book is not
    /// itself a demand; send/probe paths create a reconnect entry when needed.
    pub fn maintenance(
        &mut self,
        contacts: &[ContactId],
        now: Timestamp,
    ) -> Result<PeerLinkReport, PeerLinkError> {
        let mut report = PeerLinkReport::default();
        self.accept_pending(&mut report)?;
        self.authenticate_pending(now, &mut report)?;
        self.poll_sessions(now, &mut report)?;
        self.plan_disconnected(contacts);
        self.run_due_reconnects(now, &mut report)?;
        Ok(report)
    }

    /// Existing Tor streams belong to the previous route. Close them and make
    /// every known relationship immediately eligible for one serialized dial.
    pub fn network_changed(&mut self, now: Timestamp) {
        let mut contacts = self
            .incoming
            .keys()
            .chain(self.outgoing.keys())
            .chain(self.reconnect.keys())
            .copied()
            .collect::<Vec<_>>();
        contacts.sort_unstable();
        contacts.dedup();
        for (_, mut session) in std::mem::take(&mut self.incoming) {
            let _ = session.close();
        }
        for (_, mut session) in std::mem::take(&mut self.outgoing) {
            let _ = session.close();
        }
        self.pending.clear();
        self.pending_acks.clear();
        self.reconnect.clear();
        for contact_id in contacts {
            self.reconnect.insert(
                contact_id,
                ReconnectEntry { failures: 0, next_attempt_at: now, in_progress: false },
            );
        }
    }

    /// Sends one encrypted application envelope and waits for the matching protocol ACK. Incoming
    /// data observed while waiting is queued for the normal inbound handler.
    /// The application sends its ACK only after durable frame processing.
    pub fn send_and_wait_ack(
        &mut self,
        contact_id: ContactId,
        envelope_id: OpaqueId,
        message_kind: u16,
        ciphertext: Vec<u8>,
        timeout: Duration,
    ) -> Result<LinkAck, PeerLinkError> {
        self.send_and_wait_ack_with_limit(
            contact_id,
            envelope_id,
            message_kind,
            ciphertext,
            timeout,
            MAX_ACK_WAIT_SLICE,
        )
    }

    pub fn send_and_wait_ack_with_limit(
        &mut self,
        contact_id: ContactId,
        envelope_id: OpaqueId,
        message_kind: u16,
        ciphertext: Vec<u8>,
        timeout: Duration,
        wait_limit: Duration,
    ) -> Result<LinkAck, PeerLinkError> {
        self.send_envelope(contact_id, envelope_id, message_kind, ciphertext)?;
        let wait_slice = timeout.min(wait_limit);
        let deadline = Instant::now().checked_add(wait_slice).ok_or(PeerLinkError::AckTimeout)?;
        loop {
            if let Some(ack) = self.poll_envelope_ack(contact_id, envelope_id)? {
                return Ok(ack);
            }
            if Instant::now() >= deadline {
                let now = system_timestamp()?;
                if timeout <= MAX_ACK_WAIT_SLICE {
                    self.mark_disconnected(contact_id);
                    self.schedule_reconnect(contact_id, now)?;
                }
                return Err(PeerLinkError::AckTimeout);
            }
            thread::sleep(POLL_SLEEP);
        }
    }

    /// Sends an envelope without waiting for its acknowledgement.
    pub fn send_envelope(
        &mut self,
        contact_id: ContactId,
        envelope_id: OpaqueId,
        message_kind: u16,
        ciphertext: Vec<u8>,
    ) -> Result<(), PeerLinkError> {
        if !self.is_ready(contact_id) {
            let now = system_timestamp()?;
            let _ = self.ensure_connected(contact_id, now);
            return Err(PeerLinkError::NotReady);
        }
        let started_at = system_timestamp()?;
        self.observe(
            contact_id,
            Some(TransportDirection::Tx),
            TransportOperation::Envelope,
            OperationPhase::Started,
            Some(envelope_id),
            started_at,
        );
        if let Err(error) = self.send_data(contact_id, envelope_id, message_kind, ciphertext) {
            self.observe(
                contact_id,
                Some(TransportDirection::Tx),
                TransportOperation::Envelope,
                OperationPhase::Failed,
                Some(envelope_id),
                started_at,
            );
            return Err(error);
        }
        self.observe(
            contact_id,
            Some(TransportDirection::Tx),
            TransportOperation::Envelope,
            OperationPhase::Completed,
            Some(envelope_id),
            started_at,
        );
        Ok(())
    }

    /// Polls one step for an envelope acknowledgement. Other inbound data is
    /// queued and acknowledged at the transport boundary so simultaneous
    /// sends cannot deadlock while both application workers are waiting for
    /// each other's ACK. The bounded inbox is the receipt boundary; the
    /// application layer remains responsible for durable/idempotent handling.
    pub fn poll_envelope_ack(
        &mut self,
        contact_id: ContactId,
        envelope_id: OpaqueId,
    ) -> Result<Option<LinkAck>, PeerLinkError> {
        if let Some(ack) = self.pending_acks.remove(&(contact_id, envelope_id)) {
            return ack.map(Some);
        }
        let now = system_timestamp()?;
        match self.poll_contact(contact_id, now) {
            Ok(Some(PeerMessage::Ack { envelope_id: received, status })) => {
                let ack = match status {
                    AckStatus::Accepted => Ok(LinkAck::Accepted),
                    AckStatus::Duplicate => Ok(LinkAck::Duplicate),
                    AckStatus::Rejected => Err(PeerLinkError::AckRejected),
                };
                if received != envelope_id {
                    if self.pending_acks.len() >= MAX_PENDING_ACKS {
                        if let Some(oldest) = self.pending_acks.keys().next().copied() {
                            self.pending_acks.remove(&oldest);
                        }
                    }
                    self.pending_acks.insert((contact_id, received), ack);
                    return Ok(None);
                }
                self.observe(
                    contact_id,
                    Some(TransportDirection::Rx),
                    TransportOperation::Ack,
                    OperationPhase::Completed,
                    Some(envelope_id),
                    now,
                );
                ack.map(Some)
            }
            Ok(Some(PeerMessage::Data { envelope_id, message_kind, ciphertext })) => {
                self.observe(
                    contact_id,
                    Some(TransportDirection::Rx),
                    TransportOperation::Envelope,
                    OperationPhase::Completed,
                    Some(envelope_id),
                    now,
                );
                self.queue_inbound(InboundPeerEnvelope {
                    contact_id,
                    envelope_id,
                    message_kind,
                    ciphertext,
                })?;
                self.send_ack(contact_id, envelope_id, AckStatus::Accepted)?;
                Ok(None)
            }
            Ok(_) => Ok(None),
            Err(error) => {
                self.schedule_reconnect(contact_id, now)?;
                Err(error)
            }
        }
    }

    pub fn send_keepalive_and_wait_ack(
        &mut self,
        contact_id: ContactId,
        envelope_id: OpaqueId,
        message_kind: u16,
        ciphertext: Vec<u8>,
        timeout: Duration,
    ) -> Result<LinkAck, PeerLinkError> {
        let started_at = system_timestamp()?;
        self.observe(
            contact_id,
            Some(TransportDirection::Tx),
            TransportOperation::Keepalive,
            OperationPhase::Started,
            Some(envelope_id),
            started_at,
        );
        let result =
            self.send_and_wait_ack(contact_id, envelope_id, message_kind, ciphertext, timeout);
        let finished_at = system_timestamp().unwrap_or(started_at);
        self.observe(
            contact_id,
            Some(TransportDirection::Rx),
            TransportOperation::Keepalive,
            if result.is_ok() { OperationPhase::Completed } else { OperationPhase::TimedOut },
            Some(envelope_id),
            finished_at,
        );
        result
    }

    pub fn send_ack(
        &mut self,
        contact_id: ContactId,
        envelope_id: OpaqueId,
        status: AckStatus,
    ) -> Result<(), PeerLinkError> {
        if let Some(session) = self.outgoing.get_mut(&contact_id) {
            if session.state() == PeerSessionState::Ready {
                let result = session.send_ack(envelope_id, status).map_err(map_session);
                self.observe_send_ack(contact_id, envelope_id, &result);
                return result;
            }
        }
        if let Some(session) = self.incoming.get_mut(&contact_id) {
            if session.state() == PeerSessionState::Ready {
                let result = session.send_ack(envelope_id, status).map_err(map_session);
                self.observe_send_ack(contact_id, envelope_id, &result);
                return result;
            }
        }
        Err(PeerLinkError::NotReady)
    }

    pub fn take_inbound(&mut self) -> Option<InboundPeerEnvelope> {
        self.inbound.pop_front()
    }

    pub fn shutdown(&mut self) {
        for mut transport in self.pending.drain(..) {
            let _ = transport.close();
        }
        for (_, mut session) in std::mem::take(&mut self.incoming) {
            let _ = session.close();
        }
        for (_, mut session) in std::mem::take(&mut self.outgoing) {
            let _ = session.close();
        }
        self.reconnect.clear();
        self.inbound.clear();
    }

    pub fn into_parts(self) -> (PeerListener, S, K) {
        (self.listener, self.relationships, self.signer)
    }

    fn accept_pending(&mut self, report: &mut PeerLinkReport) -> Result<(), PeerLinkError> {
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
        Ok(())
    }

    fn authenticate_pending(
        &mut self,
        now: Timestamp,
        report: &mut PeerLinkReport,
    ) -> Result<(), PeerLinkError> {
        let mut index = 0;
        while index < self.pending.len() {
            match self.try_authenticate(index, now) {
                Ok(AuthOutcome::Waiting) => index += 1,
                Ok(AuthOutcome::Authenticated(contact_id)) => {
                    self.reconnect.remove(&contact_id);
                    report.authenticated += 1;
                }
                Err(_) => {
                    let mut rejected = self.pending.swap_remove(index);
                    let _ = rejected.close();
                    report.rejected += 1;
                }
            }
        }
        Ok(())
    }

    fn try_authenticate(
        &mut self,
        index: usize,
        now: Timestamp,
    ) -> Result<AuthOutcome, PeerLinkError> {
        let payload = match self.pending[index].try_receive() {
            Ok(Some(payload)) => payload,
            Ok(None) => return Ok(AuthOutcome::Waiting),
            Err(_) => return Err(PeerLinkError::Protocol),
        };
        let hello = match PeerCodec::decode(&payload).map_err(|_| PeerLinkError::Protocol)? {
            PeerMessage::Hello(hello) => hello,
            _ => return Err(PeerLinkError::Protocol),
        };
        let contact = self.contact_for_identity(hello.identity_id)?;
        let credential = self
            .relationships
            .credential_for_contact(contact.id())
            .map_err(map_contact)?
            .ok_or(PeerLinkError::Unauthorized)?;
        if hello.capability_id != credential.local_capability_id() {
            return Err(PeerLinkError::Unauthorized);
        }
        if self.incoming.contains_key(&contact.id()) {
            return Err(PeerLinkError::DuplicateConnection);
        }
        if self.outgoing.contains_key(&contact.id()) {
            if self.prefer_outgoing(&contact) {
                return Err(PeerLinkError::DuplicateConnection);
            }
            if let Some(mut outgoing) = self.outgoing.remove(&contact.id()) {
                let _ = outgoing.close();
            }
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
        let ack = HandshakeAck::signed(hello.session_id, hello.nonce, &self.signer)
            .map_err(|_| PeerLinkError::Protocol)?;
        session.send_handshake_ack(ack).map_err(map_session)?;
        let contact_id = contact.id();
        self.incoming.insert(contact_id, session);
        self.observe(
            contact_id,
            Some(TransportDirection::Rx),
            TransportOperation::Handshake,
            OperationPhase::Completed,
            None,
            now,
        );
        Ok(AuthOutcome::Authenticated(contact_id))
    }

    fn connect_outgoing(
        &mut self,
        contact_id: ContactId,
        now: Timestamp,
    ) -> Result<(), PeerLinkError> {
        if self.outgoing.contains_key(&contact_id) || self.incoming.contains_key(&contact_id) {
            return Err(PeerLinkError::DuplicateConnection);
        }
        let contact = self
            .relationships
            .get(contact_id)
            .map_err(map_contact)?
            .ok_or(PeerLinkError::ContactNotFound)?;
        let verifier = verifier_for(&contact)?;
        let policy = HandshakePolicy {
            expected_identity: contact.remote_identity().identity_id().to_opaque(),
            expected_capability: contact.route().capability_id(),
            max_clock_skew_ms: MAX_CLOCK_SKEW_MS,
        };
        let transport = TorPeerTransport::new(
            self.tor_client.clone(),
            contact.route().onion_address(),
            TOR_PEER_VIRTUAL_PORT,
        );
        let mut session = PeerSession::new(transport, verifier, policy);
        let session_id = self.random_id()?;
        let nonce = self.random_32()?;
        let hello = HandshakeHello::signed(
            session_id,
            self.local_identity_id,
            contact.route().capability_id(),
            now,
            nonce,
            &self.signer,
        )
        .map_err(|_| PeerLinkError::Protocol)?;
        self.observe(
            contact_id,
            None,
            TransportOperation::Connect,
            OperationPhase::Started,
            None,
            now,
        );
        if let Err(error) = session.connect(hello).map_err(map_session) {
            self.observe(
                contact_id,
                None,
                TransportOperation::Connect,
                OperationPhase::Failed,
                None,
                now,
            );
            return Err(error);
        }
        self.outgoing.insert(contact_id, session);
        Ok(())
    }

    fn observe(
        &self,
        contact_id: ContactId,
        direction: Option<TransportDirection>,
        operation: TransportOperation,
        phase: OperationPhase,
        correlation_id: Option<OpaqueId>,
        at: Timestamp,
    ) {
        if let Some(observer) = &self.connectivity {
            observer.record(
                TransportLayer::Peer(Some(contact_id.to_opaque())),
                direction,
                operation,
                phase,
                correlation_id,
                at,
                None,
                None,
            );
            observer.record(
                TransportLayer::Tor,
                direction,
                operation,
                phase,
                correlation_id,
                at,
                None,
                None,
            );
        }
    }

    fn observe_send_ack(
        &self,
        contact_id: ContactId,
        envelope_id: OpaqueId,
        result: &Result<(), PeerLinkError>,
    ) {
        if let Ok(now) = system_timestamp() {
            self.observe(
                contact_id,
                Some(TransportDirection::Tx),
                TransportOperation::Ack,
                if result.is_ok() { OperationPhase::Completed } else { OperationPhase::Failed },
                Some(envelope_id),
                now,
            );
        }
    }

    fn poll_sessions(
        &mut self,
        now: Timestamp,
        report: &mut PeerLinkReport,
    ) -> Result<(), PeerLinkError> {
        let outgoing_ids: Vec<_> = self.outgoing.keys().copied().collect();
        for contact_id in outgoing_ids {
            let was_ready = self
                .outgoing
                .get(&contact_id)
                .is_some_and(|session| session.state() == PeerSessionState::Ready);
            match self.poll_contact(contact_id, now) {
                Ok(Some(PeerMessage::Data { envelope_id, message_kind, ciphertext })) => {
                    self.queue_inbound(InboundPeerEnvelope {
                        contact_id,
                        envelope_id,
                        message_kind,
                        ciphertext,
                    })?;
                    report.inbound_queued += 1;
                }
                Ok(Some(PeerMessage::Ack { envelope_id, status })) => {
                    self.store_pending_ack(contact_id, envelope_id, status);
                }
                Ok(_) => {
                    if !was_ready && self.is_ready(contact_id) {
                        self.reconnect.remove(&contact_id);
                        self.observe(
                            contact_id,
                            Some(TransportDirection::Rx),
                            TransportOperation::Handshake,
                            OperationPhase::Completed,
                            None,
                            now,
                        );
                        report.became_ready += 1;
                    }
                }
                Err(_) => {
                    self.schedule_reconnect(contact_id, now)?;
                    report.disconnected += 1;
                }
            }
        }

        let incoming_ids: Vec<_> = self.incoming.keys().copied().collect();
        for contact_id in incoming_ids {
            match self.poll_contact(contact_id, now) {
                Ok(Some(PeerMessage::Data { envelope_id, message_kind, ciphertext })) => {
                    self.queue_inbound(InboundPeerEnvelope {
                        contact_id,
                        envelope_id,
                        message_kind,
                        ciphertext,
                    })?;
                    report.inbound_queued += 1;
                }
                Ok(Some(PeerMessage::Ack { envelope_id, status })) => {
                    self.store_pending_ack(contact_id, envelope_id, status);
                }
                Ok(_) => {}
                Err(_) => {
                    self.mark_disconnected(contact_id);
                    if self
                        .relationships
                        .get(contact_id)
                        .map_err(map_contact)?
                        .is_some_and(|contact| self.prefer_outgoing(&contact))
                    {
                        self.schedule_reconnect(contact_id, now)?;
                    }
                    report.disconnected += 1;
                }
            }
        }
        Ok(())
    }

    fn store_pending_ack(
        &mut self,
        contact_id: ContactId,
        envelope_id: OpaqueId,
        status: AckStatus,
    ) {
        let ack = match status {
            AckStatus::Accepted => Ok(LinkAck::Accepted),
            AckStatus::Duplicate => Ok(LinkAck::Duplicate),
            AckStatus::Rejected => Err(PeerLinkError::AckRejected),
        };
        if self.pending_acks.len() >= MAX_PENDING_ACKS {
            if let Some(oldest) = self.pending_acks.keys().next().copied() {
                self.pending_acks.remove(&oldest);
            }
        }
        self.pending_acks.insert((contact_id, envelope_id), ack);
    }

    fn poll_contact(
        &mut self,
        contact_id: ContactId,
        now: Timestamp,
    ) -> Result<Option<PeerMessage>, PeerLinkError> {
        if let Some(session) = self.outgoing.get_mut(&contact_id) {
            return session.poll(now).map_err(map_session);
        }
        if let Some(session) = self.incoming.get_mut(&contact_id) {
            return session.poll(now).map_err(map_session);
        }
        Ok(None)
    }

    fn send_data(
        &mut self,
        contact_id: ContactId,
        envelope_id: OpaqueId,
        message_kind: u16,
        ciphertext: Vec<u8>,
    ) -> Result<(), PeerLinkError> {
        if let Some(session) = self.outgoing.get_mut(&contact_id) {
            if session.state() == PeerSessionState::Ready {
                return session
                    .send_data(envelope_id, message_kind, ciphertext)
                    .map_err(map_session);
            }
        }
        if let Some(session) = self.incoming.get_mut(&contact_id) {
            if session.state() == PeerSessionState::Ready {
                return session
                    .send_data(envelope_id, message_kind, ciphertext)
                    .map_err(map_session);
            }
        }
        Err(PeerLinkError::NotReady)
    }

    fn queue_inbound(&mut self, envelope: InboundPeerEnvelope) -> Result<(), PeerLinkError> {
        if self.inbound.len() >= MAX_INBOUND_EVENTS {
            return Err(PeerLinkError::InboundQueueFull);
        }
        self.inbound.push_back(envelope);
        Ok(())
    }

    fn plan_disconnected(&mut self, contacts: &[ContactId]) {
        for &contact_id in contacts {
            match self.connection_state(contact_id) {
                PeerConnectionState::Ready => {
                    self.reconnect.remove(&contact_id);
                }
                PeerConnectionState::Disconnected
                | PeerConnectionState::Reconnecting
                | PeerConnectionState::Failed => {}
                PeerConnectionState::Connecting | PeerConnectionState::Handshaking => {}
            }
        }
    }

    fn run_due_reconnects(
        &mut self,
        now: Timestamp,
        report: &mut PeerLinkReport,
    ) -> Result<(), PeerLinkError> {
        let due: Vec<_> = self
            .reconnect
            .iter()
            .filter_map(|(contact_id, entry)| {
                (!entry.in_progress && entry.next_attempt_at <= now).then_some(*contact_id)
            })
            .collect();
        for contact_id in due {
            if self.is_ready(contact_id) {
                self.reconnect.remove(&contact_id);
                continue;
            }
            self.remove_non_ready(contact_id);
            match self.connect_outgoing(contact_id, now) {
                Ok(()) => {
                    if let Some(entry) = self.reconnect.get_mut(&contact_id) {
                        entry.in_progress = true;
                    }
                    report.reconnect_started += 1;
                }
                Err(_) => self.schedule_reconnect(contact_id, now)?,
            }
        }
        Ok(())
    }

    fn schedule_reconnect(
        &mut self,
        contact_id: ContactId,
        now: Timestamp,
    ) -> Result<(), PeerLinkError> {
        if self.is_ready(contact_id) {
            self.reconnect.remove(&contact_id);
            return Ok(());
        }
        let failures =
            self.reconnect.get(&contact_id).map_or(1, |entry| entry.failures.saturating_add(1));
        let delay = self.reconnect_delay(failures)?;
        let next_attempt_at = now.checked_add(delay).ok_or(PeerLinkError::Clock)?;
        self.observe(
            contact_id,
            None,
            TransportOperation::Reconnect,
            OperationPhase::Started,
            None,
            now,
        );
        self.reconnect
            .insert(contact_id, ReconnectEntry { failures, next_attempt_at, in_progress: false });
        Ok(())
    }

    fn reconnect_delay(&mut self, failures: u32) -> Result<Duration, PeerLinkError> {
        let exponent = failures.saturating_sub(1).min(16);
        let base = RECONNECT_BASE_MS.saturating_mul(1_u64 << exponent).min(RECONNECT_MAX_MS);
        let jitter_room = (base / 4).min(RECONNECT_MAX_MS.saturating_sub(base));
        let jitter = if jitter_room == 0 {
            0
        } else {
            let mut random = [0_u8; 8];
            self.random.fill_random(&mut random).map_err(|_| PeerLinkError::Randomness)?;
            u64::from_le_bytes(random) % (jitter_room + 1)
        };
        Ok(Duration::from_millis(base + jitter))
    }

    fn remove_non_ready(&mut self, contact_id: ContactId) {
        if self
            .outgoing
            .get(&contact_id)
            .is_some_and(|session| session.state() != PeerSessionState::Ready)
        {
            if let Some(mut session) = self.outgoing.remove(&contact_id) {
                let _ = session.close();
            }
        }
        if self
            .incoming
            .get(&contact_id)
            .is_some_and(|session| session.state() != PeerSessionState::Ready)
        {
            if let Some(mut session) = self.incoming.remove(&contact_id) {
                let _ = session.close();
            }
        }
    }

    fn mark_disconnected(&mut self, contact_id: ContactId) {
        if let Some(session) = self.outgoing.get_mut(&contact_id) {
            session.disconnected();
        }
        if let Some(session) = self.incoming.get_mut(&contact_id) {
            session.disconnected();
        }
    }

    fn contact_for_identity(&self, identity_id: OpaqueId) -> Result<Contact, PeerLinkError> {
        self.relationships
            .list()
            .map_err(map_contact)?
            .into_iter()
            .find(|contact| contact.remote_identity().identity_id().to_opaque() == identity_id)
            .ok_or(PeerLinkError::Unauthorized)
    }

    fn prefer_outgoing(&self, contact: &Contact) -> bool {
        self.local_identity_id < contact.remote_identity().identity_id().to_opaque()
    }

    fn random_id(&mut self) -> Result<OpaqueId, PeerLinkError> {
        for _ in 0..8 {
            let mut bytes = [0_u8; 16];
            self.random.fill_random(&mut bytes).map_err(|_| PeerLinkError::Randomness)?;
            let id = OpaqueId::from_bytes(bytes);
            if !id.is_nil() {
                return Ok(id);
            }
        }
        Err(PeerLinkError::Randomness)
    }

    fn random_32(&mut self) -> Result<[u8; 32], PeerLinkError> {
        let mut bytes = [0_u8; 32];
        self.random.fill_random(&mut bytes).map_err(|_| PeerLinkError::Randomness)?;
        Ok(bytes)
    }
}

enum AuthOutcome {
    Waiting,
    Authenticated(ContactId),
}

fn map_state(state: PeerSessionState) -> PeerConnectionState {
    match state {
        PeerSessionState::Disconnected => PeerConnectionState::Disconnected,
        PeerSessionState::Connecting => PeerConnectionState::Connecting,
        PeerSessionState::Handshaking => PeerConnectionState::Handshaking,
        PeerSessionState::Ready => PeerConnectionState::Ready,
        PeerSessionState::Reconnecting => PeerConnectionState::Reconnecting,
        PeerSessionState::Closed | PeerSessionState::Failed => PeerConnectionState::Failed,
    }
}

fn verifier_for(contact: &Contact) -> Result<Ed25519HandshakeVerifier, PeerLinkError> {
    let public: [u8; 32] = contact
        .remote_identity()
        .key()
        .public_key()
        .try_into()
        .map_err(|_| PeerLinkError::Unauthorized)?;
    Ok(Ed25519HandshakeVerifier::from_bytes(public))
}

fn map_contact(_: ContactError) -> PeerLinkError {
    PeerLinkError::Repository
}
fn map_session(_: PeerSessionError) -> PeerLinkError {
    PeerLinkError::Protocol
}
fn map_tor(_: TransportError) -> PeerLinkError {
    PeerLinkError::Listener
}

fn system_timestamp() -> Result<Timestamp, PeerLinkError> {
    let duration =
        SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| PeerLinkError::Clock)?;
    let millis = i64::try_from(duration.as_millis()).map_err(|_| PeerLinkError::Clock)?;
    Timestamp::from_unix_millis(millis).map_err(|_| PeerLinkError::Clock)
}
