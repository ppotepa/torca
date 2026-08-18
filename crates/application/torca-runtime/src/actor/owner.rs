// Responsibility: runtime owner and linear event loop.

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
    let mut health = RuntimeHealthState::default();
    let mut work = RuntimeWorkState::new();
    let mut counters = RuntimeCounters::default();
    let mut scheduling = RuntimeSchedulingState::new();

    loop {
        match wait_for_runtime_command(&receiver, scheduling.next_maintenance_at) {
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
                if let Some(owner) = work.attention_owner.take() {
                    policy.release_lease(owner);
                }
                for owner in work.visible_contact_leases.values().copied() {
                    policy.release_lease(owner);
                }
                work.visible_contact_leases.clear();
                let visible_expiry = now + Duration::from_secs(3 * 60);
                for opaque_id in context.visible_contact_ids.iter().copied() {
                    let contact_id = ContactId::from_opaque(opaque_id);
                    let owner = visible_contact_lease_owner(contact_id, context.generation);
                    acquire_visible_contact_lease(policy, contact_id, owner, visible_expiry);
                    work.visible_contact_leases.insert(contact_id, owner);
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
                        policy.acquire_focus(peer, owner, expires_at, now);
                    } else {
                        policy.acquire_lease(demand);
                    }
                    work.attention_owner = Some(owner);
                }
                policy.apply(PolicyEvent::Attention(context), now);
            }
            RuntimeWait::Command(RuntimeCommand::NetworkChanged) => {
                work.refresh_contacts = true;
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
            RuntimeWait::Command(RuntimeCommand::SetInstantContactDemand(contact_id, enabled)) => {
                if enabled {
                    let _ = tor.set_dormant(false);
                    acquire_instant_contact_lease(policy, contact_id);
                } else {
                    policy.release_lease(instant_contact_lease_owner(contact_id));
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
                work.battery_policy.set_profile(profile);
                communication.set_battery_policy(
                    profile,
                    work.metered_transfers,
                    work.metered_network,
                );
                diagnostics.set_battery_profile(profile);
            }
            RuntimeWait::Command(RuntimeCommand::SetBackgroundSync(cadence)) => {
                work.background_sync = cadence;
                scheduling.background_sync_deadline = None;
            }
            RuntimeWait::Command(RuntimeCommand::SetForeground(foreground)) => {
                work.foreground = foreground;
                if foreground {
                    scheduling.background_sync_deadline = None;
                }
            }
            RuntimeWait::Command(RuntimeCommand::SetMeteredNetwork(metered)) => {
                work.metered_network = metered;
                communication.set_battery_policy(
                    work.battery_policy.profile(),
                    work.metered_transfers,
                    work.metered_network,
                );
            }
            RuntimeWait::Command(RuntimeCommand::SetMeteredTransferPolicy(transfer_policy)) => {
                work.metered_transfers = transfer_policy;
                communication.set_battery_policy(
                    work.battery_policy.profile(),
                    work.metered_transfers,
                    work.metered_network,
                );
            }
            RuntimeWait::Command(RuntimeCommand::SetTorDormancy(dormant)) => {
                let _ = tor.set_dormant(dormant);
            }
            RuntimeWait::Command(RuntimeCommand::WakeDelivery(message_id)) => {
                let _ = tor.set_dormant(false);
                work.active_delivery_leases.insert(message_id);
                acquire_delivery_lease(policy, message_id);
                work.refresh_contacts = true;
            }
            RuntimeWait::Command(RuntimeCommand::ReleaseDelivery(message_id)) => {
                work.active_delivery_leases.remove(&message_id);
                policy.release_lease(delivery_lease_owner(message_id));
            }
            RuntimeWait::Command(command) => {
                if command_requires_network(&command) {
                    let _ = tor.set_dormant(false);
                }
                if command_writes_database(&command) {
                    diagnostics.count(RuntimeCounter::DbWrite);
                }
                work.refresh_contacts = true;
                let now = current_timestamp().unwrap_or(Timestamp::UNIX_EPOCH);
                handle_command(
                    command,
                    engine,
                    pairing,
                    communication,
                    tor,
                    &health.probes,
                    relay_info.as_ref(),
                    relay_health.as_ref(),
                    &mut health.transport_activity,
                    &connectivity,
                    policy,
                    &mut work.active_attachment_leases,
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
            // Viewport/focus leases keep peer state useful, but they are not
            // sufficient reason to keep the whole Tor stack awake while the
            // host is backgrounded. Durable delivery/radio/pairing leases
            // still prevent dormancy through this policy query.
            && !policy.has_durable_lease(std::time::Instant::now())
            && !matches!(
                work.background_sync,
                torca_battery::BackgroundSyncCadence::Instant
                    | torca_battery::BackgroundSyncCadence::OnOpen
            )
        {
            let _ = tor.set_dormant(true);
        }
        if scheduling
            .background_sync_deadline
            .is_some_and(|deadline| deadline <= std::time::Instant::now())
        {
            if let Some(interval) = work.background_sync.approximate_interval() {
                scheduling.background_sync_deadline =
                    Some(std::time::Instant::now() + interval);
                let _ = tor.set_dormant(false);
                acquire_background_sync_lease(policy);
                record(
                    diagnostics,
                    sequence,
                    now,
                    Component::Engine,
                    HealthState::Starting,
                    "BACKGROUND_SYNC_WAKE",
                );
            } else {
                scheduling.background_sync_deadline = None;
            }
        }
        maintain_runtime_health(
            engine,
            pairing,
            tor,
            relay_health.as_ref(),
            policy,
            &mut health,
            &mut work,
            &mut counters,
            diagnostics,
            sequence,
            &connectivity,
            critical_lease.as_ref(),
            now,
        );
        let communication_result = maintain_delivery_state(
            engine,
            communication,
            policy,
            &mut work,
            &mut counters,
            diagnostics,
            now,
        );
        let active_transport = maintain_peer_state(
            communication,
            policy,
            &mut health,
            &work,
            &mut scheduling,
            diagnostics,
            sequence,
            &connectivity,
            now,
            communication_result,
        );
        update_runtime_schedule(
            tor,
            pairing,
            communication,
            policy,
            work.background_sync,
            work.foreground,
            &mut scheduling,
            diagnostics,
            active_transport,
            now,
        );
    }
}
