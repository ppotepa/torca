fn request_command<T>(
    sender: &SyncSender<RuntimeCommand>,
    make: impl FnOnce(Sender<Result<T, RuntimeDriverError>>) -> RuntimeCommand,
) -> Result<T, RuntimeDriverError> {
    let (tx, rx) = mpsc::channel();
    send_with_timeout(sender, make(tx))?;
    match rx.recv_timeout(COMMAND_WAIT) {
        Ok(result) => result,
        Err(RecvTimeoutError::Timeout) => Err(RuntimeDriverError::Pending),
        Err(RecvTimeoutError::Disconnected) => Err(RuntimeDriverError::Communication),
    }
}
fn request_query<T>(
    sender: &SyncSender<RuntimeCommand>,
    make: impl FnOnce(Sender<Result<T, RuntimeDriverError>>) -> RuntimeCommand,
) -> Result<T, RuntimeDriverError> {
    let (tx, rx) = mpsc::channel();
    send_with_timeout(sender, make(tx))?;
    match rx.recv_timeout(QUERY_WAIT) {
        Ok(result) => result,
        Err(_) => Err(RuntimeDriverError::Communication),
    }
}
fn request_blocking<T>(
    sender: &SyncSender<RuntimeCommand>,
    make: impl FnOnce(Sender<Result<T, RuntimeDriverError>>) -> RuntimeCommand,
) -> Result<T, RuntimeDriverError> {
    let (tx, rx) = mpsc::channel();
    send_with_timeout(sender, make(tx))?;
    rx.recv_timeout(QUERY_WAIT).map_err(|_| RuntimeDriverError::Communication)?
}

fn send_with_timeout(
    sender: &SyncSender<RuntimeCommand>,
    mut command: RuntimeCommand,
) -> Result<(), RuntimeDriverError> {
    let deadline = std::time::Instant::now() + ENQUEUE_WAIT;
    loop {
        match sender.try_send(command) {
            Ok(()) => return Ok(()),
            Err(TrySendError::Disconnected(_)) => return Err(RuntimeDriverError::Communication),
            Err(TrySendError::Full(returned)) => {
                if std::time::Instant::now() >= deadline {
                    return Err(RuntimeDriverError::Pending);
                }
                command = returned;
                // A full bounded mailbox is backpressure, not a reason to
                // spin the caller while the runtime drains it.
                thread::sleep(std::time::Duration::from_millis(1));
            }
        }
    }
}
fn current_timestamp() -> Result<Timestamp, RuntimeDriverError> {
    let duration =
        SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| RuntimeDriverError::Engine)?;
    let millis = i64::try_from(duration.as_millis()).map_err(|_| RuntimeDriverError::Engine)?;
    Timestamp::from_unix_millis(millis).map_err(|_| RuntimeDriverError::Engine)
}
fn record(
    buffer: &mut DiagnosticBuffer,
    sequence: &mut u128,
    at: Timestamp,
    component: Component,
    state: HealthState,
    code: &str,
) {
    let event_id = OpaqueId::from_u128(*sequence);
    *sequence = sequence.saturating_add(1);
    if let Ok(code) = DiagnosticCode::new(code) {
        buffer.record(DiagnosticEvent { event_id, at, component, state, code, detail: None });
    }
}

fn record_pairing_result<T>(
    result: &Result<T, RuntimeDriverError>,
    action: &str,
    diagnostics: &mut DiagnosticBuffer,
    sequence: &mut u128,
    now: Timestamp,
) {
    let (state, suffix) = match result {
        Ok(_) => (HealthState::Ready, "ACCEPTED"),
        Err(RuntimeDriverError::Pending) => (HealthState::Degraded, "QUEUED"),
        Err(RuntimeDriverError::RouteRefreshRequired) => {
            (HealthState::Degraded, "ROUTE_REFRESH_REQUIRED")
        }
        Err(RuntimeDriverError::Communication) => {
            (HealthState::Degraded, "RETRYING")
        }
        Err(_) => (HealthState::Failed, "FAILED"),
    };
    let code = format!("PAIRING_{action}_{suffix}");
    record(diagnostics, sequence, now, Component::Engine, state, &code);
}

const fn map_communication_health(state: CommunicationState) -> HealthState {
    match state {
        CommunicationState::Stopped => HealthState::Stopped,
        CommunicationState::Starting => HealthState::Starting,
        CommunicationState::Ready => HealthState::Ready,
        CommunicationState::Degraded => HealthState::Degraded,
        CommunicationState::Failed => HealthState::Failed,
    }
}
const fn map_incoming_reachability_health(state: IncomingReachabilityState) -> HealthState {
    match state {
        IncomingReachabilityState::Reachable => HealthState::Ready,
        IncomingReachabilityState::Degraded => HealthState::Degraded,
        IncomingReachabilityState::Failed => HealthState::Failed,
        IncomingReachabilityState::Stopped => HealthState::Stopped,
        IncomingReachabilityState::Unknown | IncomingReachabilityState::Publishing => {
            HealthState::Starting
        }
    }
}
const fn incoming_reachability_event_code(state: IncomingReachabilityState) -> &'static str {
    match state {
        IncomingReachabilityState::Unknown => "INCOMING_REACHABILITY_UNKNOWN",
        IncomingReachabilityState::Publishing => "INCOMING_REACHABILITY_PENDING",
        IncomingReachabilityState::Reachable => "INCOMING_REACHABILITY_READY",
        IncomingReachabilityState::Degraded => "INCOMING_REACHABILITY_DEGRADED",
        IncomingReachabilityState::Failed => "INCOMING_REACHABILITY_FAILED",
        IncomingReachabilityState::Stopped => "INCOMING_REACHABILITY_STOPPED",
    }
}
const fn map_peer_health(state: PeerConnectionStatus) -> HealthState {
    match state {
        PeerConnectionStatus::Ready => HealthState::Ready,
        PeerConnectionStatus::Failed => HealthState::Failed,
        PeerConnectionStatus::Disconnected => HealthState::Stopped,
        PeerConnectionStatus::Connecting
        | PeerConnectionStatus::Handshaking
        | PeerConnectionStatus::Reconnecting => HealthState::Starting,
    }
}
const fn map_probe_health(state: ProbeStatus) -> HealthState {
    match state {
        ProbeStatus::Healthy => HealthState::Ready,
        ProbeStatus::Failed | ProbeStatus::Unreachable | ProbeStatus::Degraded => {
            HealthState::Degraded
        }
        ProbeStatus::Checking | ProbeStatus::Unknown | ProbeStatus::Disabled => {
            HealthState::Starting
        }
    }
}
const fn relay_event_code(state: ProbeStatus) -> &'static str {
    match state {
        ProbeStatus::Healthy => "RELAY_CONNECTED",
        ProbeStatus::Degraded => "RELAY_DEGRADED",
        ProbeStatus::Failed | ProbeStatus::Unreachable => "RELAY_DISCONNECTED",
        ProbeStatus::Checking => "RELAY_CONNECTING",
        ProbeStatus::Unknown | ProbeStatus::Disabled => "RELAY_UNKNOWN",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_scheduler_has_no_application_deadline() {
        let mut scheduler = RuntimeSchedulingState::new();
        let now = std::time::Instant::now() + Duration::from_secs(1);
        // Startup is an explicit one-shot bootstrap wake. Once consumed, an
        // idle runtime has no synthetic application deadline.
        let _ = scheduler.take_due(now);
        assert_eq!(scheduler.next_deadline(), None);
    }

    #[test]
    fn read_only_commands_do_not_request_runtime_maintenance() {
        let (diagnostics, _response) = mpsc::channel();
        let (attachments, _response) = mpsc::channel();
        assert!(!RuntimeCommand::Diagnostics(diagnostics).requires_health_maintenance());
        assert!(!RuntimeCommand::AttachmentSnapshot(attachments).requires_delivery_maintenance());
        let (response, _receiver) = mpsc::channel();
        assert!(RuntimeCommand::SetForeground(false, response).requires_health_maintenance());
    }

    #[test]
    fn battery_input_does_not_start_network_or_delivery_maintenance() {
        let (response, _receiver) = mpsc::channel();
        let command = RuntimeCommand::SetBatteryPolicyInputs(
            BatteryPreferences::default(),
            SystemEnergyState::default(),
            response,
        );
        assert!(!command.requires_health_maintenance());
        assert!(!command.requires_delivery_maintenance());
        assert!(!command.requires_peer_maintenance());
    }

    #[test]
    fn route_refresh_required_has_a_stable_retryable_error_code() {
        let descriptor = RuntimeDriverError::RouteRefreshRequired.descriptor();
        assert_eq!(descriptor.code().as_str(), "runtime.route_refresh_required");
        assert_eq!(descriptor.retry_advice(), RetryAdvice::Immediate);
    }

    #[test]
    fn targeted_delivery_wake_services_only_delivery_and_peer_lanes() {
        let command = RuntimeCommand::WakeDelivery(
            OpaqueId::from_u128(1),
            Some(ContactId::from_opaque(OpaqueId::from_u128(2))),
        );
        assert!(command.requires_delivery_maintenance());
        assert!(command.requires_peer_maintenance());
        assert!(command.requires_health_maintenance());
    }

    #[test]
    fn wake_gate_reset_is_source_specific() {
        let communication = AtomicBool::new(true);
        let lifecycle = AtomicBool::new(true);
        let radio = AtomicBool::new(true);
        clear_wake_gates(
            &[RuntimeWakeSource::ProviderDeadline],
            &communication,
            &lifecycle,
            &radio,
        );
        assert!(communication.load(Ordering::Acquire));
        assert!(!lifecycle.load(Ordering::Acquire));
        assert!(radio.load(Ordering::Acquire));
    }

    #[test]
    fn wake_storm_is_coalesced_and_next_edge_is_preserved() {
        let (sender, receiver) = mpsc::sync_channel(256);
        let gate = AtomicBool::new(false);
        for _ in 0..100_000 {
            enqueue_coalesced_wake(
                &sender,
                &gate,
                vec![RuntimeWakeSource::DeliveryDeadline],
            );
        }
        assert_eq!(receiver.try_iter().count(), 1);
        clear_wake_gates(
            &[RuntimeWakeSource::DeliveryDeadline],
            &gate,
            &AtomicBool::new(false),
            &AtomicBool::new(false),
        );
        enqueue_coalesced_wake(
            &sender,
            &gate,
            vec![RuntimeWakeSource::DeliveryDeadline],
        );
        assert_eq!(receiver.try_iter().count(), 1);
    }

    #[test]
    fn scheduler_records_the_source_for_an_executor_deadline() {
        let now = std::time::Instant::now();
        let mut scheduler = RuntimeSchedulingState::new();
        scheduler.replace_deadlines(
            now,
            [(RuntimeWakeSource::DeliveryDeadline, Some(Duration::from_millis(250)))],
        );
        assert_eq!(scheduler.next_deadline(), Some(now + Duration::from_millis(250)));
    }

    #[test]
    fn scheduler_diagnoses_zero_and_identical_deadline_replacements() {
        let now = std::time::Instant::now();
        let mut scheduler = RuntimeSchedulingState::new();
        scheduler.replace_deadlines(
            now,
            [(RuntimeWakeSource::DeliveryDeadline, Some(Duration::ZERO))],
        );
        scheduler.replace_deadlines(
            now,
            [(RuntimeWakeSource::DeliveryDeadline, Some(Duration::ZERO))],
        );

        let snapshot = scheduler.diagnostic_snapshot(now);
        assert_eq!(snapshot.zero_delay_deadlines, 2);
        assert_eq!(snapshot.identical_deadline_replacements, 1);

        scheduler.replace_deadlines(now, [(RuntimeWakeSource::DeliveryDeadline, None)]);
        scheduler.replace_deadlines(
            now,
            [(RuntimeWakeSource::DeliveryDeadline, Some(Duration::ZERO))],
        );
        assert_eq!(scheduler.diagnostic_snapshot(now).identical_deadline_replacements, 1);
    }

    #[test]
    fn peer_recovery_backoff_is_bounded() {
        assert_eq!(next_peer_recovery_delay(Duration::from_millis(250)), Duration::from_millis(500));
        assert_eq!(next_peer_recovery_delay(Duration::from_secs(5)), Duration::from_secs(5));
    }

    #[test]
    fn peer_recovery_window_is_terminal_and_resets_after_transport_recovers() {
        let now = std::time::Instant::now();
        let mut scheduling = RuntimeSchedulingState::new();
        scheduling.peer_recovery_started_at =
            Some(now.checked_sub(Duration::from_secs(31)).expect("valid test instant"));
        scheduling.peer_recovery_generation = 7;
        scheduling.peer_recovery_attempts = 12;

        assert_eq!(peer_recovery_deadline(&mut scheduling, true, now), None);
        assert!(scheduling.peer_recovery_exhausted);
        assert_eq!(peer_recovery_deadline(&mut scheduling, false, now), None);

        assert_eq!(scheduling.peer_recovery_generation, 7);
        assert_eq!(scheduling.peer_recovery_attempts, 0);
        assert!(!scheduling.peer_recovery_exhausted);

        assert_eq!(
            peer_recovery_deadline(&mut scheduling, true, now),
            Some(Duration::from_millis(250))
        );
        assert_eq!(scheduling.peer_recovery_generation, 8);
        assert_eq!(scheduling.peer_recovery_attempts, 1);
        assert_eq!(peer_recovery_deadline(&mut scheduling, false, now), None);
        assert_eq!(
            peer_recovery_deadline(&mut scheduling, true, now),
            Some(Duration::from_millis(250))
        );
        assert_eq!(scheduling.peer_recovery_generation, 9);

        let future = now.checked_add(Duration::from_secs(3 * 60 * 60)).expect("valid future");
        assert_eq!(peer_recovery_deadline(&mut scheduling, true, future), None);
        assert!(scheduling.peer_recovery_exhausted);
        assert_eq!(scheduling.peer_recovery_attempts, 1);
    }

    #[test]
    fn scheduler_selects_earliest_executor_deadline() {
        let now = std::time::Instant::now();
        let mut scheduler = RuntimeSchedulingState::new();
        scheduler.replace_deadlines(
            now,
            [
                (RuntimeWakeSource::ProviderDeadline, Some(Duration::from_secs(5))),
                (RuntimeWakeSource::PairingDeadline, Some(Duration::from_secs(3))),
                (RuntimeWakeSource::LeaseExpiry, Some(Duration::from_secs(2))),
            ],
        );
        assert_eq!(scheduler.next_deadline(), Some(now + Duration::from_secs(2)));
        assert_eq!(
            scheduler.take_due(now + Duration::from_secs(2)),
            [RuntimeWakeSource::LeaseExpiry].into_iter().collect()
        );
    }

    #[test]
    fn background_grace_is_one_shot_and_never_becomes_periodic_work() {
        let now = std::time::Instant::now();
        let mut scheduler = RuntimeSchedulingState::new();
        scheduler.replace_deadlines(
            now,
            [(RuntimeWakeSource::BackgroundGrace, Some(Duration::from_secs(30)))],
        );

        assert_eq!(
            scheduler.take_due(now + Duration::from_secs(30)),
            [RuntimeWakeSource::BackgroundGrace].into_iter().collect()
        );
        assert_eq!(scheduler.next_deadline(), None);
    }

    #[test]
    fn contact_activity_is_monotonic_and_redacted() {
        let at = Timestamp::from_unix_millis(1).expect("valid timestamp");
        let contact = ContactId::from_opaque(OpaqueId::from_u128(1));
        let mut ledger = TransportActivityLedger::default();
        ledger.mark_peer(contact, at);
        ledger.mark_peer(contact, at);

        let activity = ledger.peers.get(&contact).expect("contact activity");
        assert_eq!(activity.sequence, 2);
        assert_eq!(activity.last_activity_at, Some(at));
    }

    #[test]
    fn radio_demand_lease_authorizes_peer_health_work() {
        let mut policy = RuntimeGovernor::new(std::time::Instant::now());
        let contact = ContactId::from_opaque(OpaqueId::from_u128(42));
        assert!(!has_peer_or_radio_lease(&mut policy, contact));

        acquire_radio_lease(&mut policy, contact);
        assert!(has_peer_or_radio_lease(&mut policy, contact));

        policy.release_lease(radio_lease_owner(contact));
        assert!(!has_peer_or_radio_lease(&mut policy, contact));
    }

    #[test]
    fn radio_transmission_lease_is_separate_from_toggle_lease() {
        let mut policy = RuntimeGovernor::new(std::time::Instant::now());
        let contact = ContactId::from_opaque(OpaqueId::from_u128(7));
        acquire_radio_transmission_lease(&mut policy, contact);
        assert!(policy.has_active_lease(
            ResourceScope::Radio(contact.to_opaque()),
            std::time::Instant::now(),
        ));

        policy.release_lease(radio_transmission_lease_owner(contact));
        assert!(!policy.has_active_lease(
            ResourceScope::Radio(contact.to_opaque()),
            std::time::Instant::now(),
        ));
    }

    #[test]
    fn durable_delivery_lease_never_expires_before_job_completion() {
        let now = std::time::Instant::now();
        let message = OpaqueId::from_u128(81);
        let mut policy = RuntimeGovernor::new(now);
        acquire_delivery_lease(&mut policy, message);

        assert!(policy.has_active_lease(
            ResourceScope::Delivery(message),
            now + Duration::from_secs(24 * 60 * 60),
        ));
        policy.release_lease(delivery_lease_owner(message));
        assert!(!policy.has_active_lease(
            ResourceScope::Delivery(message),
            now + Duration::from_secs(24 * 60 * 60),
        ));
    }
}
