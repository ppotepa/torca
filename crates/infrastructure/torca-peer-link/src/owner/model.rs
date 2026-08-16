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
    activity: BTreeMap<ContactId, PeerActivitySnapshot>,
    connectivity: Option<ConnectivityObserver>,
}
