impl RuntimeOwner {
    pub fn spawn<P: PairingDriver, C: CommunicationDriver, T: TorDriver>(
        engine: EngineHandle,
        pairing: P,
        communication: C,
        tor: T,
    ) -> (RuntimeHandle, Self) {
        Self::spawn_with_connectivity(
            engine,
            pairing,
            communication,
            tor,
            None,
            ConnectivityObserver::default(),
        )
    }

    pub fn spawn_with_relay_probe<P: PairingDriver, C: CommunicationDriver, T: TorDriver>(
        engine: EngineHandle,
        pairing: P,
        communication: C,
        tor: T,
        relay_probe: Option<Arc<dyn RelayProbe>>,
    ) -> (RuntimeHandle, Self) {
        Self::spawn_with_connectivity(
            engine,
            pairing,
            communication,
            tor,
            relay_probe,
            ConnectivityObserver::default(),
        )
    }

    pub fn spawn_with_connectivity<P: PairingDriver, C: CommunicationDriver, T: TorDriver>(
        engine: EngineHandle,
        mut pairing: P,
        mut communication: C,
        mut tor: T,
        relay_probe: Option<Arc<dyn RelayProbe>>,
        connectivity: ConnectivityObserver,
    ) -> (RuntimeHandle, Self) {
        let relay_info = relay_probe.clone();
        let relay_worker = relay_probe.and_then(|probe| {
            RelayHealthWorker::spawn_demand_driven(Arc::new(RuntimeRelayHealthPort(probe)))
                .map_err(|error| {
                    eprintln!("torca-runtime: relay supervisor unavailable: {error}");
                    error
                })
                .ok()
        });
        let relay_health = relay_worker.as_ref().map(RelayHealthWorker::handle);
        let (sender, receiver) = mpsc::sync_channel(MAILBOX_CAPACITY);
        let critical_lease = Arc::new(AtomicBool::new(false));
        let critical_lease_for_actor = Arc::clone(&critical_lease);
        let wake_sender = sender.clone();
        let runtime_waker: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
            // The accept worker must never block on the runtime mailbox. A
            // full queue only coalesces this wake; the next command or
            // deadline will drain the listener as well.
            let _ = wake_sender.try_send(RuntimeCommand::Wake);
        });
        communication.set_waker(Arc::clone(&runtime_waker));
        tor.set_waker(runtime_waker);
        let handle = RuntimeHandle { sender: sender.clone(), critical_lease };
        let join = thread::spawn(move || {
            let mut diagnostics = DiagnosticBuffer::new(256);
            let mut policy = RuntimeGovernor::new(std::time::Instant::now());
            let mut sequence = 1_u128;
            let startup = current_timestamp().unwrap_or(Timestamp::UNIX_EPOCH);
            match communication.recover(startup) {
                Ok(()) => record(
                    &mut diagnostics,
                    &mut sequence,
                    startup,
                    Component::Storage,
                    HealthState::Ready,
                    "DELIVERY_RECOVERY_READY",
                ),
                Err(_) => record(
                    &mut diagnostics,
                    &mut sequence,
                    startup,
                    Component::Storage,
                    HealthState::Failed,
                    "DELIVERY_RECOVERY_FAILED",
                ),
            }
            record(
                &mut diagnostics,
                &mut sequence,
                startup,
                Component::Engine,
                HealthState::Starting,
                "RUNTIME_STARTED",
            );
            run_loop(
                receiver,
                &engine,
                &mut pairing,
                &mut communication,
                &mut tor,
                &mut diagnostics,
                &mut sequence,
                &mut policy,
                relay_health,
                relay_info,
                connectivity,
                critical_lease_for_actor,
            );
            communication.shutdown();
            pairing.shutdown();
            tor.shutdown();
        });
        (handle, Self { sender, join: Some(join), relay_worker })
    }
    pub fn shutdown(mut self) -> Result<(), RuntimeDriverError> {
        let (tx, rx) = mpsc::channel();
        send_with_timeout(&self.sender, RuntimeCommand::Shutdown(tx))?;
        rx.recv_timeout(SHUTDOWN_WAIT).map_err(|_| RuntimeDriverError::Communication)?;
        if let Some(join) = self.join.take() {
            join.join().map_err(|_| RuntimeDriverError::Communication)?;
        }
        if let Some(worker) = self.relay_worker.take() {
            worker.shutdown();
        }
        Ok(())
    }

    pub fn network_changed(&self) {
        let _ = send_with_timeout(&self.sender, RuntimeCommand::NetworkChanged);
    }
}

fn run_loop<P: PairingDriver, C: CommunicationDriver, T: TorDriver>(
    receiver: Receiver<RuntimeCommand>,
    engine: &EngineHandle,
    pairing: &mut P,
    communication: &mut C,
    tor: &mut T,
    diagnostics: &mut DiagnosticBuffer,
    sequence: &mut u128,
    policy: &mut RuntimeGovernor,
    relay_health: Option<RelayHealthHandle>,
    relay_info: Option<Arc<dyn RelayProbe>>,
    connectivity: ConnectivityObserver,
    critical_lease: Arc<AtomicBool>,
) {
    let mut last_tor_state = None;
    let mut last_onion_state = None;
    let mut last_relay_state = None::<(ProbeStatus, ErrorCode)>;
    let mut last_peer_states = BTreeMap::<ContactId, PeerConnectionStatus>::new();
    let mut last_peer_successes = BTreeMap::<ContactId, Option<Timestamp>>::new();
    let mut last_peer_activity = BTreeMap::<ContactId, PeerActivityEvidence>::new();
    let mut tor_failed = false;
    let mut pairing_failed = false;
    let mut communication_failed = false;
    let mut probes = ProbeSupervisor::default();
    let mut peer_probes = PeerProbeSupervisor::default();
    let mut transport_activity = TransportActivityLedger::default();
    let mut next_maintenance_at = Some(std::time::Instant::now());
    let mut peer_probe_deadline = None;
    let mut battery_policy = BatteryPolicy::new(BatteryProfile::AlwaysAvailable);
    let mut metered_transfers = MeteredTransferPolicy::PauseLarge;
    let mut metered_network = false;
    let mut contacts = Vec::<ContactId>::new();
    let mut refresh_contacts = true;
    let mut attention_owner = None;
    let mut attention_generation = 0_u64;
    let mut last_relay_probe_count = 0_u64;
    let mut active_attachment_leases = BTreeSet::<OpaqueId>::new();
    let mut active_delivery_leases = BTreeSet::<OpaqueId>::new();
    let mut last_worker_database_writes = 0_u64;
    let mut last_blob_writes = 0_u64;
    let mut last_attachment_chunks = 0_u64;
    let mut last_attachment_suppressed = 0_u64;
    let mut last_projection_events = 0_u64;
    let mut bootstrap_relay_probe_started = false;
    let mut bootstrap_relay_probe_finished = false;
    loop {
        match wait_for_runtime_command(&receiver, next_maintenance_at) {
            RuntimeWait::Command(RuntimeCommand::Shutdown(response)) => {
                let _ = response.send(());
                break;
            }
            RuntimeWait::Command(RuntimeCommand::SetAttention(context)) => {
                if context.generation < attention_generation {
                    // The policy reducer also rejects stale attention, but
                    // the runtime-owned lease must obey the same ordering or
                    // an old route could release a newer route's demand.
                    continue;
                }
                attention_generation = context.generation;
                let now = std::time::Instant::now();
                if let Some(owner) = attention_owner.take() {
                    policy.release_lease(owner);
                }
                let owner = OpaqueId::from_u128(u128::from(context.generation.max(1)));
                let expires_at = now + Duration::from_secs(5 * 60);
                let demand = match context.surface {
                    torca_battery::AttentionSurface::Conversation(peer)
                    | torca_battery::AttentionSurface::Radio(peer) => Some(WorkDemand {
                        scope: ResourceScope::Peer(peer),
                        class: if matches!(
                            context.surface,
                            torca_battery::AttentionSurface::Conversation(_)
                        ) {
                            WorkClass::PeerDial
                        } else {
                            WorkClass::PeerProbe
                        },
                        reason: if matches!(
                            context.surface,
                            torca_battery::AttentionSurface::Radio(_)
                        ) {
                            DemandReason::RadioSession
                        } else {
                            // The single active conversation is the runtime's
                            // implicit focus lease. Only this peer receives
                            // proactive responsiveness while the route is open.
                            DemandReason::FocusedConversation
                        },
                        owner,
                        expires_at,
                    }),
                    torca_battery::AttentionSurface::Pairing(_relay) => Some(WorkDemand {
                        scope: ResourceScope::Relay,
                        class: WorkClass::RelayProbe,
                        reason: DemandReason::ActivePairing,
                        owner,
                        expires_at,
                    }),
                    _ => None,
                };
                if let Some(demand) = demand {
                    if let torca_battery::AttentionSurface::Conversation(peer) = context.surface {
                        // Conversation attention is the one explicit focus
                        // lease. This replaces, rather than stacks with,
                        // previous focus and keeps one peer responsive.
                        policy.acquire_focus(peer, owner, expires_at, now);
                    } else {
                        policy.acquire_lease(demand);
                    }
                    attention_owner = Some(owner);
                }
                policy.apply(PolicyEvent::Attention(context), now);
            }
            RuntimeWait::Command(RuntimeCommand::NetworkChanged) => {
                refresh_contacts = true;
                if let Some(relay) = &relay_health {
                    relay.network_changed();
                }
                let now = current_timestamp().unwrap_or(Timestamp::UNIX_EPOCH);
                pairing.network_changed(now);
                communication.network_changed(now);
                record(
                    diagnostics,
                    sequence,
                    now,
                    Component::Relay,
                    HealthState::Starting,
                    "RELAY_NETWORK_CHANGED",
                );
            }
            RuntimeWait::Command(RuntimeCommand::SetRadioDemand(contact_id, enabled)) => {
                if enabled {
                    let _ = tor.set_dormant(false);
                    acquire_radio_lease(policy, contact_id);
                    diagnostics.record_battery(BatteryMetric::RadioWake, 1, WakeReason::Radio);
                } else {
                    policy.release_lease(radio_lease_owner(contact_id));
                }
            }
            RuntimeWait::Command(RuntimeCommand::SetRadioTransmission(contact_id, active)) => {
                if active {
                    let _ = tor.set_dormant(false);
                    acquire_radio_transmission_lease(policy, contact_id);
                    diagnostics.record_battery(BatteryMetric::RadioWake, 1, WakeReason::Radio);
                } else {
                    policy.release_lease(radio_transmission_lease_owner(contact_id));
                }
            }
            RuntimeWait::Command(RuntimeCommand::SetBatteryProfile(profile)) => {
                battery_policy.set_profile(profile);
                communication.set_battery_policy(profile, metered_transfers, metered_network);
                diagnostics.set_battery_profile(profile);
            }
            RuntimeWait::Command(RuntimeCommand::SetMeteredNetwork(metered)) => {
                metered_network = metered;
                communication.set_battery_policy(
                    battery_policy.profile(),
                    metered_transfers,
                    metered_network,
                );
            }
            RuntimeWait::Command(RuntimeCommand::SetMeteredTransferPolicy(policy)) => {
                metered_transfers = policy;
                communication.set_battery_policy(
                    battery_policy.profile(),
                    metered_transfers,
                    metered_network,
                );
            }
            RuntimeWait::Command(RuntimeCommand::SetTorDormancy(dormant)) => {
                let _ = tor.set_dormant(dormant);
            }
            RuntimeWait::Command(RuntimeCommand::WakeDelivery(message_id)) => {
                let _ = tor.set_dormant(false);
                active_delivery_leases.insert(message_id);
                acquire_delivery_lease(policy, message_id);
                refresh_contacts = true;
            }
            RuntimeWait::Command(RuntimeCommand::ReleaseDelivery(message_id)) => {
                active_delivery_leases.remove(&message_id);
                policy.release_lease(delivery_lease_owner(message_id));
            }
            RuntimeWait::Command(command) => {
                if command_requires_network(&command) {
                    // A durable/user-visible operation is a demand edge. It
                    // must wake soft-dormant Tor before its transport worker
                    // attempts a dial; otherwise background delivery can sit
                    // behind dormancy until the next lifecycle event.
                    let _ = tor.set_dormant(false);
                }
                if command_writes_database(&command) {
                    diagnostics.count(RuntimeCounter::DbWrite);
                }
                refresh_contacts = true;
                let now = current_timestamp().unwrap_or(Timestamp::UNIX_EPOCH);
                handle_command(
                    command,
                    engine,
                    pairing,
                    communication,
                    tor,
                    &probes,
                    relay_info.as_ref(),
                    relay_health.as_ref(),
                    &mut transport_activity,
                    &connectivity,
                    policy,
                    &mut active_attachment_leases,
                    diagnostics,
                    sequence,
                    now,
                );
            }
            RuntimeWait::Timeout => {}
            RuntimeWait::Closed => break,
        }
        diagnostics.count(RuntimeCounter::SchedulerWakeup);
        let now = current_timestamp().unwrap_or(Timestamp::UNIX_EPOCH);
        if refresh_contacts {
            if let Ok(snapshot) = engine.overview_snapshot() {
                diagnostics.count(RuntimeCounter::SnapshotBuild);
                diagnostics.count(RuntimeCounter::DbRead);
                contacts = snapshot
                    .contacts
                    .iter()
                    .filter(|contact| contact.status() == ContactStatus::Active)
                    .map(torca_contacts::Contact::id)
                    .collect();
            }
            refresh_contacts = false;
        }
        if let Some(relay) = &relay_health {
            let demanded = policy.has_active_lease(ResourceScope::Relay, std::time::Instant::now());
            relay.set_demand(demanded);
        }
        critical_lease.store(
            !active_attachment_leases.is_empty()
                || !active_delivery_leases.is_empty()
                || policy.has_active_lease(ResourceScope::Relay, std::time::Instant::now()),
            Ordering::Release,
        );
        let relay_snapshot = relay_health
            .as_ref()
            .map_or_else(RelayHealthSnapshot::default, RelayHealthHandle::snapshot);
        let relay_probe_completed = relay_snapshot.probe_count > last_relay_probe_count;
        if relay_probe_completed {
            diagnostics.count_by(
                RuntimeCounter::RelayProbe,
                relay_snapshot.probe_count.saturating_sub(last_relay_probe_count),
            );
            last_relay_probe_count = relay_snapshot.probe_count;
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
            && bootstrap_relay_probe_started
            && matches!(
                relay_snapshot.status,
                ProbeStatus::Healthy
                    | ProbeStatus::Degraded
                    | ProbeStatus::Failed
                    | ProbeStatus::Unreachable
            )
        {
            policy.release_lease(bootstrap_relay_lease_owner());
            bootstrap_relay_probe_started = false;
            bootstrap_relay_probe_finished = true;
        }
        if last_relay_state.as_ref() != Some(&relay_state) {
            record(
                diagnostics,
                sequence,
                now,
                Component::Relay,
                map_probe_health(relay_snapshot.status),
                relay_event_code(relay_snapshot.status),
            );
            last_relay_state = Some(relay_state);
        }
        observe_maintenance(
            tor.maintenance(now),
            &mut tor_failed,
            diagnostics,
            sequence,
            now,
            Component::Tor,
            "TOR_MAINTENANCE_FAILED",
            "TOR_MAINTENANCE_RECOVERED",
        );
        observe_maintenance(
            pairing.maintenance(now),
            &mut pairing_failed,
            diagnostics,
            sequence,
            now,
            Component::Relay,
            "PAIRING_MAINTENANCE_FAILED",
            "PAIRING_MAINTENANCE_RECOVERED",
        );
        let tor_state = tor.state();
        let onion_state = tor.onion_service_state();
        // Demand-driven relay health must still receive one bounded initial
        // sample after Tor becomes usable. Without this lease the bootstrap
        // projection can remain `Checking` forever because no pairing UI is
        // visible yet to create normal relay demand.
        if tor_state == TorState::Ready
            && !bootstrap_relay_probe_started
            && !bootstrap_relay_probe_finished
        {
            acquire_bootstrap_relay_lease(policy);
            if let Some(relay) = &relay_health {
                relay.set_demand(true);
            }
            bootstrap_relay_probe_started = true;
        }
        record_runtime_probes(
            &mut probes,
            tor_state,
            onion_state,
            communication_failed,
            relay_probe_result(relay_snapshot, now),
            now,
        );
        for probe in probes.latest() {
            connectivity.record_probe(&probe);
        }
        if last_tor_state != Some(tor_state) {
            last_tor_state = Some(tor_state);
            record(
                diagnostics,
                sequence,
                now,
                Component::Tor,
                map_health(tor_state),
                "TOR_STATE_CHANGED",
            );
        }
        if last_onion_state != Some(onion_state) {
            last_onion_state = Some(onion_state);
            record(
                diagnostics,
                sequence,
                now,
                Component::Tor,
                map_onion_health(onion_state),
                onion_event_code(onion_state),
            );
        }
        // Process actual transport activity before choosing the next cosmetic
        // probe. Otherwise a frame received in this maintenance turn could
        // leave a stale probe deadline armed for another wakeup.
        let mut maintenance_result = communication.maintenance(&contacts, now);
        let worker_database_writes = communication.database_write_count();
        if worker_database_writes > last_worker_database_writes {
            diagnostics.count_by(
                RuntimeCounter::DbWrite,
                worker_database_writes.saturating_sub(last_worker_database_writes),
            );
        }
        last_worker_database_writes = worker_database_writes;
        let blob_writes = communication.blob_write_count();
        if blob_writes > last_blob_writes {
            diagnostics
                .count_by(RuntimeCounter::BlobWrite, blob_writes.saturating_sub(last_blob_writes));
        }
        last_blob_writes = blob_writes;
        let attachment_chunks = communication.attachment_chunk_tx_count();
        if attachment_chunks > last_attachment_chunks {
            diagnostics.record_battery(
                BatteryMetric::AttachmentChunkTx,
                attachment_chunks - last_attachment_chunks,
                WakeReason::AttachmentTransfer,
            );
        }
        last_attachment_chunks = attachment_chunks;

        let suppressed = communication.attachment_policy_suppressed_count();
        if suppressed > last_attachment_suppressed {
            diagnostics.record_battery(
                BatteryMetric::SuppressedWork,
                suppressed - last_attachment_suppressed,
                WakeReason::PolicySuppressed,
            );
        }
        last_attachment_suppressed = suppressed;
        let projection_events = engine.projection_event_count();
        let projection_changed = projection_events > last_projection_events;
        if projection_changed {
            diagnostics.count_by(
                RuntimeCounter::ProjectionEvent,
                projection_events.saturating_sub(last_projection_events),
            );
        }
        last_projection_events = projection_events;
        if !active_attachment_leases.is_empty()
            && let Ok(views) = communication.attachment_snapshot()
        {
            for view in views {
                if matches!(view.status.as_str(), "available" | "cancelled") {
                    active_attachment_leases.remove(&view.id);
                    policy.release_lease(attachment_lease_owner(view.id));
                } else {
                    acquire_attachment_lease(policy, &mut active_attachment_leases, view.id);
                }
            }
        }
        if projection_changed && !active_delivery_leases.is_empty() {
            for message_id in active_delivery_leases.iter().copied().collect::<Vec<_>>() {
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
                    active_delivery_leases.remove(&message_id);
                    policy.release_lease(delivery_lease_owner(message_id));
                }
            }
        }
        let mut current = BTreeMap::new();
        let mut current_successes = BTreeMap::new();
        let activity = CommunicationDriver::peer_activity(communication)
            .into_iter()
            .map(|evidence| (evidence.contact_id, evidence))
            .collect::<BTreeMap<_, _>>();
        let mut current_activity = BTreeMap::new();
        for id in contacts.iter().copied() {
            let state = communication.connection_state(id);
            let previous_state = last_peer_states.get(&id).copied();
            if last_peer_states.get(&id) != Some(&state) {
                transport_activity.mark_peer(id, now);
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
            let health = communication.peer_health(id);
            if health.last_success_at.is_some()
                && last_peer_successes.get(&id) != Some(&health.last_success_at)
            {
                transport_activity.mark_peer(id, now);
                policy.apply(
                    PolicyEvent::Evidence {
                        scope: ResourceScope::Peer(id.to_opaque()),
                        kind: torca_battery::EvidenceKind::Ack,
                    },
                    std::time::Instant::now(),
                );
            }
            if let Some(evidence) = activity.get(&id).copied() {
                let previous = last_peer_activity.get(&id).copied();
                let tx_changed = evidence.tx_frames > previous.map_or(0, |value| value.tx_frames);
                let rx_changed = evidence.rx_frames > previous.map_or(0, |value| value.rx_frames);
                let ack_changed = evidence.tx_acks > previous.map_or(0, |value| value.tx_acks)
                    || evidence.rx_acks > previous.map_or(0, |value| value.rx_acks);
                let handshake_changed =
                    evidence.handshakes > previous.map_or(0, |value| value.handshakes);
                let failure_changed =
                    evidence.failures > previous.map_or(0, |value| value.failures);
                let tx_delta =
                    evidence.tx_frames.saturating_sub(previous.map_or(0, |value| value.tx_frames));
                let rx_delta =
                    evidence.rx_frames.saturating_sub(previous.map_or(0, |value| value.rx_frames));
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
                    transport_activity.mark_peer(id, now);
                }
                let scope = ResourceScope::Peer(id.to_opaque());
                let policy_now = std::time::Instant::now();
                if tx_changed {
                    policy.apply(
                        PolicyEvent::Evidence { scope, kind: torca_battery::EvidenceKind::Tx },
                        policy_now,
                    );
                }
                if rx_changed {
                    policy.apply(
                        PolicyEvent::Evidence { scope, kind: torca_battery::EvidenceKind::Rx },
                        policy_now,
                    );
                }
                if ack_changed {
                    policy.apply(
                        PolicyEvent::Evidence { scope, kind: torca_battery::EvidenceKind::Ack },
                        policy_now,
                    );
                }
                if handshake_changed {
                    policy.apply(
                        PolicyEvent::Evidence {
                            scope,
                            kind: torca_battery::EvidenceKind::Handshake,
                        },
                        policy_now,
                    );
                }
                if failure_changed {
                    policy.apply(
                        PolicyEvent::Evidence { scope, kind: torca_battery::EvidenceKind::Failure },
                        policy_now,
                    );
                }
                current_activity.insert(id, evidence);
            }
            current.insert(id, state);
            connectivity.set_peer_ready(id.to_opaque(), state == PeerConnectionStatus::Ready);
            current_successes.insert(id, health.last_success_at);
        }
        let active_transport = current.values().any(|state| {
            matches!(
                state,
                PeerConnectionStatus::Connecting
                    | PeerConnectionStatus::Handshaking
                    | PeerConnectionStatus::Reconnecting
            )
        });
        last_peer_states = current;
        last_peer_successes = current_successes;
        last_peer_activity = current_activity;

        if maintenance_result.is_ok() {
            maintenance_result = maintain_peer_probes(
                communication,
                &contacts,
                &mut peer_probes,
                policy,
                battery_policy,
                now,
            )
            .map(|(deadline, probe_started)| {
                peer_probe_deadline = deadline;
                if probe_started {
                    diagnostics.count(RuntimeCounter::PeerProbe);
                }
            });
        }
        observe_maintenance(
            maintenance_result,
            &mut communication_failed,
            diagnostics,
            sequence,
            now,
            Component::Peer,
            "COMMUNICATION_MAINTENANCE_FAILED",
            "COMMUNICATION_MAINTENANCE_RECOVERED",
        );

        let lease_delay = policy
            .next_lease_expiry()
            .map(|expiry| expiry.saturating_duration_since(std::time::Instant::now()));
        let peer_delay = (!active_transport)
            .then_some(peer_probe_deadline)
            .flatten()
            .and_then(|deadline| deadline.duration_since(now));
        let next_delay = next_runtime_delay(
            tor.next_maintenance_delay(now),
            pairing.next_maintenance_delay(now),
            communication.next_maintenance_delay(now),
            lease_delay,
            peer_delay,
        );
        diagnostics.set_policy_snapshot(policy.snapshot(std::time::Instant::now()));
        next_maintenance_at = next_delay.map(|delay| std::time::Instant::now() + delay);
    }
}
