// Responsibility: one runtime maintenance turn split into explicit phases.

fn maintain_runtime_health<
    P: PairingDriver,
    C: CommunicationDriver,
    T: CommunicationLifecycle,
>(
    pairing: &mut P,
    communication: &mut C,
    communication_lifecycle: &mut T,
    rendezvous_health: Option<&PairingServiceHealthHandle>,
    policy: &mut RuntimeGovernor,
    health: &mut RuntimeHealthState,
    work: &mut RuntimeWorkState,
    counters: &mut RuntimeCounters,
    diagnostics: &mut DiagnosticBuffer,
    sequence: &mut u128,
    connectivity: &ConnectivityObserver,
    now: Timestamp,
) {
    if let Some(rendezvous) = rendezvous_health {
        let demanded = policy.has_active_lease(ResourceScope::Rendezvous, std::time::Instant::now());
        rendezvous.set_demand(demanded);
    }

    let relay_snapshot = rendezvous_health
        .map_or_else(PairingServiceHealthSnapshot::default, PairingServiceHealthHandle::snapshot);
    let relay_probe_completed = relay_snapshot.probe_count > counters.last_relay_probe_count;
    if relay_probe_completed {
        diagnostics.count_by(
            RuntimeCounter::RendezvousProbe,
            relay_snapshot.probe_count.saturating_sub(counters.last_relay_probe_count),
        );
        counters.last_relay_probe_count = relay_snapshot.probe_count;
    }
    let relay_state = (relay_snapshot.status, relay_snapshot.diagnostic_code);
    if relay_probe_completed && relay_snapshot.status == ProbeStatus::Healthy {
        policy.apply(
            PolicyEvent::Evidence {
                scope: ResourceScope::Rendezvous,
                kind: EvidenceKind::Probe,
            },
            std::time::Instant::now(),
        );
    }
    if relay_probe_completed
        && work.bootstrap_relay_probe_started
        && matches!(
            relay_snapshot.status,
            ProbeStatus::Healthy
                | ProbeStatus::Degraded
                | ProbeStatus::Failed
                | ProbeStatus::Unreachable
        )
    {
        policy.release_lease(bootstrap_relay_lease_owner());
        work.bootstrap_relay_probe_started = false;
        work.bootstrap_relay_probe_finished = true;
    }
    if health.last_relay_state.as_ref() != Some(&relay_state) {
        record(
            diagnostics,
            sequence,
            now,
            Component::Relay,
            map_probe_health(relay_snapshot.status),
            relay_event_code(relay_snapshot.status),
        );
        health.last_relay_state = Some(relay_state);
    }

    observe_maintenance(
        communication_lifecycle.maintenance(now),
        &mut health.communication_lifecycle_failed,
        diagnostics,
        sequence,
        now,
        Component::Communication,
        "COMMUNICATION_MAINTENANCE_FAILED",
        "COMMUNICATION_MAINTENANCE_RECOVERED",
    );
    let provider_route_state = communication_lifecycle.runtime_diagnostics().route_state;
    if provider_route_state != health.last_provider_route_state {
        if let Some(route_state) = provider_route_state {
            let (state, code) = match route_state {
                torca_transport_api::ProviderRouteState::Fresh
                    if health.last_provider_route_state
                        == Some(torca_transport_api::ProviderRouteState::Stale) =>
                {
                    (HealthState::Ready, "PEER_ROUTE_REFRESHED")
                }
                torca_transport_api::ProviderRouteState::Fresh => {
                    (HealthState::Ready, "PEER_ROUTE_FRESH")
                }
                torca_transport_api::ProviderRouteState::Stale => {
                    (HealthState::Degraded, "PEER_ROUTE_STALE")
                }
                torca_transport_api::ProviderRouteState::Unavailable => {
                    (HealthState::Starting, "PEER_ROUTE_UNAVAILABLE")
                }
            };
            record(diagnostics, sequence, now, Component::Communication, state, code);
        }
        health.last_provider_route_state = provider_route_state;
    }
    let pairing_maintenance = pairing.maintenance(now);
    if let Ok(report) = &pairing_maintenance {
        prime_completed_pairings(report, |contact_id| {
            record(
                diagnostics,
                sequence,
                now,
                Component::Storage,
                HealthState::Ready,
                "PAIRING_CONTACT_PERSISTED",
            );
            communication.prime_contact(contact_id);
            record(
                diagnostics,
                sequence,
                now,
                Component::Peer,
                HealthState::Starting,
                "PEER_PRIME_REQUESTED",
            );
        });
    }
    observe_maintenance(
        pairing_maintenance.map(|_| ()),
        &mut health.pairing_failed,
        diagnostics,
        sequence,
        now,
        // Pairing is implemented by the selected communication provider.
        // It is not inherently a relay operation (Iroh and WebRTC use direct
        // or externally signalled routes), so keep provider-specific relay
        // terminology out of generic runtime diagnostics.
        Component::Communication,
        "PAIRING_MAINTENANCE_FAILED",
        "PAIRING_MAINTENANCE_RECOVERED",
    );

    let communication_state = communication_lifecycle.state();
    let incoming_reachability_state = communication_lifecycle.incoming_reachability_state();
    if rendezvous_health.is_some()
        && communication_state == CommunicationState::Ready
        && !work.bootstrap_relay_probe_started
        && !work.bootstrap_relay_probe_finished
    {
        acquire_bootstrap_relay_lease(policy);
        if let Some(rendezvous) = rendezvous_health {
            rendezvous.set_demand(true);
        }
        work.bootstrap_relay_probe_started = true;
    }
    record_runtime_probes(
        &mut health.probes,
        communication_state,
        incoming_reachability_state,
        health.communication_failed,
        relay_probe_result(relay_snapshot, now),
        now,
    );
    for probe in health.probes.latest() {
        connectivity.record_probe(&probe);
    }
    if health.last_communication_state != Some(communication_state) {
        health.last_communication_state = Some(communication_state);
        record(
            diagnostics,
            sequence,
            now,
            Component::Communication,
            map_communication_health(communication_state),
            "COMMUNICATION_STATE_CHANGED",
        );
    }
    if health.last_incoming_reachability_state != Some(incoming_reachability_state) {
        health.last_incoming_reachability_state = Some(incoming_reachability_state);
        record(
            diagnostics,
            sequence,
            now,
            Component::Communication,
            map_incoming_reachability_health(incoming_reachability_state),
            incoming_reachability_event_code(incoming_reachability_state),
        );
    }
}

fn prime_completed_pairings(
    report: &PairingMaintenanceReport,
    mut prime_contact: impl FnMut(ContactId),
) {
    for contact_id in report.completed_contacts.iter().copied().collect::<BTreeSet<_>>() {
        prime_contact(contact_id);
    }
}

fn maintain_delivery_state<C: CommunicationDriver>(
    engine: &EngineHandle,
    communication: &mut C,
    policy: &mut RuntimeGovernor,
    work: &mut RuntimeWorkState,
    counters: &mut RuntimeCounters,
    diagnostics: &mut DiagnosticBuffer,
    now: Timestamp,
) -> Result<(), RuntimeDriverError> {
    // The regular path contains only recipients with durable pending work.
    // A known contact never becomes maintenance work by itself.
    let scoped_delivery_contacts = work
        .active_delivery_contacts
        .values()
        .copied()
        .chain(work.active_attachment_contacts.values().copied())
        .chain(work.active_control_contacts.iter().copied())
        .chain(work.pending_delivery_contacts.iter().copied())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let delivery_contacts = scoped_delivery_contacts.as_slice();
    let maintenance_result = communication.maintenance(delivery_contacts, now);

    // Attachment preparation runs on a worker because it performs file I/O
    // and crypto. Reconcile failed preparations here: the companion message
    // was created before the worker started, so leaving it in the outbox would
    // make the UI show a transfer forever and retain its runtime lease.
    for (attachment_id, message_id) in communication.take_attachment_prepare_failures() {
        let _ = engine.dispatch(torca_client_engine::EngineCommand::CancelMessage {
            message_id: torca_messaging::MessageId::from_opaque(message_id),
            at: now,
        });
        work.active_attachment_leases.remove(&attachment_id);
        work.active_attachment_contacts.remove(&attachment_id);
        policy.release_lease(attachment_lease_owner(attachment_id));
    }
    work.active_control_contacts = communication.active_control_contacts().into_iter().collect();
    if work.active_attachment_leases.is_empty() && work.active_delivery_leases.is_empty() {
        let retained = policy
            .active_peer_ids(std::time::Instant::now())
            .into_iter()
            .map(ContactId::from_opaque)
            .collect::<Vec<_>>();
        let _ = communication.close_idle_peers(&retained, now);
    }

    let worker_database_writes = communication.database_write_count();
    if worker_database_writes > counters.last_worker_database_writes {
        diagnostics.count_by(
            RuntimeCounter::DbWrite,
            worker_database_writes.saturating_sub(counters.last_worker_database_writes),
        );
    }
    counters.last_worker_database_writes = worker_database_writes;

    let blob_writes = communication.blob_write_count();
    if blob_writes > counters.last_blob_writes {
        diagnostics.count_by(
            RuntimeCounter::BlobWrite,
            blob_writes.saturating_sub(counters.last_blob_writes),
        );
    }
    counters.last_blob_writes = blob_writes;

    let attachment_chunks = communication.attachment_chunk_tx_count();
    if attachment_chunks > counters.last_attachment_chunks {
        diagnostics.record_battery(
            BatteryMetric::AttachmentChunkTx,
            attachment_chunks - counters.last_attachment_chunks,
            WakeReason::AttachmentTransfer,
        );
    }
    counters.last_attachment_chunks = attachment_chunks;

    let suppressed = communication.attachment_policy_suppressed_count();
    if suppressed > counters.last_attachment_suppressed {
        diagnostics.record_battery(
            BatteryMetric::SuppressedWork,
            suppressed - counters.last_attachment_suppressed,
            WakeReason::PolicySuppressed,
        );
    }
    counters.last_attachment_suppressed = suppressed;

    let projection_events = engine.projection_event_count();
    let projection_changed = projection_events > counters.last_projection_events;
    if projection_changed {
        diagnostics.count_by(
            RuntimeCounter::ProjectionEvent,
            projection_events.saturating_sub(counters.last_projection_events),
        );
    }
    counters.last_projection_events = projection_events;

    work.pending_delivery_contacts =
        engine.pending_delivery_contacts().unwrap_or_default().into_iter().collect();

    if !work.active_attachment_leases.is_empty()
        && let Ok(views) = communication.attachment_snapshot()
    {
        for view in views {
            if matches!(view.status.as_str(), "available" | "cancelled") {
                work.active_attachment_leases.remove(&view.id);
                work.active_attachment_contacts.remove(&view.id);
                policy.release_lease(attachment_lease_owner(view.id));
            } else {
                acquire_attachment_lease(policy, &mut work.active_attachment_leases, view.id);
            }
        }
    }
    if projection_changed && !work.active_delivery_leases.is_empty() {
        for message_id in work.active_delivery_leases.iter().copied().collect::<Vec<_>>() {
            let message_key = torca_messaging::MessageId::from_opaque(message_id);
            if let Ok(Some(status)) = engine.message_status(message_key)
                && matches!(
                    status,
                    MessageStatus::Delivered
                        | MessageStatus::Read
                        | MessageStatus::Failed
                        | MessageStatus::Cancelled
                )
            {
                work.active_delivery_leases.remove(&message_id);
                work.active_delivery_contacts.remove(&message_id);
                policy.release_lease(delivery_lease_owner(message_id));
            }
        }
    }

    maintenance_result
}

fn maintain_peer_state<C: CommunicationDriver>(
    communication: &mut C,
    policy: &mut RuntimeGovernor,
    health: &mut RuntimeHealthState,
    battery_policy: BatteryPolicy,
    scheduling: &mut RuntimeSchedulingState,
    diagnostics: &mut DiagnosticBuffer,
    sequence: &mut u128,
    connectivity: &ConnectivityObserver,
    now: Timestamp,
    mut maintenance_result: Result<(), RuntimeDriverError>,
) -> bool {
    let mut current = BTreeMap::new();
    let mut current_successes = BTreeMap::new();
    let activity = CommunicationDriver::peer_activity(communication)
        .into_iter()
        .map(|evidence| (evidence.contact_id, evidence))
        .collect::<BTreeMap<_, _>>();
    // Known contacts are durable data, not runtime work. Only an active
    // lease, an existing live transport state, or real transport evidence can
    // place a peer on this turn's observation set.
    let observed_contacts = policy
        .active_peer_ids(std::time::Instant::now())
        .into_iter()
        .map(ContactId::from_opaque)
        .chain(health.last_peer_states.iter().filter_map(|(contact_id, state)| {
            matches!(
                state,
                PeerConnectionStatus::Ready
                    | PeerConnectionStatus::Connecting
                    | PeerConnectionStatus::Handshaking
                    | PeerConnectionStatus::Reconnecting
            )
            .then_some(*contact_id)
        }))
        .chain(activity.keys().copied())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut current_activity = BTreeMap::new();

    for id in observed_contacts.iter().copied() {
        let state = communication.connection_state(id);
        let previous_state = health.last_peer_states.get(&id).copied();
        if health.last_peer_states.get(&id) != Some(&state) {
            health.transport_activity.mark_peer(id, now);
            let diagnostic_code = match state {
                PeerConnectionStatus::Connecting => "PEER_DIAL_STARTED",
                PeerConnectionStatus::Handshaking => "PEER_HANDSHAKING",
                PeerConnectionStatus::Ready => "PEER_READY",
                PeerConnectionStatus::Failed => "PEER_DIAL_FAILED",
                PeerConnectionStatus::Reconnecting => "PEER_RECONNECTING",
                PeerConnectionStatus::Disconnected => "PEER_DISCONNECTED",
            };
            record(
                diagnostics,
                sequence,
                now,
                Component::Peer,
                map_peer_health(state),
                diagnostic_code,
            );
        }
        if state == PeerConnectionStatus::Ready
            && previous_state != Some(PeerConnectionStatus::Ready)
        {
            policy.apply(
                PolicyEvent::Evidence {
                    scope: ResourceScope::Peer(id.to_opaque()),
                    kind: EvidenceKind::Handshake,
                },
                std::time::Instant::now(),
            );
        } else if matches!(
            state,
            PeerConnectionStatus::Disconnected
                | PeerConnectionStatus::Reconnecting
                | PeerConnectionStatus::Failed
        ) && previous_state == Some(PeerConnectionStatus::Ready)
        {
            policy.apply(
                PolicyEvent::Evidence {
                    scope: ResourceScope::Peer(id.to_opaque()),
                    kind: EvidenceKind::Failure,
                },
                std::time::Instant::now(),
            );
        }

        let peer_health = communication.peer_health(id);
        if peer_health.last_success_at.is_some()
            && health.last_peer_successes.get(&id) != Some(&peer_health.last_success_at)
        {
            health.transport_activity.mark_peer(id, now);
            policy.apply(
                PolicyEvent::Evidence {
                    scope: ResourceScope::Peer(id.to_opaque()),
                    kind: EvidenceKind::Ack,
                },
                std::time::Instant::now(),
            );
        }

        if let Some(evidence) = activity.get(&id).copied() {
            let previous = health.last_peer_activity.get(&id).copied();
            let tx_changed = evidence.tx_frames > previous.map_or(0, |value| value.tx_frames);
            let rx_changed = evidence.rx_frames > previous.map_or(0, |value| value.rx_frames);
            let ack_changed = evidence.tx_acks > previous.map_or(0, |value| value.tx_acks)
                || evidence.rx_acks > previous.map_or(0, |value| value.rx_acks);
            let handshake_changed =
                evidence.handshakes > previous.map_or(0, |value| value.handshakes);
            let failure_changed = evidence.failures > previous.map_or(0, |value| value.failures);
            let tx_delta =
                evidence.tx_frames.saturating_sub(previous.map_or(0, |value| value.tx_frames));
            let rx_delta =
                evidence.rx_frames.saturating_sub(previous.map_or(0, |value| value.rx_frames));
            let handshake_delta =
                evidence.handshakes.saturating_sub(previous.map_or(0, |value| value.handshakes));

            if tx_delta > 0 {
                diagnostics.record_battery(
                    BatteryMetric::TxFrame,
                    tx_delta,
                    WakeReason::DurableDelivery,
                );
            }
            if rx_delta > 0 {
                diagnostics.record_battery(
                    BatteryMetric::RxFrame,
                    rx_delta,
                    WakeReason::DurableDelivery,
                );
            }
            if handshake_delta > 0 {
                diagnostics.record_battery(
                    BatteryMetric::Handshake,
                    handshake_delta,
                    WakeReason::Scheduler,
                );
            }
            if tx_changed || rx_changed || ack_changed || handshake_changed {
                health.transport_activity.mark_peer(id, now);
            }

            let scope = ResourceScope::Peer(id.to_opaque());
            let policy_now = std::time::Instant::now();
            for (changed, kind) in [
                (tx_changed, EvidenceKind::Tx),
                (rx_changed, EvidenceKind::Rx),
                (ack_changed, EvidenceKind::Ack),
                (handshake_changed, EvidenceKind::Handshake),
                (failure_changed, EvidenceKind::Failure),
            ] {
                if changed {
                    policy.apply(PolicyEvent::Evidence { scope, kind }, policy_now);
                }
            }
            if tx_changed || rx_changed {
                record(
                    diagnostics,
                    sequence,
                    now,
                    Component::Peer,
                    HealthState::Ready,
                    "MESSAGE_PEER_STAGE_COMPLETED",
                );
            }
            if ack_changed {
                record(
                    diagnostics,
                    sequence,
                    now,
                    Component::Peer,
                    HealthState::Ready,
                    "RECEIPT_PEER_STAGE_COMPLETED",
                );
            }
            current_activity.insert(id, evidence);
        }

        current.insert(id, state);
        connectivity.set_peer_ready(id.to_opaque(), state == PeerConnectionStatus::Ready);
        current_successes.insert(id, peer_health.last_success_at);
    }

    let active_transport = current.values().any(|state| {
        matches!(
            state,
            PeerConnectionStatus::Connecting
                | PeerConnectionStatus::Handshaking
                | PeerConnectionStatus::Reconnecting
        )
    });
    health.last_peer_states = current;
    health.last_peer_successes = current_successes;
    health.last_peer_activity = current_activity;

    if maintenance_result.is_ok() && observed_contacts.is_empty() {
        // A probe supervisor may retain a due timestamp after the last peer
        // was released.  Do not carry that stale deadline into the central
        // scheduler: it would create a zero-delay wake spin with no work.
        scheduling.peer_probe_deadline = None;
    } else if maintenance_result.is_ok() {
        maintenance_result = maintain_peer_probes(
            communication,
            &observed_contacts,
            &mut health.peer_probes,
            policy,
            battery_policy,
            now,
        )
        .map(|(deadline, probe_started)| {
            scheduling.peer_probe_deadline = deadline;
            if probe_started {
                diagnostics.count(RuntimeCounter::PeerProbe);
            }
        });
    }
    observe_maintenance(
        maintenance_result,
        &mut health.communication_failed,
        diagnostics,
        sequence,
        now,
        Component::Peer,
        "COMMUNICATION_MAINTENANCE_FAILED",
        "COMMUNICATION_MAINTENANCE_RECOVERED",
    );

    active_transport
}

fn update_runtime_schedule<P: PairingDriver, C: CommunicationDriver, T: CommunicationLifecycle>(
    communication_lifecycle: &T,
    pairing: &P,
    communication: &C,
    policy: &mut RuntimeGovernor,
    scheduling: &mut RuntimeSchedulingState,
    diagnostics: &mut DiagnosticBuffer,
    active_transport: bool,
    now: Timestamp,
) {
    let background_delay = scheduling
        .background_grace_deadline
        .map(|deadline| deadline.saturating_duration_since(std::time::Instant::now()));
    let lease_delay = policy
        .next_lease_expiry()
        .map(|expiry| expiry.saturating_duration_since(std::time::Instant::now()));
    // A handshaking/connecting peer must keep a bounded polling deadline even
    // when its transport wake raced with session creation.  Relying solely on
    // the transport callback can strand a session forever if the first ACK
    // arrives before the callback is installed.  Ready peers remain fully
    // event-driven; only active connection establishment gets this cheap
    // recovery tick.
    let peer_delay = peer_recovery_deadline(scheduling, active_transport, std::time::Instant::now())
        .or_else(|| scheduling.peer_probe_deadline.and_then(|deadline| deadline.duration_since(now)));
    scheduling.replace_deadlines(
        std::time::Instant::now(),
        [
            (
                RuntimeWakeSource::ProviderDeadline,
                communication_lifecycle.next_maintenance_delay(now),
            ),
            (RuntimeWakeSource::PairingDeadline, pairing.next_maintenance_delay(now)),
            (RuntimeWakeSource::DeliveryDeadline, communication.next_maintenance_delay(now)),
            (RuntimeWakeSource::RadioDeadline, communication.next_radio_maintenance_delay(now)),
            (RuntimeWakeSource::LeaseExpiry, lease_delay),
            (RuntimeWakeSource::PeerDeadline, peer_delay),
            (RuntimeWakeSource::BackgroundGrace, background_delay),
        ],
    );
    diagnostics.set_policy_snapshot(policy.snapshot(std::time::Instant::now()));
    diagnostics.set_runtime_schedule(scheduling.diagnostic_snapshot(std::time::Instant::now()));
}

fn next_peer_recovery_delay(delay: Duration) -> Duration {
    (delay * 2).min(Duration::from_secs(5))
}

const PEER_RECOVERY_WINDOW: Duration = Duration::from_secs(30);

/// Schedules only a bounded safety-net for a connection callback that was
/// lost. Once the window expires, a permanently stuck transport must wait for
/// a real provider/network event instead of waking the runtime forever.
fn peer_recovery_deadline(
    scheduling: &mut RuntimeSchedulingState,
    active_transport: bool,
    now: std::time::Instant,
) -> Option<Duration> {
    if !active_transport {
        scheduling.peer_recovery_delay = None;
        scheduling.peer_recovery_started_at = None;
        scheduling.peer_recovery_attempts = 0;
        scheduling.peer_recovery_exhausted = false;
        return None;
    }

    let started_at = match scheduling.peer_recovery_started_at {
        Some(started_at) => started_at,
        None => {
            scheduling.peer_recovery_generation =
                scheduling.peer_recovery_generation.saturating_add(1);
            scheduling.peer_recovery_attempts = 0;
            scheduling.peer_recovery_exhausted = false;
            let started_at = now;
            scheduling.peer_recovery_started_at = Some(started_at);
            started_at
        }
    };
    let elapsed = now.saturating_duration_since(started_at);
    if elapsed >= PEER_RECOVERY_WINDOW {
        scheduling.peer_recovery_delay = None;
        scheduling.peer_recovery_exhausted = true;
        return None;
    }

    let delay = scheduling.peer_recovery_delay.unwrap_or(Duration::from_millis(250));
    let delay = delay.min(PEER_RECOVERY_WINDOW.saturating_sub(elapsed));
    scheduling.peer_recovery_attempts = scheduling.peer_recovery_attempts.saturating_add(1);
    scheduling.peer_recovery_delay = Some(next_peer_recovery_delay(delay));
    Some(delay)
}
