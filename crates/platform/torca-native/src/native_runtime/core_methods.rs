impl TorcaRuntime {

fn read_models(&self) -> &ApplicationReadModels {
    self.application_runtime
        .read_models()
        .expect("production composition always installs application read-model ports")
}

pub(crate) fn new(event_hub: Arc<RuntimeEventHub>) -> Result<Self, String> {
    let logger = open_startup_logger();
    let parts = match spawn_production_engine() {
        Ok(parts) => parts,
        Err(error) => {
            if let Some(logger) = logger.as_ref() {
                let _ = logger.event(
                    "bootstrap",
                    Level::Error,
                    "composition",
                    "COMPOSITION_FAILED",
                    &format!("Native engine composition failed: {error}"),
                );
            }
            return Err(format!("native engine composition failed: {error}"));
        }
    };
    let application = parts.application.clone();
    let mut application_runtime = ClientApplicationRuntime::new(application.clone());
    application_runtime.attach_read_models(parts.read_models);
    application_runtime.attach_pending_store(parts.pending);
    let notifications_enabled = application_runtime
        .read_models()
        .and_then(|models| models.settings.notifications_enabled().ok())
        .unwrap_or(true);
    let read_receipts_enabled = application_runtime
        .read_models()
        .and_then(|models| models.settings.read_receipts_enabled().ok())
        .unwrap_or(true);
    let battery_preferences = application_runtime
        .read_models()
        .and_then(|models| models.settings.battery_preferences().ok())
        .unwrap_or_default();
    let read_receipt_policy = ReadReceiptPolicy::new(read_receipts_enabled);
    let contact_notification_seen = application_runtime
        .snapshot_context()
        .map(bridge_snapshot_from_application)
        .map(|snapshot| snapshot.contacts.into_iter().map(|contact| contact.id).collect())
        .unwrap_or_default();
    let mut runtime = Self {
        application_runtime,
        event_hub,
        actor: Some(parts.actor),
        host: None,
        host_start: None,
        host_start_started_at: None,
        host_start_started_at_ms: None,
        host_last_progress_at_ms: None,
        host_progress: 0,
        host_attempt: 0,
        host_status_code: None,
        host_status_summary: None,
        host_onion_started_at_ms: None,
        host_onion_last_progress_at_ms: None,
        host_onion_progress: 0,
        host_onion_attempt: 0,
        host_onion_status_code: None,
        host_onion_status_summary: None,
        host_onion_retry_at: None,
        host_start_deadline: None,
        host_retry_at: None,
        host_failures: 0,
        host_state_hint: TorState::Stopped,
        network_changed_pending: false,
        last_onion_log_state: None,
        last_relay_log_state: None,
        last_peer_log_state: HashMap::new(),
        last_message_log_state: HashMap::new(),
        last_attachment_log_state: HashMap::new(),
        last_radio_log_state: HashMap::new(),
        network_ready_logged: false,
        last_result_json: success_result("initialized"),
        last_error_descriptor: None,
        snapshot_json: empty_snapshot_json(),
        snapshot_value: serde_json::Value::Null,
        query_json: "{\"messages\":[],\"hasMore\":false}".into(),
        logger,
        notification_seen: HashMap::new(),
        contact_notification_seen,
        pairing_notification_seen: HashSet::new(),
        notification_cursor: 0,
        notification_events: Vec::new(),
        notifications_enabled,
        read_receipts_enabled,
        battery_policy: BatteryPolicyState::new(
            battery_preferences,
            SystemEnergyState::default(),
        ),
        read_receipt_policy,
    };
    runtime.log(
        "runtime",
        Level::Info,
        "native",
        "RUNTIME_INITIALIZED",
        "Native runtime initialized",
    );
    if !runtime.has_identity().map_err(|_| "read local identity failed".to_owned())? {
        runtime.log(
            "bootstrap",
            Level::Info,
            "identity",
            "IDENTITY_CREATING",
            "No local identity found; creating device identity",
        );
        runtime
            .create_bootstrap_identity()
            .map_err(|_| "create bootstrap device identity failed".to_owned())?;
        runtime.log(
            "bootstrap",
            Level::Info,
            "identity",
            "IDENTITY_CREATED",
            "Device identity created",
        );
    }
    runtime.begin_runtime_start();
    if runtime.refresh_snapshot() != ABI_OK {
        runtime.log(
            "runtime",
            Level::Error,
            "native",
            "SNAPSHOT_UNAVAILABLE",
            "Initial native snapshot unavailable",
        );
        eprintln!("Torca native engine initialization failed: initial snapshot unavailable");
        return Err("initial native snapshot unavailable".to_owned());
    }
    let instant_contacts = runtime
        .application_runtime
        .snapshot_context()
        .map(|context| {
            context
                .application
                .contacts
                .iter()
                .filter_map(|contact| {
                    let id = contact.id();
                    runtime
                        .read_models()
                        .settings
                        .contact_availability(id)
                        .ok()
                        .filter(|mode| {
                            *mode == torca_battery::ContactAvailabilityMode::Instant
                        })
                        .map(|_| id)
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    for contact_id in instant_contacts {
        runtime.application_runtime.set_instant_contact_demand(contact_id, true);
    }
    runtime.log(
        "bootstrap",
        Level::Info,
        "runtime",
        "LOCAL_READY",
        "Local runtime and initial snapshot are ready",
    );
    Ok(runtime)
}

pub(crate) fn execute_with_request_id(
    &mut self,
    command: torca_contract::BridgeCommand,
    request_id: &str,
) -> i32 {
    self.last_error_descriptor = None;
    if self.is_closed() {
        self.last_result_json = error_result("native engine is closed");
        return ABI_CLOSED;
    }
    self.advance_runtime_start();
    if let torca_contract::BridgeCommand::SetNotifications { enabled } = &command {
        self.notifications_enabled = *enabled;
        if let Ok(elapsed) = SystemTime::now().duration_since(UNIX_EPOCH) {
            let _ = self.read_models().settings.set_notifications_enabled(
                *enabled,
                i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX),
            );
        }
    }
    if let torca_contract::BridgeCommand::SetReadReceipts { enabled } = &command {
        let now = unix_time_ms().unwrap_or(0);
        if self.read_models().settings.set_read_receipts_enabled(*enabled, now).is_err() {
            self.last_result_json = error_result("read receipt setting storage unavailable");
            return ABI_ERROR;
        }
        self.read_receipts_enabled = *enabled;
        self.read_receipt_policy.set_enabled(*enabled);
    }
    if let torca_contract::BridgeCommand::SetBatteryPreferences {
        mode,
        background_sync,
        allow_delayed_background_delivery,
        metered_transfers,
        visual_activity,
    } = &command
    {
        let preferences = BatteryPreferences::from_wire(
            mode,
            background_sync,
            *allow_delayed_background_delivery,
            metered_transfers,
            visual_activity,
        );
        let now = unix_time_ms().unwrap_or(0);
        if self.read_models().settings.set_battery_preferences(preferences, now).is_err() {
            self.last_result_json = error_result("battery preference storage unavailable");
            return ABI_ERROR;
        }
        self.battery_policy.preferences = preferences;
        self.application_runtime.set_battery_profile(profile_for_preferences(preferences));
        self.application_runtime.set_metered_transfer_policy(preferences.metered_transfers);
    }
    if let torca_contract::BridgeCommand::SetContactAvailability {
        contact_id_hex,
        mode,
    } = &command
    {
        let Ok(contact_id) = contact_id_hex.parse::<OpaqueId>() else {
            self.last_result_json = error_result("invalid contact availability id");
            return ABI_ERROR;
        };
        let contact_id = torca_contacts::ContactId::from_opaque(contact_id);
        let availability = torca_battery::ContactAvailabilityMode::from_wire(mode);
        let now = unix_time_ms().unwrap_or(0);
        if self
            .read_models()
            .settings
            .set_contact_availability(contact_id, availability, now)
            .is_err()
        {
            self.last_result_json = error_result("contact availability storage unavailable");
            return ABI_ERROR;
        }
    }
    if matches!(&command, torca_contract::BridgeCommand::AcknowledgeNewContacts) {
        let now = unix_time_ms().unwrap_or(0);
        if self.read_models().settings.acknowledge_new_contacts(now).is_err() {
            self.last_result_json = error_result("contact acknowledgement storage unavailable");
            return ABI_ERROR;
        }
    }
    let is_profile = matches!(&command, torca_contract::BridgeCommand::UpdateProfile { .. });
    let pairing_operation = match &command {
        torca_contract::BridgeCommand::CreatePairing { session_id_hex } => {
            Some(("pairing.create", session_id_hex.clone()))
        }
        torca_contract::BridgeCommand::JoinPairing { session_id_hex, .. } => {
            Some(("pairing.join", session_id_hex.clone()))
        }
        torca_contract::BridgeCommand::ApprovePairing { session_id_hex } => {
            Some(("pairing.approve", session_id_hex.clone()))
        }
        torca_contract::BridgeCommand::RejectPairing { session_id_hex } => {
            Some(("pairing.reject", session_id_hex.clone()))
        }
        torca_contract::BridgeCommand::CancelPairing { session_id_hex } => {
            Some(("pairing.cancel", session_id_hex.clone()))
        }
        _ => None,
    };
    let radio_operation = match &command {
        torca_contract::BridgeCommand::SetRadioEnabled { contact_id_hex, .. } => {
            Some(("radio.set_enabled".to_owned(), contact_id_hex.clone()))
        }
        torca_contract::BridgeCommand::BeginRadioTransmission { contact_id_hex } => {
            Some(("radio.begin_transmission".to_owned(), contact_id_hex.clone()))
        }
        torca_contract::BridgeCommand::EndRadioTransmission { contact_id_hex } => {
            Some(("radio.end_transmission".to_owned(), contact_id_hex.clone()))
        }
        _ => None,
    };
    let message_operation = match &command {
        torca_contract::BridgeCommand::QueueMessage { message_id_hex, .. } => {
            Some(("message.queue".to_owned(), message_id_hex.clone()))
        }
        torca_contract::BridgeCommand::RetryMessage { message_id_hex, .. } => {
            Some(("message.retry".to_owned(), message_id_hex.clone()))
        }
        torca_contract::BridgeCommand::CancelMessage { message_id_hex, .. } => {
            Some(("message.cancel".to_owned(), message_id_hex.clone()))
        }
        _ => None,
    };
    if is_profile {
        self.log_profile(request_id, "PROFILE_REQUEST_RECEIVED");
        self.log_profile(request_id, "PROFILE_COMMAND_QUEUED");
        self.log_profile(request_id, "PROFILE_COMMAND_STARTED");
        self.log_profile(request_id, "PROFILE_STORAGE_STARTED");
    }
    if let Some((operation, session_id)) = &pairing_operation {
        self.log_pairing(request_id, operation, session_id, "PAIRING_REQUEST_STARTED", None);
    }
    if let Some((operation, contact_id)) = radio_operation.as_ref() {
        self.log(
            "radio",
            Level::Info,
            "command",
            "RADIO_COMMAND_STARTED",
            &format!("operation={operation} contact={contact_id} requestId={request_id}"),
        );
    }
    if let Some((operation, message_id)) = message_operation.as_ref() {
        self.log(
            "messaging",
            Level::Info,
            "command",
            "MESSAGE_COMMAND_STARTED",
            &format!("operation={operation} message={} requestId={request_id}", message_id),
        );
    }

    let application_result = match decode_application_command(command) {
        Ok(command) => self.application_runtime.execute(command),
        Err(error) => Err(ApplicationError::invalid_input(error)),
    };
    self.last_error_descriptor = application_result.as_ref().err().map(ClassifiedError::descriptor);
    let result = bridge_result_from_application(application_result);
    if !result.ok {
        if is_profile {
            self.log_profile(request_id, "PROFILE_STORAGE_FAILED");
        }
        if let (Some(logger), Some((operation, contact_id))) = (&self.logger, radio_operation.as_ref()) {
            let context = json!({
                "operation": operation,
                "contactId": contact_id,
                "requestId": request_id,
                "errorCode": &result.error_code,
            })
            .to_string();
            let _ = logger.event_with_context(
                "radio",
                Level::Error,
                "command",
                "BRIDGE_COMMAND_FAILED",
                "Radio command rejected by native engine",
                Some(&context),
            );
        } else {
            self.log(
                "bridge",
                Level::Error,
                "command",
                "BRIDGE_COMMAND_FAILED",
                "Bridge command rejected by native engine",
            );
        }
        if let Some((operation, session_id)) = &pairing_operation {
            self.log_pairing(
                request_id,
                operation,
                session_id,
                "PAIRING_REQUEST_FAILED",
                result.error_code.as_deref(),
            );
        }
        if let Some((operation, message_id)) = message_operation.as_ref() {
            self.log(
                "messaging",
                Level::Error,
                "command",
                "MESSAGE_COMMAND_FAILED",
                &format!(
                    "operation={operation} message={} requestId={} errorCode={}",
                    message_id,
                    request_id,
                    result.error_code.as_deref().unwrap_or("unknown"),
                ),
            );
        }
        self.last_result_json = bridge_result_json(&result);
        return ABI_ERROR;
    }
    if is_profile {
        self.log_profile(request_id, "PROFILE_STORAGE_COMMITTED");
    }
    self.last_error_descriptor = None;
    self.last_result_json = bridge_result_json(&result);
    let _ = self.refresh_snapshot();
    if is_profile {
        self.log_profile(request_id, "PROFILE_SNAPSHOT_PUBLISHED");
        self.log_profile(request_id, "PROFILE_REQUEST_SUCCEEDED");
    }
    if let Some((operation, session_id)) = &pairing_operation {
        self.log_pairing(
            request_id,
            operation,
            session_id,
            "PAIRING_REQUEST_SUCCEEDED",
            Some(result.kind.as_str()),
        );
    }
    if let Some((operation, contact_id)) = radio_operation.as_ref() {
        self.log(
            "radio",
            Level::Info,
            "command",
            "RADIO_COMMAND_SUCCEEDED",
            &format!("operation={operation} contact={contact_id} requestId={request_id}"),
        );
    }
    if let Some((operation, message_id)) = message_operation.as_ref() {
        self.log(
            "messaging",
            Level::Info,
            "command",
            "MESSAGE_COMMAND_SUCCEEDED",
            &format!("operation={operation} message={} requestId={request_id}", message_id),
        );
    }
    ABI_OK
}

}
