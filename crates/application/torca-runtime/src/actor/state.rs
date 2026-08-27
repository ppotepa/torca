// Responsibility: runtime-owned mutable state grouped by lifecycle responsibility.

#[derive(Default)]
struct RuntimeHealthState {
    last_communication_state: Option<CommunicationState>,
    last_incoming_reachability_state: Option<IncomingReachabilityState>,
    last_provider_route_state: Option<torca_transport_api::ProviderRouteState>,
    last_relay_state: Option<(ProbeStatus, ErrorCode)>,
    last_peer_states: BTreeMap<ContactId, PeerConnectionStatus>,
    last_peer_successes: BTreeMap<ContactId, Option<Timestamp>>,
    last_peer_activity: BTreeMap<ContactId, PeerActivityEvidence>,
    communication_lifecycle_failed: bool,
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
    /// after background grace and only when no durable work owns the selected
    /// communication provider.
    communication_dormancy_allowed: bool,
    metered_transfers: MeteredTransferPolicy,
    metered_network: bool,
    /// Recipients reconstructed from the durable outbox at startup and after
    /// a delivery turn. Unlike `contacts`, this set is actual runtime demand.
    pending_delivery_contacts: BTreeSet<ContactId>,
    attention_owner: Option<OpaqueId>,
    attention_generation: u64,
    visible_contact_leases: BTreeMap<ContactId, OpaqueId>,
    active_attachment_leases: BTreeSet<OpaqueId>,
    /// Attachment-to-recipient routing for the worker's current durable jobs.
    active_attachment_contacts: BTreeMap<OpaqueId, ContactId>,
    /// Contacts with durable receipt/reaction/attachment-control outbox work.
    active_control_contacts: BTreeSet<ContactId>,
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
            communication_dormancy_allowed: false,
            metered_transfers: MeteredTransferPolicy::PauseLarge,
            metered_network: false,
            pending_delivery_contacts: BTreeSet::new(),
            attention_owner: None,
            attention_generation: 0,
            visible_contact_leases: BTreeMap::new(),
            active_attachment_leases: BTreeSet::new(),
            active_attachment_contacts: BTreeMap::new(),
            active_control_contacts: BTreeSet::new(),
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
    peer_recovery_delay: Option<std::time::Duration>,
    peer_recovery_started_at: Option<std::time::Instant>,
    peer_recovery_generation: u64,
    peer_recovery_attempts: u64,
    peer_recovery_exhausted: bool,
    last_deadline_delays: BTreeMap<RuntimeWakeSource, u64>,
    zero_delay_deadlines: u64,
    identical_deadline_replacements: u64,
}

impl RuntimeSchedulingState {
    fn new() -> Self {
        let now = std::time::Instant::now();
        let mut initial = BTreeSet::new();
        initial.extend([
            RuntimeWakeSource::ProviderDeadline,
            RuntimeWakeSource::PairingDeadline,
            RuntimeWakeSource::DeliveryDeadline,
            RuntimeWakeSource::RadioDeadline,
            RuntimeWakeSource::PeerDeadline,
        ]);
        Self {
            deadlines: BTreeMap::from([(now, initial)]),
            peer_probe_deadline: None,
            background_grace_deadline: None,
            peer_recovery_delay: None,
            peer_recovery_started_at: None,
            peer_recovery_generation: 0,
            peer_recovery_attempts: 0,
            peer_recovery_exhausted: false,
            last_deadline_delays: BTreeMap::new(),
            zero_delay_deadlines: 0,
            identical_deadline_replacements: 0,
        }
    }

    fn replace_deadlines(
        &mut self,
        now: std::time::Instant,
        candidates: impl IntoIterator<Item = (RuntimeWakeSource, Option<Duration>)>,
    ) {
        self.deadlines.clear();
        let mut present_sources = BTreeSet::new();
        for (source, delay) in candidates {
            if let Some(delay) = delay {
                present_sources.insert(source);
                let delay_ms = delay.as_millis().min(u128::from(u64::MAX)) as u64;
                if delay.is_zero() {
                    self.zero_delay_deadlines = self.zero_delay_deadlines.saturating_add(1);
                }
                if self.last_deadline_delays.get(&source).copied() == Some(delay_ms) {
                    self.identical_deadline_replacements =
                        self.identical_deadline_replacements.saturating_add(1);
                }
                self.last_deadline_delays.insert(source, delay_ms);
                self.deadlines.entry(now + delay).or_default().insert(source);
            }
        }
        self.last_deadline_delays.retain(|source, _| present_sources.contains(source));
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
            zero_delay_deadlines: self.zero_delay_deadlines,
            identical_deadline_replacements: self.identical_deadline_replacements,
            peer_recovery_generation: self.peer_recovery_generation,
            peer_recovery_attempts: self.peer_recovery_attempts,
            peer_recovery_exhausted: self.peer_recovery_exhausted,
        }
    }
}
