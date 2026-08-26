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

#[must_use]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PeerActivitySnapshot {
    pub sequence: u64,
    pub tx_frames: u64,
    pub rx_frames: u64,
    pub tx_acks: u64,
    pub rx_acks: u64,
    pub handshakes: u64,
    pub failures: u64,
    pub last_activity_at: Option<Timestamp>,
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
    InboundPending,
    InboundQueueFull,
    Clock,
}
impl fmt::Display for PeerLinkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for PeerLinkError {}

type IncomingSession = PeerSession<Box<dyn PeerTransport + Send>, Ed25519HandshakeVerifier>;
type OutgoingSession = PeerSession<Box<dyn PeerTransport + Send>, Ed25519HandshakeVerifier>;

pub struct PeerLink<S, K> {
    transport_factory: Box<dyn PeerTransportFactory>,
    relationships: S,
    signer: K,
    local_identity_id: OpaqueId,
    random: RustCryptoProvider,
    pending: Vec<Box<dyn PeerTransport + Send>>,
    incoming: BTreeMap<ContactId, IncomingSession>,
    outgoing: BTreeMap<ContactId, OutgoingSession>,
    reconnect: BTreeMap<ContactId, ReconnectEntry>,
    pending_acks: BTreeMap<(ContactId, OpaqueId), Result<LinkAck, PeerLinkError>>,
    inbound: VecDeque<InboundPeerEnvelope>,
    activity: BTreeMap<ContactId, PeerActivitySnapshot>,
    /// Highest route generation accepted from each authenticated contact.
    /// This is intentionally ephemeral; a fresh handshake re-establishes the
    /// current route after process restart.
    route_generations: BTreeMap<ContactId, u64>,
    /// Route generation most recently sent to each contact on this process.
    advertised_route_generations: BTreeMap<ContactId, u64>,
    connectivity: Option<ConnectivityObserver>,
    waker: Option<Arc<dyn Fn() + Send + Sync>>,
}
