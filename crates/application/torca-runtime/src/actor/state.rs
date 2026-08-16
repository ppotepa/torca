//! Runtime-owned mutable state grouped by lifecycle responsibility.

#[derive(Default)]
struct RuntimeHealthState {
    last_tor_state: Option<TorState>,
    last_onion_state: Option<OnionServiceState>,
    last_relay_state: Option<(ProbeStatus, ErrorCode)>,
    last_peer_states: BTreeMap<ContactId, PeerConnectionStatus>,
    last_peer_successes: BTreeMap<ContactId, Option<Timestamp>>,
    last_peer_activity: BTreeMap<ContactId, PeerActivityEvidence>,
    tor_failed: bool,
    pairing_failed: bool,
    communication_failed: bool,
    probes: ProbeSupervisor,
    peer_probes: PeerProbeSupervisor,
    transport_activity: TransportActivityLedger,
}

struct RuntimeWorkState {
    battery_policy: BatteryPolicy,
    metered_transfers: MeteredTransferPolicy,
    metered_network: bool,
    contacts: Vec<ContactId>,
    refresh_contacts: bool,
    attention_owner: Option<OpaqueId>,
    attention_generation: u64,
    active_attachment_leases: BTreeSet<OpaqueId>,
    active_delivery_leases: BTreeSet<OpaqueId>,
    bootstrap_relay_probe_started: bool,
    bootstrap_relay_probe_finished: bool,
}

impl RuntimeWorkState {
    fn new() -> Self {
        Self {
            battery_policy: BatteryPolicy::new(BatteryProfile::AlwaysAvailable),
            metered_transfers: MeteredTransferPolicy::PauseLarge,
            metered_network: false,
            contacts: Vec::new(),
            refresh_contacts: true,
            attention_owner: None,
            attention_generation: 0,
            active_attachment_leases: BTreeSet::new(),
            active_delivery_leases: BTreeSet::new(),
            bootstrap_relay_probe_started: false,
            bootstrap_relay_probe_finished: false,
        }
    }
}

#[derive(Default)]
struct RuntimeCounters {
    last_relay_probe_count: u64,
    last_worker_database_writes: u64,
    last_blob_writes: u64,
    last_attachment_chunks: u64,
    last_attachment_suppressed: u64,
    last_projection_events: u64,
}

struct RuntimeSchedulingState {
    next_maintenance_at: Option<std::time::Instant>,
    peer_probe_deadline: Option<Timestamp>,
}

impl RuntimeSchedulingState {
    fn new() -> Self {
        Self {
            next_maintenance_at: Some(std::time::Instant::now()),
            peer_probe_deadline: None,
        }
    }
}
