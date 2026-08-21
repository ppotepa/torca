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
                thread::yield_now();
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
        Err(RuntimeDriverError::Communication | RuntimeDriverError::Tor) => {
            (HealthState::Degraded, "RETRYING")
        }
        Err(_) => (HealthState::Failed, "FAILED"),
    };
    let code = format!("PAIRING_{action}_{suffix}");
    record(diagnostics, sequence, now, Component::Engine, state, &code);
}

const fn map_health(state: TorState) -> HealthState {
    match state {
        TorState::Stopped => HealthState::Stopped,
        TorState::Starting => HealthState::Starting,
        TorState::Ready => HealthState::Ready,
        TorState::Degraded => HealthState::Degraded,
        TorState::Failed => HealthState::Failed,
    }
}
const fn map_onion_health(state: OnionServiceState) -> HealthState {
    match state {
        OnionServiceState::Reachable => HealthState::Ready,
        OnionServiceState::Degraded => HealthState::Degraded,
        OnionServiceState::Failed => HealthState::Failed,
        OnionServiceState::Stopped => HealthState::Stopped,
        OnionServiceState::Unknown | OnionServiceState::Publishing => HealthState::Starting,
    }
}
const fn onion_event_code(state: OnionServiceState) -> &'static str {
    match state {
        OnionServiceState::Unknown => "ONION_UNKNOWN",
        OnionServiceState::Publishing => "ONION_PUBLISHING",
        OnionServiceState::Reachable => "ONION_REACHABLE",
        OnionServiceState::Degraded => "ONION_DEGRADED",
        OnionServiceState::Failed => "ONION_FAILED",
        OnionServiceState::Stopped => "ONION_STOPPED",
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
        assert!(RuntimeCommand::SetForeground(false).requires_health_maintenance());
    }

    #[test]
    fn battery_input_does_not_start_network_or_delivery_maintenance() {
        let command = RuntimeCommand::SetBatteryPolicyInputs(
            BatteryPreferences::default(),
            SystemEnergyState::default(),
        );
        assert!(!command.requires_health_maintenance());
        assert!(!command.requires_delivery_maintenance());
        assert!(!command.requires_peer_maintenance());
        assert!(!command.requires_contact_refresh());
    }

    #[test]
    fn diagnostics_and_attachment_queries_do_not_refresh_contacts() {
        let (tx, _rx) = std::sync::mpsc::channel();
        assert!(!RuntimeCommand::Diagnostics(tx).requires_contact_refresh());

        let (tx, _rx) = std::sync::mpsc::channel();
        assert!(!RuntimeCommand::AttachmentSnapshot(tx).requires_contact_refresh());
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
    fn scheduler_selects_earliest_executor_deadline() {
        let now = std::time::Instant::now();
        let mut scheduler = RuntimeSchedulingState::new();
        scheduler.replace_deadlines(
            now,
            [
                (RuntimeWakeSource::TorDeadline, Some(Duration::from_secs(5))),
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
}
