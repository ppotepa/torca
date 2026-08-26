// Responsibility: runtime owner and linear event loop.

impl RuntimeOwner {
    pub fn spawn<P: PairingDriver, C: CommunicationDriver, T: CommunicationLifecycle>(
        engine: EngineHandle,
        pairing: P,
        communication: C,
        communication_lifecycle: T,
    ) -> (RuntimeHandle, Self) {
        Self::spawn_with_connectivity(
            engine,
            pairing,
            communication,
            communication_lifecycle,
            None,
            ConnectivityObserver::default(),
        )
    }

    pub fn spawn_with_rendezvous_probe<
        P: PairingDriver,
        C: CommunicationDriver,
        T: CommunicationLifecycle,
    >(
        engine: EngineHandle,
        pairing: P,
        communication: C,
        communication_lifecycle: T,
        rendezvous_probe: Option<Arc<dyn RendezvousProbe>>,
    ) -> (RuntimeHandle, Self) {
        Self::spawn_with_connectivity(
            engine,
            pairing,
            communication,
            communication_lifecycle,
            rendezvous_probe,
            ConnectivityObserver::default(),
        )
    }

    pub fn spawn_with_connectivity<
        P: PairingDriver,
        C: CommunicationDriver,
        T: CommunicationLifecycle,
    >(
        engine: EngineHandle,
        mut pairing: P,
        mut communication: C,
        mut communication_lifecycle: T,
        rendezvous_probe: Option<Arc<dyn RendezvousProbe>>,
        connectivity: ConnectivityObserver,
    ) -> (RuntimeHandle, Self) {
        let rendezvous_info = rendezvous_probe.clone();
        let rendezvous_worker = rendezvous_probe.and_then(|probe| {
            RendezvousHealthWorker::spawn_demand_driven(Arc::new(RuntimeRendezvousHealthPort(probe)))
                .map_err(|error| {
                    eprintln!("torca-runtime: rendezvous supervisor unavailable: {error}");
                    error
                })
                .ok()
        });
        let rendezvous_health =
            rendezvous_worker.as_ref().map(RendezvousHealthWorker::handle);
        let (sender, receiver) = mpsc::sync_channel(MAILBOX_CAPACITY);
        let communication_sender = sender.clone();
        let communication_wake_pending = Arc::new(AtomicBool::new(false));
        let communication_wake_gate = Arc::clone(&communication_wake_pending);
        let communication_waker: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
            // Transport activity can advance delivery and peer evidence, but
            // must not become an anonymous "maintain everything" wake.
            if communication_wake_gate.swap(true, Ordering::AcqRel) {
                return;
            }
            if communication_sender
                .try_send(RuntimeCommand::Wake(vec![
                    RuntimeWakeSource::DeliveryDeadline,
                    RuntimeWakeSource::PeerDeadline,
                ]))
                .is_err()
            {
                communication_wake_gate.store(false, Ordering::Release);
            }
        });
        let lifecycle_sender = sender.clone();
        let lifecycle_wake_pending = Arc::new(AtomicBool::new(false));
        let lifecycle_wake_gate = Arc::clone(&lifecycle_wake_pending);
        let lifecycle_waker: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
            if lifecycle_wake_gate.swap(true, Ordering::AcqRel) {
                return;
            }
            if lifecycle_sender
                .try_send(RuntimeCommand::Wake(vec![RuntimeWakeSource::ProviderDeadline]))
                .is_err()
            {
                lifecycle_wake_gate.store(false, Ordering::Release);
            }
        });
        let radio_sender = sender.clone();
        let radio_wake_pending = Arc::new(AtomicBool::new(false));
        let radio_wake_gate = Arc::clone(&radio_wake_pending);
        let radio_waker: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
            if radio_wake_gate.swap(true, Ordering::AcqRel) {
                return;
            }
            if radio_sender
                .try_send(RuntimeCommand::Wake(vec![RuntimeWakeSource::RadioDeadline]))
                .is_err()
            {
                radio_wake_gate.store(false, Ordering::Release);
            }
        });
        communication.set_waker(communication_waker);
        communication.set_radio_waker(radio_waker);
        communication_lifecycle.set_waker(lifecycle_waker);
        let handle = RuntimeHandle { sender: sender.clone() };
        let join = thread::spawn(move || {
        let mut diagnostics = DiagnosticBuffer::new(256);
            diagnostics.set_provider_context(
                communication_lifecycle.provider().wire_value(),
                communication_lifecycle.provider_profile(),
            );
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
                &mut communication_lifecycle,
                &mut diagnostics,
                &mut sequence,
                &mut policy,
                rendezvous_health,
                rendezvous_info,
                connectivity,
                communication_wake_pending,
                lifecycle_wake_pending,
                radio_wake_pending,
            );
            communication.shutdown();
            pairing.shutdown();
            communication_lifecycle.shutdown();
        });
        (handle, Self { sender, join: Some(join), rendezvous_worker })
    }

    pub fn shutdown(mut self) -> Result<(), RuntimeDriverError> {
        let (tx, rx) = mpsc::channel();
        send_with_timeout(&self.sender, RuntimeCommand::Shutdown(tx))?;
        rx.recv_timeout(SHUTDOWN_WAIT).map_err(|_| RuntimeDriverError::Communication)?;
        if let Some(join) = self.join.take() {
            join.join().map_err(|_| RuntimeDriverError::Communication)?;
        }
        if let Some(worker) = self.rendezvous_worker.take() {
            worker.shutdown();
        }
        Ok(())
    }

    pub fn network_changed(&self) {
        let _ = send_with_timeout(&self.sender, RuntimeCommand::NetworkChanged);
    }
}

fn run_loop<P: PairingDriver, C: CommunicationDriver, T: CommunicationLifecycle>(
    receiver: Receiver<RuntimeCommand>,
    engine: &EngineHandle,
    pairing: &mut P,
    communication: &mut C,
    communication_lifecycle: &mut T,
    diagnostics: &mut DiagnosticBuffer,
    sequence: &mut u128,
    policy: &mut RuntimeGovernor,
    rendezvous_health: Option<RendezvousHealthHandle>,
    rendezvous_info: Option<Arc<dyn RendezvousProbe>>,
    connectivity: ConnectivityObserver,
    communication_wake_pending: Arc<AtomicBool>,
    lifecycle_wake_pending: Arc<AtomicBool>,
    radio_wake_pending: Arc<AtomicBool>,
) {
    let mut health = RuntimeHealthState::default();
    let mut work = RuntimeWorkState::new();
    work.pending_delivery_contacts =
        engine.pending_delivery_contacts().unwrap_or_default().into_iter().collect();
    let mut counters = RuntimeCounters::default();
    let mut scheduling = RuntimeSchedulingState::new();
    let mut hot_loop = RuntimeHotLoopGuard::default();

    loop {
        let wait_started = std::time::Instant::now();
        let runtime_wait = wait_for_runtime_command(&receiver, scheduling.next_deadline());
        if matches!(runtime_wait, RuntimeWait::Command(RuntimeCommand::Wake(_))) {
            communication_wake_pending.store(false, Ordering::Release);
            lifecycle_wake_pending.store(false, Ordering::Release);
            radio_wake_pending.store(false, Ordering::Release);
            // A transport frame (notably a handshake ACK) is also a delivery
            // wake.  The runtime command itself is intentionally lightweight,
            // but the worker bridge must be told to revisit the durable
            // outbox; otherwise a message that previously saw `NotReady` can
            // remain parked even after the peer becomes ready.
            if let RuntimeWait::Command(RuntimeCommand::Wake(sources)) = &runtime_wait
                && sources.contains(&RuntimeWakeSource::DeliveryDeadline)
            {
                communication.wake_delivery();
            }
        }
        hot_loop.observe_wait(wait_started.elapsed(), &runtime_wait, &scheduling);
        let command_health = matches!(
            &runtime_wait,
            RuntimeWait::Command(command) if command.requires_health_maintenance()
        );
        let command_delivery = matches!(
            &runtime_wait,
            RuntimeWait::Command(command) if command.requires_delivery_maintenance()
        );
        let command_peer = matches!(
            &runtime_wait,
            RuntimeWait::Command(command) if command.requires_peer_maintenance()
        );
        let mut due_sources = matches!(&runtime_wait, RuntimeWait::Timeout)
            .then(|| scheduling.take_due(std::time::Instant::now()))
            .unwrap_or_default();
        if let RuntimeWait::Command(RuntimeCommand::Wake(sources)) = &runtime_wait {
            due_sources.extend(sources.iter().copied());
        }
        if due_sources.is_empty() {
            diagnostics.record_runtime_wake(match &runtime_wait {
                RuntimeWait::Command(RuntimeCommand::NetworkChanged) => {
                    RuntimeWakeSource::NetworkChange
                }
                RuntimeWait::Command(RuntimeCommand::Wake(_)) => RuntimeWakeSource::Platform,
                RuntimeWait::Command(_) => RuntimeWakeSource::Command,
                RuntimeWait::Timeout => RuntimeWakeSource::Platform,
                RuntimeWait::Closed => RuntimeWakeSource::Platform,
            });
        } else {
            for source in due_sources.iter().copied() {
                diagnostics.record_runtime_wake(source);
            }
        }
        match runtime_wait {
            RuntimeWait::Command(RuntimeCommand::Shutdown(response)) => {
                let _ = response.send(());
                break;
            }
            RuntimeWait::Command(RuntimeCommand::SetAttention(context)) => {
                if context.generation < work.attention_generation {
                    continue;
                }
                work.attention_generation = context.generation;
                let now = std::time::Instant::now();
                release_attention_leases(policy, &mut work);
                for opaque_id in context.visible_contact_ids.iter().copied() {
                    let contact_id = ContactId::from_opaque(opaque_id);
                    let owner = visible_contact_lease_owner(contact_id, context.generation);
                    acquire_visible_contact_lease(policy, contact_id, owner);
                    work.visible_contact_leases.insert(contact_id, owner);
                }
                let owner = OpaqueId::from_u128(u128::from(context.generation.max(1)));
                let demand = match context.surface {
                    AttentionSurface::Conversation(peer) | AttentionSurface::Radio(peer) => {
                        Some(WorkDemand {
                            scope: ResourceScope::Peer(peer),
                            class: if matches!(context.surface, AttentionSurface::Conversation(_)) {
                                WorkClass::PeerDial
                            } else {
                                WorkClass::PeerProbe
                            },
                            reason: if matches!(context.surface, AttentionSurface::Radio(_)) {
                                DemandReason::RadioSession
                            } else {
                                DemandReason::FocusedConversation
                            },
                            owner,
                            expires_at: now,
                        })
                    }
                    AttentionSurface::Pairing(_relay) => Some(WorkDemand {
                        scope: ResourceScope::Rendezvous,
                        class: WorkClass::RendezvousProbe,
                        reason: DemandReason::ActivePairing,
                        owner,
                        expires_at: now,
                    }),
                    _ => None,
                };
                if let Some(demand) = demand {
                    if let AttentionSurface::Conversation(peer) = context.surface {
                        policy.focus_until_release(peer, owner, now);
                    } else {
                        policy.acquire_until_release(demand);
                    }
                    work.attention_owner = Some(owner);
                }
                policy.apply(PolicyEvent::Attention(context), now);
            }
            RuntimeWait::Command(RuntimeCommand::StartBatteryObservation(response)) => {
                diagnostics.start_battery_observation();
                let _ = response.send(Ok(()));
            }
            RuntimeWait::Command(RuntimeCommand::StopBatteryObservation(response)) => {
                diagnostics.stop_battery_observation();
                let _ = response.send(Ok(()));
            }
            RuntimeWait::Command(RuntimeCommand::ResetBatteryObservation(response)) => {
                diagnostics.reset_battery_observation();
                let _ = response.send(Ok(()));
            }
            RuntimeWait::Command(RuntimeCommand::NetworkChanged) => {
                if let Some(rendezvous) = &rendezvous_health {
                    rendezvous.network_changed();
                }
                let now = current_timestamp().unwrap_or(Timestamp::UNIX_EPOCH);
                pairing.network_changed(now);
                communication.network_changed(now);
                communication_lifecycle.network_changed(now);
                record(
                    diagnostics,
                    sequence,
                    now,
                    Component::Communication,
                    HealthState::Starting,
                    "COMMUNICATION_NETWORK_CHANGED",
                );
            }
            RuntimeWait::Command(RuntimeCommand::SetRadioDemand(contact_id, enabled, response)) => {
                if enabled {
                    if let Err(error) = communication_lifecycle.set_dormant(false) {
                        let _ = response.send(Err(error));
                        continue;
                    }
                    acquire_radio_lease(policy, contact_id);
                    diagnostics.record_battery(BatteryMetric::RadioWake, 1, WakeReason::Radio);
                } else {
                    policy.release_lease(radio_lease_owner(contact_id));
                }
                let _ = response.send(Ok(()));
            }
            RuntimeWait::Command(RuntimeCommand::SetInstantContactDemand(contact_id, enabled, response)) => {
                if enabled {
                    if let Err(error) = communication_lifecycle.set_dormant(false) {
                        let _ = response.send(Err(error));
                        continue;
                    }
                    acquire_instant_contact_lease(policy, contact_id);
                } else {
                    policy.release_lease(instant_contact_lease_owner(contact_id));
                }
                let _ = response.send(Ok(()));
            }
            RuntimeWait::Command(RuntimeCommand::SetRadioTransmission(contact_id, active, response)) => {
                if active {
                    if let Err(error) = communication_lifecycle.set_dormant(false) {
                        let _ = response.send(Err(error));
                        continue;
                    }
                    acquire_radio_transmission_lease(policy, contact_id);
                    diagnostics.record_battery(BatteryMetric::RadioWake, 1, WakeReason::Radio);
                } else {
                    policy.release_lease(radio_transmission_lease_owner(contact_id));
                }
                let _ = response.send(Ok(()));
            }
            RuntimeWait::Command(RuntimeCommand::SetBatteryPolicyInputs(preferences, system, response)) => {
                work.battery_preferences = preferences;
                work.system_energy = system;
                let effective = preferences.effective(system, false);
                work.battery_policy.set_profile(effective.profile);
                work.communication_dormancy_allowed = effective.communication_dormancy_allowed;
                work.metered_transfers = effective.metered_transfers;
                work.metered_network = system.metered_network == Some(true);
                communication.set_battery_policy(
                    effective.profile,
                    effective.metered_transfers,
                    work.metered_network,
                );
                diagnostics.set_battery_profile(effective.profile);
                if !effective.communication_dormancy_allowed {
                    if let Err(error) = communication_lifecycle.set_dormant(false) {
                        let _ = response.send(Err(error));
                        continue;
                    }
                } else if !work.foreground
                    && scheduling.background_grace_deadline.is_none()
                    && !policy.has_durable_lease(std::time::Instant::now())
                {
                    // A user can change policy after the one-shot grace has
                    // already elapsed. Apply that new permission immediately
                    // without manufacturing another wake window.
                    if let Err(error) = communication_lifecycle.set_dormant(true) {
                        let _ = response.send(Err(error));
                        continue;
                    }
                }
                let _ = response.send(Ok(()));
            }
            RuntimeWait::Command(RuntimeCommand::SetForeground(foreground, response)) => {
                // A foreground transition is a user-visible request to make
                // the selected provider usable now. Do not acknowledge it or
                // mutate policy state when the provider cannot be resumed;
                // otherwise the UI reports foreground while the transport is
                // still dormant and subsequent commands appear to vanish.
                if foreground {
                    if let Err(error) = communication_lifecycle.set_dormant(false) {
                        let _ = response.send(Err(error));
                        continue;
                    }
                }
                work.foreground = foreground;
                work.system_energy.foreground = foreground;
                let effective = work.battery_preferences.effective(work.system_energy, false);
                work.battery_policy.set_profile(effective.profile);
                work.communication_dormancy_allowed = effective.communication_dormancy_allowed;
                work.metered_transfers = effective.metered_transfers;
                work.metered_network = work.system_energy.metered_network == Some(true);
                communication.set_battery_policy(
                    effective.profile,
                    effective.metered_transfers,
                    work.metered_network,
                );
                diagnostics.set_battery_profile(effective.profile);
                if foreground {
                    scheduling.background_grace_deadline = None;
                } else {
                    release_attention_leases(policy, &mut work);
                    scheduling.background_grace_deadline =
                        Some(std::time::Instant::now() + communication_lifecycle.background_grace());
                }
                let _ = response.send(Ok(()));
            }
            RuntimeWait::Command(RuntimeCommand::WakeDelivery(message_id, contact_id)) => {
                if let Err(error) = communication_lifecycle.set_dormant(false) {
                    let now = current_timestamp().unwrap_or(Timestamp::UNIX_EPOCH);
                    record(
                        diagnostics,
                        sequence,
                        now,
                        Component::Communication,
                        HealthState::Degraded,
                        "COMMUNICATION_WAKE_FAILED",
                    );
                    // WakeDelivery has no response channel: retain the
                    // durable job and let the next provider/network event
                    // retry it instead of silently losing the wake failure.
                    eprintln!("torca-runtime: provider wake failed for delivery: {error}");
                }
                communication.wake_delivery();
                work.active_delivery_leases.insert(message_id);
                let resolved_contact = if let Some(contact_id) = contact_id {
                    Some(contact_id)
                } else if let Ok(Some(contact_id)) =
                    engine.message_contact(MessageId::from_opaque(message_id))
                {
                    // Legacy callers may not have sent the recipient hint.
                    // Resolve only this durable message; do not refresh or
                    // scan the contact projection as a fallback.
                    Some(contact_id)
                } else {
                    None
                };
                if let Some(contact_id) = resolved_contact {
                    work.active_delivery_contacts.insert(message_id, contact_id);
                    // A durable message is an explicit peer demand. Prime
                    // only its relationship; warming every known contact
                    // would defeat the lazy connectivity policy.
                    communication.prime_contact(contact_id);
                }
                acquire_delivery_lease(policy, message_id);
            }
            RuntimeWait::Command(RuntimeCommand::ReleaseDelivery(message_id)) => {
                work.active_delivery_leases.remove(&message_id);
                work.active_delivery_contacts.remove(&message_id);
                policy.release_lease(delivery_lease_owner(message_id));
            }
            RuntimeWait::Command(command) => {
                if command_requires_network(&command) {
                    if let Err(error) = communication_lifecycle.set_dormant(false) {
                        let now = current_timestamp().unwrap_or(Timestamp::UNIX_EPOCH);
                        record(
                            diagnostics,
                            sequence,
                            now,
                            Component::Communication,
                            HealthState::Degraded,
                            "COMMUNICATION_WAKE_FAILED",
                        );
                        eprintln!("torca-runtime: provider wake failed before command: {error}");
                    }
                }
                if command_writes_database(&command) {
                    diagnostics.count(RuntimeCounter::DbWrite);
                }
                let now = current_timestamp().unwrap_or(Timestamp::UNIX_EPOCH);
                handle_command(
                    command,
                    engine,
                    pairing,
                    communication,
                    communication_lifecycle,
                    &health.probes,
                    rendezvous_info.as_ref(),
                    rendezvous_health.as_ref(),
                    &mut health.transport_activity,
                    &connectivity,
                    policy,
                    &mut work.active_attachment_leases,
                    &mut work.active_attachment_contacts,
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
        if !work.foreground
            && scheduling
                .background_grace_deadline
                .is_some_and(|deadline| deadline <= std::time::Instant::now())
        {
            scheduling.background_grace_deadline = None;
            // Viewport/focus leases are deliberately not sufficient here.
            // Only durable feature work keeps the selected provider active beyond the short
            // transition grace period.
            if work.communication_dormancy_allowed
                && !policy.has_durable_lease(std::time::Instant::now())
            {
                let _ = communication_lifecycle.set_dormant(true);
                record(
                    diagnostics,
                    sequence,
                    now,
                    Component::Communication,
                    HealthState::Stopped,
                    "BACKGROUND_GRACE_EXPIRED",
                );
            }
        }
        // A command is allowed to reconcile the immediately affected state.
        // Deadline wakes, however, execute only the owning subsystem.  This
        // prevents an unrelated provider deadline from scanning contacts, probing
        // peers or driving attachment state in the background.
        let run_health = command_health
            || due_sources.contains(&RuntimeWakeSource::ProviderDeadline)
            || due_sources.contains(&RuntimeWakeSource::PairingDeadline)
            || due_sources.contains(&RuntimeWakeSource::RelayDeadline)
            || due_sources.contains(&RuntimeWakeSource::LeaseExpiry);
        let run_delivery =
            command_delivery || due_sources.contains(&RuntimeWakeSource::DeliveryDeadline);
        let run_radio = due_sources.contains(&RuntimeWakeSource::RadioDeadline);
        let run_peer = command_peer || due_sources.contains(&RuntimeWakeSource::PeerDeadline);
        if run_health {
            maintain_runtime_health(
                pairing,
                communication_lifecycle,
                rendezvous_health.as_ref(),
                policy,
                &mut health,
                &mut work,
                &mut counters,
                diagnostics,
                sequence,
                &connectivity,
                now,
            );
        }
        let communication_result = if run_delivery {
            maintain_delivery_state(
                engine,
                communication,
                policy,
                &mut work,
                &mut counters,
                diagnostics,
                now,
            )
        } else {
            Ok(())
        };
        let active_transport = if run_peer {
            maintain_peer_state(
                communication,
                policy,
                &mut health,
                work.battery_policy,
                &mut scheduling,
                diagnostics,
                sequence,
                &connectivity,
                now,
                communication_result,
            )
        } else {
            false
        };
        if run_radio {
            if let Err(error) = communication.maintain_radio(now) {
                record(
                    diagnostics,
                    sequence,
                    now,
                    Component::Peer,
                    HealthState::Degraded,
                    "RADIO_MAINTENANCE_FAILED",
                );
                eprintln!("torca-runtime: radio maintenance failed: {error}");
            }
        }
        // Reachability probing is a real demand, not a property of the
        // provider name.  Foreground use, explicit AlwaysAvailable mode and
        // durable work may justify it; an Automatic/BatterySaver background
        // runtime must not wake Iroh's relay/discovery machinery merely to
        // refresh a cosmetic status.  The lifecycle implementation coalesces
        // repeated values and only starts a provider-owned probe on a rising
        // edge or a network-generation event.
        let effective_policy =
            work.battery_preferences.effective(work.system_energy, false);
        let reachability_demand = work.foreground
            || matches!(
                effective_policy.profile,
                torca_runtime_policy::BatteryProfile::AlwaysAvailable
                    | torca_runtime_policy::BatteryProfile::Diagnostics
            )
            || policy.has_durable_lease(std::time::Instant::now());
        communication_lifecycle.set_reachability_demand(reachability_demand);
        // Refresh provider-owned, redaction-safe runtime facts whenever the
        // actor processes a wake. This keeps diagnostics aligned with Iroh
        // endpoint/network generations without adding a polling timer.
        diagnostics.set_provider_runtime(communication_lifecycle.runtime_diagnostics());
        update_runtime_schedule(
            communication_lifecycle,
            pairing,
            communication,
            policy,
            &mut scheduling,
            diagnostics,
            active_transport,
            now,
        );
    }
}

/// Detects an accidental zero-deadline/runtime-mailbox spin without adding a
/// periodic diagnostic timer.  It only emits a redaction-safe line after a
/// sustained burst of sub-millisecond turns, so normal interactive traffic is
/// unaffected.
#[derive(Default)]
struct RuntimeHotLoopGuard {
    rapid_turns: u32,
    last_reported_turns: u64,
    total_turns: u64,
}

impl RuntimeHotLoopGuard {
    fn observe_wait(
        &mut self,
        waited: std::time::Duration,
        wait: &RuntimeWait,
        scheduling: &RuntimeSchedulingState,
    ) {
        self.total_turns = self.total_turns.saturating_add(1);
        if waited <= std::time::Duration::from_millis(2) {
            self.rapid_turns = self.rapid_turns.saturating_add(1);
        } else {
            self.rapid_turns = 0;
        }
        if self.rapid_turns < 100 || self.total_turns.saturating_sub(self.last_reported_turns) < 100 {
            return;
        }
        self.last_reported_turns = self.total_turns;
        let wait_kind = match wait {
            RuntimeWait::Command(command) => command_kind(command),
            RuntimeWait::Timeout => "deadline",
            RuntimeWait::Closed => "closed",
        };
        let next_deadline_ms = scheduling
            .next_deadline()
            .map(|deadline| deadline.saturating_duration_since(std::time::Instant::now()).as_millis());
        eprintln!(
            "torca-runtime: hot-loop suspected turns={} rapid_turns={} wait={} next_deadline_ms={:?}",
            self.total_turns, self.rapid_turns, wait_kind, next_deadline_ms
        );
    }
}

fn command_kind(command: &RuntimeCommand) -> &'static str {
    match command {
        RuntimeCommand::Wake(_) => "wake",
        RuntimeCommand::WakeDelivery(..) => "wake_delivery",
        RuntimeCommand::NetworkChanged => "network_changed",
        RuntimeCommand::SetAttention(_) => "attention",
        RuntimeCommand::SetForeground(..) => "foreground",
        RuntimeCommand::SetBatteryPolicyInputs(..) => "battery_policy",
        RuntimeCommand::SetRadioDemand(..) => "radio_demand",
        RuntimeCommand::SetInstantContactDemand(..) => "instant_demand",
        RuntimeCommand::SetRadioTransmission(..) => "radio_transmission",
        RuntimeCommand::Shutdown(_) => "shutdown",
        _ => "command",
    }
}
