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
        let communication_sender = sender.clone();
        let communication_waker: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
            // Transport activity can advance delivery and peer evidence, but
            // must not become an anonymous "maintain everything" wake.
            let _ = communication_sender.try_send(RuntimeCommand::Wake(vec![
                RuntimeWakeSource::DeliveryDeadline,
                RuntimeWakeSource::PeerDeadline,
            ]));
        });
        let tor_sender = sender.clone();
        let tor_waker: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
            let _ = tor_sender.try_send(RuntimeCommand::Wake(vec![RuntimeWakeSource::TorDeadline]));
        });
        communication.set_waker(communication_waker);
        tor.set_waker(tor_waker);
        let handle = RuntimeHandle { sender: sender.clone() };
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
) {
    let mut health = RuntimeHealthState::default();
    let mut work = RuntimeWorkState::new();
    let mut counters = RuntimeCounters::default();
    let mut scheduling = RuntimeSchedulingState::new();

    loop {
        let runtime_wait = wait_for_runtime_command(&receiver, scheduling.next_deadline());
        let command_wake = matches!(
            &runtime_wait,
            RuntimeWait::Command(command) if command.requires_reconciliation()
        );
        let mut due_sources = matches!(&runtime_wait, RuntimeWait::Timeout)
            .then(|| scheduling.take_due(std::time::Instant::now()))
            .unwrap_or_default();
        if let RuntimeWait::Command(RuntimeCommand::Wake(sources)) = &runtime_wait {
            due_sources.extend(sources.iter().copied());
        }
        if due_sources.is_empty() {
            diagnostics.record_runtime_wake(match &runtime_wait {
                RuntimeWait::Command(RuntimeCommand::NetworkChanged) => RuntimeWakeSource::NetworkChange,
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
                        expires_at: now,
                    }),
                    torca_battery::AttentionSurface::Pairing(_relay) => Some(WorkDemand {
                        scope: ResourceScope::Relay,
                        class: WorkClass::RelayProbe,
                        reason: DemandReason::ActivePairing,
                        owner,
                        expires_at: now,
                    }),
                    _ => None,
                };
                if let Some(demand) = demand {
                    if let torca_battery::AttentionSurface::Conversation(peer) = context.surface {
                        policy.acquire_persistent_focus(peer, owner, now);
                    } else {
                        policy.acquire_persistent_lease(demand);
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
            RuntimeWait::Command(RuntimeCommand::SetForeground(foreground)) => {
                work.foreground = foreground;
                if foreground {
                    scheduling.background_grace_deadline = None;
                    let _ = tor.set_dormant(false);
                } else {
                    release_attention_leases(policy, &mut work);
                    scheduling.background_grace_deadline =
                        Some(std::time::Instant::now() + Duration::from_secs(30));
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
            RuntimeWait::Command(RuntimeCommand::SetTorDormancyAllowed(allowed)) => {
                work.tor_dormancy_allowed = allowed;
                // Removing permission is immediate: users must never remain
                // dormant after selecting a reachability-first policy. Giving
                // permission is deliberately deferred to grace expiry below.
                if !allowed {
                    let _ = tor.set_dormant(false);
                }
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
            && scheduling
                .background_grace_deadline
                .is_some_and(|deadline| deadline <= std::time::Instant::now())
        {
            scheduling.background_grace_deadline = None;
            // Viewport/focus leases are deliberately not sufficient here.
            // Only durable feature work keeps Tor active beyond the short
            // transition grace period.
            if work.tor_dormancy_allowed && !policy.has_durable_lease(std::time::Instant::now()) {
                let _ = tor.set_dormant(true);
                record(
                    diagnostics,
                    sequence,
                    now,
                    Component::Tor,
                    HealthState::Stopped,
                    "BACKGROUND_GRACE_EXPIRED",
                );
            }
        }
        // A command is allowed to reconcile the immediately affected state.
        // Deadline wakes, however, execute only the owning subsystem.  This
        // prevents an unrelated Tor deadline from scanning contacts, probing
        // peers or driving attachment state in the background.
        let run_health = command_wake
            || work.refresh_contacts
            || due_sources.contains(&RuntimeWakeSource::TorDeadline)
            || due_sources.contains(&RuntimeWakeSource::PairingDeadline)
            || due_sources.contains(&RuntimeWakeSource::RelayDeadline)
            || due_sources.contains(&RuntimeWakeSource::LeaseExpiry);
        let run_delivery = command_wake
            || due_sources.contains(&RuntimeWakeSource::DeliveryDeadline);
        let run_peer = command_wake || due_sources.contains(&RuntimeWakeSource::PeerDeadline);
        if run_health {
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
                &work,
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
        update_runtime_schedule(
            tor,
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
