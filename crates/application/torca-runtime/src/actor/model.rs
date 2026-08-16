#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TorState {
    Stopped,
    Starting,
    Ready,
    Degraded,
    Failed,
}
/// Publication health of this device's onion endpoint.  This is deliberately
/// separate from `TorState`: a bootstrapped Tor client can still be waiting
/// for introduction points or descriptor publication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OnionServiceState {
    Unknown,
    Publishing,
    Reachable,
    Degraded,
    Failed,
    Stopped,
}
/// Application-owned peer session state. Infrastructure adapters map their provider state here.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PeerConnectionStatus {
    Disconnected,
    Connecting,
    Handshaking,
    Ready,
    Reconnecting,
    Failed,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PeerHealthQuality {
    Unknown,
    Excellent,
    Good,
    Fair,
    Poor,
}
#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PeerHealthSnapshot {
    pub state: PeerConnectionStatus,
    pub quality: PeerHealthQuality,
    pub rtt_ms: Option<u64>,
    pub last_success_at: Option<Timestamp>,
    pub consecutive_failures: u32,
    pub reconnect_attempt: u32,
}

/// Redacted transport activity used only to animate presentation indicators.
/// It intentionally carries no payload, address, key or message content.
#[must_use]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TransportActivitySnapshot {
    pub last_activity_at: Option<Timestamp>,
    pub sequence: u64,
}

/// Monotonic, payload-free transport counters exposed by communication
/// executors. The runtime samples deltas and records them as health evidence;
/// counters never contain message contents or identifiers.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PeerActivityEvidence {
    pub contact_id: ContactId,
    pub sequence: u64,
    pub tx_frames: u64,
    pub rx_frames: u64,
    pub tx_acks: u64,
    pub rx_acks: u64,
    pub handshakes: u64,
    pub failures: u64,
    pub last_activity_at: Option<Timestamp>,
}

#[derive(Default)]
struct TransportActivityLedger {
    peers: BTreeMap<ContactId, TransportActivitySnapshot>,
}

impl TransportActivityLedger {
    fn mark_peer(&mut self, contact_id: ContactId, now: Timestamp) {
        Self::mark(self.peers.entry(contact_id).or_default(), now);
    }

    fn mark(activity: &mut TransportActivitySnapshot, now: Timestamp) {
        activity.last_activity_at = Some(now);
        activity.sequence = activity.sequence.saturating_add(1);
    }
}
impl PeerHealthSnapshot {
    pub const fn from_connection_state(state: PeerConnectionStatus) -> Self {
        Self {
            state,
            quality: PeerHealthQuality::Unknown,
            rtt_ms: None,
            last_success_at: None,
            consecutive_failures: 0,
            reconnect_attempt: 0,
        }
    }
}

#[must_use]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ContactVerificationSnapshot {
    pub verified: bool,
    pub verified_at: Option<Timestamp>,
}

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairingInvitationView {
    pub session_id: PairingSessionId,
    pub code: PairingCode,
    pub uri: String,
    pub expires_at: Timestamp,
}
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NetworkSnapshot {
    pub tor: TorState,
    pub onion_address: Option<String>,
    pub peers: BTreeMap<ContactId, PeerConnectionStatus>,
    pub peer_health: BTreeMap<ContactId, PeerHealthSnapshot>,
    pub contact_names: BTreeMap<ContactId, String>,
    pub contact_verifications: BTreeMap<ContactId, ContactVerificationSnapshot>,
    pub peer_activity: BTreeMap<ContactId, TransportActivitySnapshot>,
    pub probes: Vec<ProbeResult>,
    pub connectivity: ConnectivitySnapshot,
    pub relay_info: Option<RelayServiceInfo>,
}

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelayServiceInfo {
    pub product_version: String,
    pub build_id: String,
    pub source_commit: String,
    pub protocol_version: u16,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeDriverError {
    Pairing,
    Communication,
    /// A stable, redacted descriptor preserved from a narrower application
    /// port.  The runtime must not erase the cause into `Communication`.
    Classified(ErrorDescriptor),
    Tor,
    Engine,
    Pending,
}
impl core::fmt::Display for RuntimeDriverError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for RuntimeDriverError {}

impl ClassifiedError for RuntimeDriverError {
    fn descriptor(&self) -> ErrorDescriptor {
        let (code, category, retry) = match self {
            Self::Pairing => {
                ("runtime.pairing_failed", ErrorCategory::Conflict, RetryAdvice::Never)
            }
            Self::Communication => (
                "runtime.communication_unavailable",
                ErrorCategory::Unavailable,
                RetryAdvice::Backoff,
            ),
            Self::Classified(descriptor) => return *descriptor,
            Self::Tor => {
                ("runtime.tor_unavailable", ErrorCategory::Unavailable, RetryAdvice::Backoff)
            }
            Self::Engine => ("runtime.engine_failed", ErrorCategory::Internal, RetryAdvice::Never),
            Self::Pending => {
                ("runtime.pending", ErrorCategory::Unavailable, RetryAdvice::Immediate)
            }
        };
        ErrorDescriptor::new(ErrorCode::new(code), category, retry)
    }
}
