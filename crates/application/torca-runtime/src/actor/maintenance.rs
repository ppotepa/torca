// Responsibility: one runtime maintenance turn split into explicit phases.

fn maintain_runtime_health<P: PairingDriver, T: TorDriver>(
    engine: &EngineHandle,
    pairing: &mut P,
    tor: &mut T,
    relay_health: Option<&RelayHealthHandle>,
    policy: &mut RuntimeGovernor,
    health: &mut RuntimeHealthState,
    work: &mut RuntimeWorkState,
    counters: &mut RuntimeCounters,
    diagnostics: &mut DiagnosticBuffer,
    sequence: &mut u128,
    connectivity: &ConnectivityObserver,
    critical_lease: &AtomicBool,
    now: Timestamp,
) {
    if work.refresh_contacts {
        if let Ok(snapshot) = engine.overview_snapshot() {
            diagnostics.count(RuntimeCounter::SnapshotBuild);
            diagnostics.count(RuntimeCounter::DbRead);
            work.contacts = snapshot
                .contacts
                .iter()
                .filter(|contact| contact.status() == ContactStatus::Active)
                .map(torca_contacts::Contact::id)
                .collect();
        }
        work.refresh_contacts = false;
    }

    if let Some(relay) = relay_health {
        let demanded = policy.has_active_lease(ResourceScope::Relay, std::time::Instant::now());
        relay.set_demand(demanded);
    }
    critical_lease.store(
        !work.active_attachment_leases.is_empty()
            || !work.active_delivery_leases.is_empty()
            || policy.has_durable_lease(std::time::Instant::now()),
        Ordering::Release,
    );

    let relay_snapshot = relay_health
        .map_or_else(RelayHealthSnapshot::default, RelayHealthHandle::snapshot);
    let relay_probe_completed = relay_snapshot.probe_count > counters.last_relay_probe_count;
    if relay_probe_completed {
        diagnostics.count_by(
            RuntimeCounter::RelayProbe,
            relay_snapshot
                .probe_count
                .saturating_sub(counters.last_relay_probe_count),
        );
        counters.last_relay_probe_count = relay_snapshot.probe_count;
    }
    let relay_state = (relay_snapshot.status, relay_snapshot.diagnostic_code);
    if relay_probe_completed && relay_snapshot.status == ProbeStatus::Healthy {
        policy.apply(
            PolicyEvent::Evidence {
                scope: ResourceScope::Relay,
                kind: torca_battery::EvidenceKind::Probe,
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
        tor.maintenance(now),
        &mut health.tor_failed,
        diagnostics,
        sequence,
        now,
        Component::Tor,
        "TOR_MAINTENANCE_FAILED",
        "TOR_MAINTENANCE_RECOVERED",
    );
    observe_maintenance(
        pairing.maintenance(now),
        &mut health.pairing_failed,
        diagnostics,
        sequence,
        now,
        Component::Relay,
        "PAIRING_MAINTENANCE_FAILED",
        "PAIRING_MAINTENANCE_RECOVERED",
    );

    let tor_state = tor.state();
    let onion_state = tor.onion_service_state();
    if tor_state == TorState::Ready
        && !work.bootstrap_relay_probe_started
        && !work.bootstrap_relay_probe_finished
    {
        acquire_bootstrap_relay_lease(policy);
        if let Some(relay) = relay_health {
            relay.set_demand(true);
        }
        work.bootstrap_relay_probe_started = true;
    }
    record_runtime_probes(
        &mut health.probes,
        tor_state,
        onion_state,
        health.communication_failed,
        relay_probe_result(relay_snapshot, now),
        now,
    );
    for probe in health.probes.latest() {
        connectivity.record_probe(&probe);
    }
    if health.last_tor_state != Some(tor_state) {
        health.last_tor_state = Some(tor_state);
        record(
            diagnostics,
            sequence,
            now,
            Component::Tor,
            map_health(tor_state),
            "TOR_STATE_CHANGED",
        );
    }
    if health.last_onion_state != Some(onion_state) {
        health.last_onion_state = Some(onion_state);
        record(
            diagnostics,
            sequence,
            now,
            Component::Tor,
            map_onion_health(onion_state),
            onion_event_code(onion_state),
        );
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
    let maintenance_result = communication.maintenance(&work.contacts, now);
    if work.active_attachment_leases.is_empty() && work.active_delivery_leases.is_empty() {
        let retained = work
            .contacts
            .iter()
            .copied()
            .filter(|contact_id| has_peer_or_radio_lease(policy, *contact_id))
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

    if !work.active_attachment_leases.is_empty()
        && let Ok(views) = communication.attachment_snapshot()
    {
        for view in views {
            if matches!(view.status.as_str(), "available" | "cancelled") {
                work.active_attachment_leases.remove(&view.id);
                policy.release_lease(attachment_lease_owner(view.id));
            } else {
                acquire_attachment_lease(policy, &mut work.active_attachment_leases, view.id);
            }
        }
    }
    if projection_changed && !work.active_delivery_leases.is_empty() {
        for message_id in work
            .active_delivery_leases
            .iter()
            .copied()
            .collect::<Vec<_>>()
        {
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
    work: &RuntimeWorkState,
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
    let mut current_activity = BTreeMap::new();

    for id in work.contacts.iter().copied() {
        let state = communication.connection_state(id);
        let previous_state = health.last_peer_states.get(&id).copied();
        if health.last_peer_states.get(&id) != Some(&state) {
            health.transport_activity.mark_peer(id, now);
            record(
                diagnostics,
                sequence,
                now,
                Component::Peer,
                map_peer_health(state),
                "PEER_STATE_CHANGED",
            );
        }
        if state == PeerConnectionStatus::Ready
            && previous_state != Some(PeerConnectionStatus::Ready)
        {
            policy.apply(
                PolicyEvent::Evidence {
                    scope: ResourceScope::Peer(id.to_opaque()),
                    kind: torca_battery::EvidenceKind::Handshake,
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
                    kind: torca_battery::EvidenceKind::Failure,
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
                    kind: torca_battery::EvidenceKind::Ack,
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
            let tx_delta = evidence
                .tx_frames
                .saturating_sub(previous.map_or(0, |value| value.tx_frames));
            let rx_delta = evidence
                .rx_frames
                .saturating_sub(previous.map_or(0, |value| value.rx_frames));
            let handshake_delta = evidence
                .handshakes
                .saturating_sub(previous.map_or(0, |value| value.handshakes));

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
                (tx_changed, torca_battery::EvidenceKind::Tx),
                (rx_changed, torca_battery::EvidenceKind::Rx),
                (ack_changed, torca_battery::EvidenceKind::Ack),
                (handshake_changed, torca_battery::EvidenceKind::Handshake),
                (failure_changed, torca_battery::EvidenceKind::Failure),
            ] {
                if changed {
                    policy.apply(PolicyEvent::Evidence { scope, kind }, policy_now);
                }
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

    if maintenance_result.is_ok() {
        maintenance_result = maintain_peer_probes(
            communication,
            &work.contacts,
            &mut health.peer_probes,
            policy,
            work.battery_policy,
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

fn update_runtime_schedule<P: PairingDriver, C: CommunicationDriver, T: TorDriver>(
    tor: &T,
    pairing: &P,
    communication: &C,
    policy: &mut RuntimeGovernor,
    background_sync: torca_battery::BackgroundSyncCadence,
    foreground: bool,
    scheduling: &mut RuntimeSchedulingState,
    diagnostics: &mut DiagnosticBuffer,
    active_transport: bool,
    now: Timestamp,
) {
    let background_delay = (!foreground).then(|| policy_background_delay(scheduling, background_sync)).flatten();
    let lease_delay = policy
        .next_lease_expiry()
        .map(|expiry| expiry.saturating_duration_since(std::time::Instant::now()));
    let peer_delay = (!active_transport)
        .then_some(scheduling.peer_probe_deadline)
        .flatten()
        .and_then(|deadline| deadline.duration_since(now));
    let next_delay = next_runtime_delay(
        tor.next_maintenance_delay(now),
        pairing.next_maintenance_delay(now),
        communication.next_maintenance_delay(now),
        lease_delay,
        peer_delay,
        background_delay,
    );
    diagnostics.set_policy_snapshot(policy.snapshot(std::time::Instant::now()));
    scheduling.next_maintenance_at = next_delay.map(|delay| std::time::Instant::now() + delay);
}

fn policy_background_delay(
    scheduling: &mut RuntimeSchedulingState,
    cadence: torca_battery::BackgroundSyncCadence,
) -> Option<Duration> {
    let interval = cadence.approximate_interval()?;
    let deadline = scheduling
        .background_sync_deadline
        .get_or_insert_with(|| std::time::Instant::now() + interval);
    Some(deadline.saturating_duration_since(std::time::Instant::now()))
}
