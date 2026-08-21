/// Application owns peer probe cadence and retry timing. The communication
/// adapter only validates eligibility, executes one bounded keepalive and
/// maintains its transport-health sample.
fn maintain_peer_probes<C: PeerSessionPort>(
    communication: &mut C,
    contacts: &[ContactId],
    supervisor: &mut PeerProbeSupervisor,
    policy: &mut RuntimeGovernor,
    battery_policy: BatteryPolicy,
    now: Timestamp,
) -> Result<(Option<Timestamp>, bool), RuntimeDriverError> {
    if matches!(battery_policy.profile(), BatteryProfile::BatterySaver) {
        // Cosmetic reachability probes are suppressed in Battery Saver. Real
        // traffic and durable delivery remain owned by their executors.
        supervisor.suspend();
        return Ok((None, false));
    }
    if let Some(contact_id) = communication.take_peer_probe_completion(now)? {
        let health = communication.peer_health(contact_id);
        supervisor.complete(
            contact_id.to_opaque(),
            health.consecutive_failures == 0 && health.last_success_at.is_some(),
            now,
        );
    }
    let candidates = contacts
        .iter()
        .copied()
        .map(|contact_id| {
            let health = communication.peer_health(contact_id);
            PeerProbeCandidate {
                peer_id: contact_id.to_opaque(),
                ready: health.state == PeerConnectionStatus::Ready,
                eligible: communication.peer_probe_eligible(contact_id)
                    && has_peer_or_radio_lease(policy, contact_id),
                freshness: policy.freshness(
                    ResourceScope::Peer(contact_id.to_opaque()),
                    std::time::Instant::now(),
                ),
                reported_rtt_ms: health.rtt_ms,
            }
        })
        .collect::<Vec<_>>();
    supervisor.reconcile(&candidates, now);
    let Some(request) = supervisor.next_due(&candidates, now) else {
        return Ok((supervisor.next_deadline(), false));
    };
    let Some(contact_id) =
        contacts.iter().copied().find(|contact_id| contact_id.to_opaque() == request.peer_id)
    else {
        supervisor.complete(request.peer_id, false, now);
        return Ok((supervisor.next_deadline(), false));
    };
    if let Err(error) =
        communication.begin_peer_probe(contact_id, request.probe_id, request.reported_rtt_ms)
    {
        supervisor.complete(request.peer_id, false, now);
        return Err(error);
    }
    Ok((supervisor.next_deadline(), true))
}

fn has_peer_or_radio_lease(policy: &mut RuntimeGovernor, contact_id: ContactId) -> bool {
    let now = std::time::Instant::now();
    policy.has_active_lease(ResourceScope::Peer(contact_id.to_opaque()), now)
        || policy.has_active_lease(ResourceScope::Radio(contact_id.to_opaque()), now)
}

const ATTACHMENT_OWNER_NAMESPACE: u128 = 0xA77A_C4A4_0000_0000_0000_0000_0000_0001;
const DELIVERY_OWNER_NAMESPACE: u128 = 0xD311_0000_0000_0000_0000_0000_0000_0001;
const PAIRING_OWNER_NAMESPACE: u128 = 0xA117_0000_0000_0000_0000_0000_0000_0001;
const RADIO_OWNER_NAMESPACE: u128 = 0xA1D1_0000_0000_0000_0000_0000_0000_0001;
const RADIO_TRANSMISSION_OWNER_NAMESPACE: u128 = 0xA1D2_0000_0000_0000_0000_0000_0000_0001;
const INSTANT_CONTACT_OWNER_NAMESPACE: u128 = 0x1A57_AA70_0000_0000_0000_0000_0000_0001;
const VISIBLE_CONTACT_OWNER_NAMESPACE: u128 = 0x5151_B1E0_0000_0000_0000_0000_0000_0001;
const BOOTSTRAP_RELAY_OWNER: u128 = 0xB007_57A4_0000_0000_0000_0000_0000_0001;

fn bootstrap_relay_lease_owner() -> OpaqueId {
    OpaqueId::from_u128(BOOTSTRAP_RELAY_OWNER)
}

fn acquire_bootstrap_relay_lease(policy: &mut RuntimeGovernor) {
    policy.acquire_lease(WorkDemand {
        scope: ResourceScope::Relay,
        class: WorkClass::RelayProbe,
        reason: DemandReason::BootstrapValidation,
        owner: bootstrap_relay_lease_owner(),
        // Two bounded attempts (8 s each) plus jittered retry fit well inside
        // this lease. Expiry remains a final safety net if a driver wedges.
        expires_at: std::time::Instant::now() + Duration::from_secs(45),
    });
}

fn delivery_lease_owner(message_id: OpaqueId) -> OpaqueId {
    OpaqueId::from_u128(message_id.to_u128() ^ DELIVERY_OWNER_NAMESPACE)
}

fn acquire_delivery_lease(policy: &mut RuntimeGovernor, message_id: OpaqueId) {
    policy.acquire_lease(WorkDemand {
        scope: ResourceScope::Delivery(message_id),
        class: WorkClass::Delivery,
        reason: DemandReason::PendingMessage,
        owner: delivery_lease_owner(message_id),
        expires_at: std::time::Instant::now() + Duration::from_secs(10 * 60),
    });
}

fn attachment_lease_owner(attachment_id: OpaqueId) -> OpaqueId {
    OpaqueId::from_u128(attachment_id.to_u128() ^ ATTACHMENT_OWNER_NAMESPACE)
}

fn pairing_lease_owner(session_id: PairingSessionId) -> OpaqueId {
    OpaqueId::from_u128(session_id.to_opaque().to_u128() ^ PAIRING_OWNER_NAMESPACE)
}

fn radio_lease_owner(contact_id: ContactId) -> OpaqueId {
    OpaqueId::from_u128(contact_id.to_opaque().to_u128() ^ RADIO_OWNER_NAMESPACE)
}

fn radio_transmission_lease_owner(contact_id: ContactId) -> OpaqueId {
    OpaqueId::from_u128(contact_id.to_opaque().to_u128() ^ RADIO_TRANSMISSION_OWNER_NAMESPACE)
}

fn instant_contact_lease_owner(contact_id: ContactId) -> OpaqueId {
    OpaqueId::from_u128(contact_id.to_opaque().to_u128() ^ INSTANT_CONTACT_OWNER_NAMESPACE)
}

fn visible_contact_lease_owner(contact_id: ContactId, generation: u64) -> OpaqueId {
    OpaqueId::from_u128(
        contact_id.to_opaque().to_u128()
            ^ VISIBLE_CONTACT_OWNER_NAMESPACE
            ^ u128::from(generation),
    )
}

fn acquire_visible_contact_lease(
    policy: &mut RuntimeGovernor,
    contact_id: ContactId,
    owner: OpaqueId,
) {
    policy.acquire_persistent_lease(WorkDemand {
        scope: ResourceScope::Peer(contact_id.to_opaque()),
        class: WorkClass::PeerProbe,
        reason: DemandReason::VisibleContact,
        owner,
        // UI attention is released explicitly on navigation/background, so
        // it must not manufacture a synthetic expiry wake.
        expires_at: std::time::Instant::now(),
    });
}

fn release_attention_leases(policy: &mut RuntimeGovernor, work: &mut RuntimeWorkState) {
    if let Some(owner) = work.attention_owner.take() {
        policy.release_lease(owner);
    }
    for owner in work.visible_contact_leases.values().copied() {
        policy.release_lease(owner);
    }
    work.visible_contact_leases.clear();
}

fn acquire_instant_contact_lease(policy: &mut RuntimeGovernor, contact_id: ContactId) {
    policy.acquire_persistent_lease(WorkDemand {
        scope: ResourceScope::Peer(contact_id.to_opaque()),
        class: WorkClass::PeerDial,
        reason: DemandReason::InstantContact,
        owner: instant_contact_lease_owner(contact_id),
        expires_at: std::time::Instant::now(),
    });
}

fn acquire_radio_lease(policy: &mut RuntimeGovernor, contact_id: ContactId) {
    policy.acquire_persistent_lease(WorkDemand {
        scope: ResourceScope::Radio(contact_id.to_opaque()),
        class: WorkClass::Radio,
        reason: DemandReason::RadioSession,
        owner: radio_lease_owner(contact_id),
        // Persistent leases have an explicit release on radio disable. The
        // timestamp is retained for the common demand shape but is not used
        // as a synthetic renewal timer.
        expires_at: std::time::Instant::now(),
    });
}

fn acquire_radio_transmission_lease(policy: &mut RuntimeGovernor, contact_id: ContactId) {
    policy.acquire_lease(WorkDemand {
        scope: ResourceScope::Radio(contact_id.to_opaque()),
        class: WorkClass::Radio,
        reason: DemandReason::RadioSession,
        owner: radio_transmission_lease_owner(contact_id),
        expires_at: std::time::Instant::now() + Duration::from_secs(30),
    });
}

fn acquire_pairing_lease(policy: &mut RuntimeGovernor, session_id: PairingSessionId) {
    policy.acquire_lease(WorkDemand {
        scope: ResourceScope::Relay,
        class: WorkClass::RelayConnect,
        reason: DemandReason::ActivePairing,
        owner: pairing_lease_owner(session_id),
        expires_at: std::time::Instant::now() + Duration::from_secs(5 * 60),
    });
}

fn acquire_attachment_lease(
    policy: &mut RuntimeGovernor,
    active_attachment_leases: &mut BTreeSet<OpaqueId>,
    attachment_id: OpaqueId,
) {
    active_attachment_leases.insert(attachment_id);
    policy.acquire_lease(WorkDemand {
        scope: ResourceScope::Attachment(attachment_id),
        class: WorkClass::Attachment,
        reason: DemandReason::AttachmentTransfer,
        owner: attachment_lease_owner(attachment_id),
        expires_at: std::time::Instant::now() + Duration::from_secs(10 * 60),
    });
}

fn observe_maintenance(
    result: Result<(), RuntimeDriverError>,
    failed: &mut bool,
    diagnostics: &mut DiagnosticBuffer,
    sequence: &mut u128,
    now: Timestamp,
    component: Component,
    failure_code: &str,
    recovery_code: &str,
) {
    match (result, *failed) {
        (Err(_), false) => {
            *failed = true;
            record(diagnostics, sequence, now, component, HealthState::Failed, failure_code);
        }
        (Ok(()), true) => {
            *failed = false;
            record(diagnostics, sequence, now, component, HealthState::Ready, recovery_code);
        }
        _ => {}
    }
}

fn record_runtime_probes(
    probes: &mut ProbeSupervisor,
    tor_state: TorState,
    onion_state: OnionServiceState,
    peer_failed: bool,
    relay_result: ProbeResult,
    now: Timestamp,
) {
    for target in [
        ProbeTarget::NativeBridge,
        ProbeTarget::SecureStorage,
        ProbeTarget::Database,
        ProbeTarget::Engine,
    ] {
        probes.record(runtime_probe(target, ProbeKind::Readiness, ProbeStatus::Healthy, "OK", now));
    }
    probes.record(runtime_probe(
        ProbeTarget::Tor,
        ProbeKind::Readiness,
        match tor_state {
            TorState::Ready => ProbeStatus::Healthy,
            TorState::Starting => ProbeStatus::Checking,
            TorState::Degraded => ProbeStatus::Degraded,
            TorState::Failed => ProbeStatus::Failed,
            TorState::Stopped => ProbeStatus::Unknown,
        },
        if matches!(tor_state, TorState::Ready) { "TOR_READY" } else { "TOR_NOT_READY" },
        now,
    ));
    probes.record(relay_result);
    probes.record(runtime_probe(
        ProbeTarget::OnionService,
        ProbeKind::Readiness,
        match onion_state {
            OnionServiceState::Reachable => ProbeStatus::Healthy,
            OnionServiceState::Publishing => ProbeStatus::Checking,
            OnionServiceState::Degraded => ProbeStatus::Degraded,
            OnionServiceState::Failed => ProbeStatus::Failed,
            OnionServiceState::Unknown | OnionServiceState::Stopped => ProbeStatus::Unknown,
        },
        match onion_state {
            OnionServiceState::Reachable => "ONION_REACHABLE",
            OnionServiceState::Publishing => "ONION_PUBLISHING",
            OnionServiceState::Degraded => "ONION_DEGRADED",
            OnionServiceState::Failed => "ONION_FAILED",
            OnionServiceState::Unknown | OnionServiceState::Stopped => "ONION_UNAVAILABLE",
        },
        now,
    ));
    probes.record(runtime_probe(
        ProbeTarget::Peer,
        ProbeKind::Connectivity,
        if peer_failed { ProbeStatus::Degraded } else { ProbeStatus::Healthy },
        if peer_failed { "PEER_MAINTENANCE_FAILED" } else { "PEER_MAINTENANCE_READY" },
        now,
    ));
}

fn runtime_probe(
    target: ProbeTarget,
    kind: ProbeKind,
    status: ProbeStatus,
    diagnostic_code: &str,
    measured_at: Timestamp,
) -> ProbeResult {
    ProbeResult {
        target,
        kind,
        status,
        diagnostic_code: diagnostic_code.into(),
        latency_ms: None,
        measured_at,
    }
}

fn relay_probe_result(snapshot: RelayHealthSnapshot, measured_at: Timestamp) -> ProbeResult {
    ProbeResult {
        target: ProbeTarget::Relay,
        kind: ProbeKind::Connectivity,
        status: snapshot.status,
        diagnostic_code: snapshot.diagnostic_code.to_string(),
        latency_ms: snapshot.latency_ms,
        measured_at,
    }
}
