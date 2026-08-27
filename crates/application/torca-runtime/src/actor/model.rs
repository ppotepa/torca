// Responsibility: runtime-facing state snapshots and classified driver errors.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommunicationState {
    Stopped,
    Starting,
    Ready,
    Degraded,
    Failed,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IncomingReachabilityState {
    Unknown,
    Publishing,
    Reachable,
    Degraded,
    Failed,
    Stopped,
}

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
    pub availability: PeerAvailability,
    pub quality: PeerHealthQuality,
    pub rtt_ms: Option<u64>,
    pub last_success_at: Option<Timestamp>,
    pub consecutive_failures: u32,
    pub reconnect_attempt: u32,
}

#[must_use]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TransportActivitySnapshot {
    pub last_activity_at: Option<Timestamp>,
    pub sequence: u64,
}

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
            availability: if matches!(state, PeerConnectionStatus::Ready) {
                PeerAvailability::Reachable
            } else {
                PeerAvailability::Unknown
            },
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

/// Relationships made durable by one successful pairing maintenance turn.
/// Runtime consumers must prime only these contacts, after persistence has
/// completed, and may assume the list is deduplicated.
#[must_use]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PairingMaintenanceReport {
    pub completed_contacts: Vec<ContactId>,
}
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NetworkSnapshot {
    /// Provider-owned commissioning projection used by all new application
    /// policy and presentation. The Tor fields below are retained only while
    /// older native/Flutter contract consumers migrate.
    pub communication: torca_transport_api::ProviderCommissioning,
    /// Legacy compatibility state. The field name remains stable for one wire
    /// migration; its type is provider-neutral.
    pub tor: CommunicationState,
    pub peers: BTreeMap<ContactId, PeerConnectionStatus>,
    pub peer_health: BTreeMap<ContactId, PeerHealthSnapshot>,
    pub contact_names: BTreeMap<ContactId, String>,
    pub contact_verifications: BTreeMap<ContactId, ContactVerificationSnapshot>,
    pub peer_activity: BTreeMap<ContactId, TransportActivitySnapshot>,
    pub probes: Vec<ProbeResult>,
    pub connectivity: ConnectivitySnapshot,
    /// Provider-neutral metadata from the optional pairing rendezvous service.
    pub rendezvous_info: Option<RendezvousServiceInfo>,
    /// Legacy compatibility projection for existing bridge consumers.
    pub relay_info: Option<RelayServiceInfo>,
}

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RendezvousServiceInfo {
    pub product_version: String,
    pub build_id: String,
    pub source_commit: String,
    pub protocol_version: u16,
}

/// Compatibility alias for older bridge consumers. New provider-neutral code
/// must use [`RendezvousServiceInfo`].
pub type RelayServiceInfo = RendezvousServiceInfo;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeDriverError {
    Pairing,
    Communication,
    /// The selected provider is migrating its local route. Callers must wait
    /// for the provider route-refresh event instead of retrying a captured
    /// endpoint. This is deliberately provider-neutral (it also applies to
    /// future WebRTC or other address-changing transports).
    RouteRefreshRequired,
    Classified(ErrorDescriptor),
    Engine,
    Pending,
}
impl core::fmt::Display for RuntimeDriverError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for RuntimeDriverError {}

impl From<torca_client_engine::EngineError> for RuntimeDriverError {
    fn from(value: torca_client_engine::EngineError) -> Self {
        Self::Classified(value.descriptor())
    }
}

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
            Self::RouteRefreshRequired => (
                "runtime.route_refresh_required",
                ErrorCategory::Unavailable,
                RetryAdvice::Immediate,
            ),
            Self::Classified(descriptor) => return *descriptor,
            Self::Engine => ("runtime.engine_failed", ErrorCategory::Internal, RetryAdvice::Never),
            Self::Pending => {
                ("runtime.pending", ErrorCategory::Unavailable, RetryAdvice::Immediate)
            }
        };
        ErrorDescriptor::new(ErrorCode::new(code), category, retry)
    }
}
