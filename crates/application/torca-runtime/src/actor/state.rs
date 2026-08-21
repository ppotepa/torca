// Responsibility: runtime-owned mutable state grouped by lifecycle responsibility.

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

// These flags are independent policy inputs (foreground, metering and two
// one-shot bootstrap observations); combining them into an enum would create
// invalid coupling between unrelated lifecycle dimensions.
#[allow(clippy::struct_excessive_bools)]
struct RuntimeWorkState {
    battery_policy: BatteryPolicy,
    battery_preferences: BatteryPreferences,
    system_energy: SystemEnergyState,
    foreground: bool,
    /// A policy permission, not an imperative. RuntimeOwner applies it only
    /// after background grace and only when no durable work owns Tor.
    tor_dormancy_allowed: bool,
    metered_transfers: MeteredTransferPolicy,
    metered_network: bool,
    contacts: Vec<ContactId>,
    refresh_contacts: bool,
    /// Recipients reconstructed from the durable outbox at startup and after
    /// a delivery turn. Unlike `contacts`, this set is actual runtime demand.
    pending_delivery_contacts: BTreeSet<ContactId>,
    attention_owner: Option<OpaqueId>,
    attention_generation: u64,
    visible_contact_leases: BTreeMap<ContactId, OpaqueId>,
    active_attachment_leases: BTreeSet<OpaqueId>,
    /// Attachment-to-recipient routing for the worker's current durable jobs.
    active_attachment_contacts: BTreeMap<OpaqueId, ContactId>,
    active_delivery_leases: BTreeSet<OpaqueId>,
    /// Recipient routing for live durable delivery. This is ephemeral: the
    /// durable message/outbox remains authoritative after a process restart.
    active_delivery_contacts: BTreeMap<OpaqueId, ContactId>,
    bootstrap_relay_probe_started: bool,
    bootstrap_relay_probe_finished: bool,
}

impl RuntimeWorkState {
    fn new() -> Self {
        Self {
            // The host applies the persisted effective policy during startup.
            // Until that event arrives, choose the safe non-aggressive
            // default so a standalone RuntimeOwner cannot accidentally make
            // cosmetic probes always available.
            battery_policy: BatteryPolicy::new(BatteryProfile::Balanced),
            battery_preferences: BatteryPreferences::default(),
            system_energy: SystemEnergyState::default().with_foreground(true),
            foreground: true,
            tor_dormancy_allowed: false,
            metered_transfers: MeteredTransferPolicy::PauseLarge,
            metered_network: false,
            contacts: Vec::new(),
            refresh_contacts: false,
            pending_delivery_contacts: BTreeSet::new(),
            attention_owner: None,
            attention_generation: 0,
            visible_contact_leases: BTreeMap::new(),
            active_attachment_leases: BTreeSet::new(),
            active_attachment_contacts: BTreeMap::new(),
            active_delivery_leases: BTreeSet::new(),
            active_delivery_contacts: BTreeMap::new(),
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
    deadlines: BTreeMap<std::time::Instant, BTreeSet<RuntimeWakeSource>>,
    peer_probe_deadline: Option<Timestamp>,
    background_grace_deadline: Option<std::time::Instant>,
}

impl RuntimeSchedulingState {
    fn new() -> Self {
        let now = std::time::Instant::now();
        let mut initial = BTreeSet::new();
        initial.extend([
            RuntimeWakeSource::TorDeadline,
            RuntimeWakeSource::PairingDeadline,
            RuntimeWakeSource::DeliveryDeadline,
            RuntimeWakeSource::RadioDeadline,
            RuntimeWakeSource::PeerDeadline,
        ]);
        Self {
            deadlines: BTreeMap::from([(now, initial)]),
            peer_probe_deadline: None,
            background_grace_deadline: None,
        }
    }

    fn replace_deadlines(
        &mut self,
        now: std::time::Instant,
        candidates: impl IntoIterator<Item = (RuntimeWakeSource, Option<Duration>)>,
    ) {
        self.deadlines.clear();
        for (source, delay) in candidates {
            if let Some(delay) = delay {
                self.deadlines.entry(now + delay).or_default().insert(source);
            }
        }
    }

    fn next_deadline(&self) -> Option<std::time::Instant> {
        self.deadlines.keys().next().copied()
    }

    fn take_due(&mut self, now: std::time::Instant) -> BTreeSet<RuntimeWakeSource> {
        let due = self.deadlines.range(..=now).map(|(deadline, _)| *deadline).collect::<Vec<_>>();
        let mut sources = BTreeSet::new();
        for deadline in due {
            if let Some(items) = self.deadlines.remove(&deadline) {
                sources.extend(items);
            }
        }
        sources
    }

    fn diagnostic_snapshot(
        &self,
        now: std::time::Instant,
    ) -> torca_diagnostics::RuntimeScheduleSnapshot {
        let mut sources = BTreeMap::new();
        for wake_sources in self.deadlines.values() {
            for source in wake_sources {
                *sources.entry(*source).or_default() += 1;
            }
        }
        torca_diagnostics::RuntimeScheduleSnapshot {
            active_deadlines: self.deadlines.len() as u64,
            next_deadline_in_ms: self
                .next_deadline()
                .map(|deadline| deadline.saturating_duration_since(now).as_millis() as u64),
            sources,
        }
    }
}
