impl TorcaRuntime {

pub(crate) fn close(&mut self) -> i32 {
    self.log(
        "runtime",
        Level::Info,
        "native",
        "RUNTIME_STOPPING",
        "Native runtime shutdown requested",
    );
    self.host_retry_at = None;
    self.host_failures = 0;
    self.host_state_hint = TorState::Stopped;
    self.host_start = None;
    self.host_start_started_at = None;
    self.host_start_started_at_ms = None;
    self.host_last_progress_at_ms = None;
    self.host_progress = 0;
    self.host_attempt = 0;
    self.host_status_code = None;
    self.host_status_summary = None;
    self.host_onion_started_at_ms = None;
    self.host_onion_last_progress_at_ms = None;
    self.host_onion_progress = 0;
    self.host_onion_attempt = 0;
    self.host_onion_status_code = None;
    self.host_onion_status_summary = None;
    self.host_onion_retry_at = None;
    self.host_start_deadline = None;
    if let Some(host) = self.host.take()
        && host.shutdown().is_err()
    {
        self.last_result_json = error_result("secure runtime shutdown failed");
    }
    let Some(actor) = self.actor.take() else {
        if let Some(logger) = &self.logger {
            let _ = logger.finish("completed", "runtime already stopped");
        }
        return ABI_OK;
    };
    match actor.shutdown() {
        Ok(()) => {
            if let Some(logger) = &self.logger {
                let _ = logger.finish("completed", "runtime stopped");
            }
            ABI_OK
        }
        Err(error) => {
            self.log(
                "runtime",
                Level::Error,
                "native",
                "RUNTIME_STOP_FAILED",
                &error.to_string(),
            );
            if let Some(logger) = &self.logger {
                let _ = logger.finish("failed", &error.to_string());
            }
            self.last_result_json = error_result(&error.to_string());
            ABI_ERROR
        }
    }
}

pub(crate) fn notification_events_json(&mut self, after_cursor: u64) -> i32 {
    let _ = self.collect_notification_events();
    let events = self
        .notification_events
        .iter()
        .filter(|event| event.cursor > after_cursor)
        .map(notification_event_json)
        .collect::<Vec<_>>();
    self.query_json = serde_json::json!({
        "afterCursor": after_cursor,
        "events": events,
    })
    .to_string();
    ABI_OK
}

pub(crate) fn diagnostics_json(&mut self) -> i32 {
    match self.diagnostic_snapshot_json() {
        Ok(diagnostics) => {
            self.query_json = diagnostics;
            ABI_OK
        }
        Err(error) => {
            self.last_result_json = error_result(&error);
            ABI_ERROR
        }
    }
}

/// Persists one explicitly user-requested, redacted diagnostics snapshot.
///
/// Incident capture is deliberately command-driven: it does not create a
/// timer, a network request, or a new native worker. The existing run logger
/// owns retention and redaction for the resulting small JSON file.
pub(crate) fn mark_incident(&mut self) -> Result<(), String> {
    let diagnostics = self.diagnostic_snapshot_json()?;
    let marked_at_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|elapsed| i64::try_from(elapsed.as_millis()).ok())
        .unwrap_or(0);
    let Some(logger) = &self.logger else {
        return Err("local diagnostics logger unavailable".into());
    };
    logger
        .write_incident_bundle(&format!("incident-{marked_at_ms}"), &diagnostics)
        .map_err(|error| format!("incident bundle write failed: {error}"))?;
    let context = json!({ "markedAtMs": marked_at_ms }).to_string();
    let _ = logger.event_with_context(
        "diagnostics",
        torca_logging::Level::Info,
        "incident",
        "DIAGNOSTICS_INCIDENT_MARKED",
        "User marked a local diagnostics incident",
        Some(&context),
    );
    Ok(())
}

fn diagnostic_snapshot_json(&mut self) -> Result<String, String> {
    let diagnostics = self
        .application_runtime
        .diagnostics_json()
        .map_err(|error| error.to_string())?;
    let mut value = serde_json::from_str::<Value>(&diagnostics)
        .unwrap_or_else(|_| Value::Object(serde_json::Map::new()));
    if let Some(counters) = value.get_mut("counters").and_then(Value::as_object_mut)
        && let Some(stats) = self.event_hub.stats()
    {
        counters.insert("ffiWakes".into(), Value::from(stats.wakeups));
    }
    if let Some(counters) = value.get_mut("counters").and_then(Value::as_object_mut) {
        counters.insert(
            "radioWakeups".into(),
            Value::from(self.application_runtime.radio_wake_count()),
        );
    }
    let (mode, background_sync, allow_delayed, metered_transfers, visual_activity) =
        self.battery_policy.preferences.wire();
    let effective = self.effective_battery_policy(false);
    value["batteryPreferences"] = json!({
        "mode": mode,
        "backgroundSync": background_sync,
        "allowDelayedBackgroundDelivery": allow_delayed,
        "meteredTransfers": metered_transfers,
        "visualActivity": visual_activity,
    });
    value["effectiveBatteryPolicy"] = json!({
        "profile": format!("{:?}", effective.profile),
        "reason": format!("{:?}", effective.reason),
        "torDormancyAllowed": effective.tor_dormancy_allowed,
        "backgroundSync": effective.background_sync.wire(),
        "meteredTransfers": effective.metered_transfers.wire(),
        "visualActivity": effective.visual_activity.wire(),
    });
    Ok(value.to_string())
}

pub(crate) fn avatar_genome_json(&mut self, identity_id: Option<&str>) -> i32 {
    match self.application_runtime.avatar_genome_json(identity_id) {
        Ok(value) => {
            self.query_json = value;
            ABI_OK
        }
        Err(_) => {
            self.query_json = serde_json::json!({
                "errorCode": "avatar_unavailable"
            })
            .to_string();
            ABI_ERROR
        }
    }
}

pub(crate) fn parse_pairing_uri(&mut self, raw_uri: &str) -> i32 {
    let parsed =
        torca_pairing_coordinator::decode_invite_uri(raw_uri).map(Some).or_else(|_| {
            torca_pairing::PairingCode::new(raw_uri).map(|code| (code, None)).map(Some)
        });
    let Ok(Some((code, ticket))) = parsed else {
        self.query_json = "{}".into();
        return ABI_ERROR;
    };
    self.query_json = serde_json::json!({
        "code": code.as_str(),
        "ticket": ticket.as_ref().map(|value| value.as_hex()),
    })
    .to_string();
    ABI_OK
}

pub(crate) fn encode_pairing_uri(&mut self, raw_code: &str) -> i32 {
    let Ok(code) = torca_pairing::PairingCode::new(raw_code) else {
        self.query_json = "{}".into();
        return ABI_ERROR;
    };
    self.query_json = serde_json::json!({
        "uri": torca_pairing_coordinator::encode_invite_uri(&code, None),
    })
    .to_string();
    ABI_OK
}

fn collect_notification_events(&mut self) -> Result<(), ()> {
    if !self.notifications_enabled {
        return Ok(());
    }
    let summaries = self.read_models().history.conversation_summaries().map_err(|_| ())?;
    let snapshot = self
        .application_runtime
        .snapshot_context()
        .map(bridge_snapshot_from_application)
        .map_err(|_| ())?;
    let contact_names = snapshot
        .contacts
        .iter()
        .map(|contact| (contact.id.clone(), contact.display_name.clone()))
        .collect::<HashMap<_, _>>();
    for pairing in &snapshot.pairings {
        if pairing.role != "creator"
            || !matches!(pairing.state.as_str(), "peer_joined" | "awaiting_approval")
            || !self.pairing_notification_seen.insert(pairing.id.clone())
        {
            continue;
        }
        self.notification_cursor = self.notification_cursor.saturating_add(1);
        let event_id = crate::torca_runtime::secure_id_hex()
            .unwrap_or_else(|_| format!("notification-{}", self.notification_cursor));
        let intent = torca_notifications::notification_intent(
            torca_notifications::NotificationEvent::PairingApprovalRequested {
                pairing_id: OpaqueId::from_u128(self.notification_cursor as u128),
            },
            torca_notifications::NotificationPrivacy::Redacted,
            None,
        );
        self.notification_events.push(torca_contract::NotificationEvent {
            cursor: self.notification_cursor,
            event_id,
            kind: "pairing_request".into(),
            resource_id: pairing.id.clone(),
            conversation_id: String::new(),
            contact_display_name: pairing
                .remote_display_name
                .clone()
                .filter(|name| !name.trim().is_empty())
                .unwrap_or_else(|| "New contact request".into()),
            created_at_ms: pairing.expires_at_ms.saturating_sub(5 * 60 * 1_000),
            title: intent.as_ref().map_or_else(|| "Torca".into(), |value| value.title.clone()),
            body: intent.as_ref().map_or_else(String::new, |value| value.body.clone()),
        });
    }
    for contact in &snapshot.contacts {
        if !self.contact_notification_seen.insert(contact.id.clone()) {
            continue;
        }
        self.notification_cursor = self.notification_cursor.saturating_add(1);
        let event_id = crate::torca_runtime::secure_id_hex()
            .unwrap_or_else(|_| format!("notification-{}", self.notification_cursor));
        let conversation_id = snapshot
            .conversations
            .iter()
            .find(|conversation| conversation.contact_id == contact.id)
            .map_or_else(String::new, |conversation| conversation.id.clone());
        let intent = torca_notifications::notification_intent(
            torca_notifications::NotificationEvent::ContactAdded {
                contact_id: OpaqueId::from_u128(self.notification_cursor as u128),
            },
            torca_notifications::NotificationPrivacy::Redacted,
            None,
        );
        self.notification_events.push(torca_contract::NotificationEvent {
            cursor: self.notification_cursor,
            event_id,
            kind: "contact_added".into(),
            resource_id: contact.id.clone(),
            conversation_id,
            contact_display_name: contact.display_name.clone(),
            created_at_ms: contact.created_at_ms,
            title: intent.as_ref().map_or_else(|| "Torca".into(), |value| value.title.clone()),
            body: intent.as_ref().map_or_else(String::new, |value| value.body.clone()),
        });
    }
    for (conversation_id, summary) in summaries {
        let key = conversation_id.to_string();
        let unread = summary.unread_count;
        let previous = self.notification_seen.insert(key.clone(), unread).unwrap_or(0);
        let Some(message) = summary.last_message else { continue };
        if message.direction() != MessageDirection::Inbound || unread <= previous {
            continue;
        }
        self.notification_cursor = self.notification_cursor.saturating_add(1);
        let contact_display_name = snapshot_contact_name(&snapshot, &contact_names, &key);
        let event_id = crate::torca_runtime::secure_id_hex()
            .unwrap_or_else(|_| format!("notification-{}", self.notification_cursor));
        let intent = torca_notifications::notification_intent(
            torca_notifications::NotificationEvent::IncomingMessage {
                contact_id: OpaqueId::from_u128(self.notification_cursor as u128),
                conversation_id: OpaqueId::from_u128(self.notification_cursor as u128),
            },
            torca_notifications::NotificationPrivacy::Redacted,
            None,
        );
        self.notification_events.push(torca_contract::NotificationEvent {
            cursor: self.notification_cursor,
            event_id,
            kind: "message_received".into(),
            resource_id: key.clone(),
            conversation_id: key,
            contact_display_name,
            created_at_ms: message.created_at().to_unix_millis(),
            title: intent.as_ref().map_or_else(|| "Torca".into(), |value| value.title.clone()),
            body: intent.as_ref().map_or_else(String::new, |value| value.body.clone()),
        });
    }
    if self.notification_events.len() > 256 {
        let remove = self.notification_events.len() - 256;
        self.notification_events.drain(..remove);
    }
    Ok(())
}

pub(crate) fn lifecycle(&mut self, event: &str) -> i32 {
    if !matches!(
        event,
        "host_started"
            | "foregrounded"
            | "backgrounded"
            | "network_changed"
            | "low_memory"
            | "power_saver_on"
            | "power_saver_off"
            | "charging_on"
            | "charging_off"
            | "metered_network_on"
            | "metered_network_off"
            | "network_validated"
            | "network_unvalidated"
            | "data_stall_on"
            | "data_stall_off"
            | "terminating"
    ) {
        self.last_result_json = error_result("unknown lifecycle event");
        return ABI_ERROR;
    }
    self.log("runtime", Level::Info, "lifecycle", "LIFECYCLE_EVENT", event);
    let radio_lifecycle = match event {
        "foregrounded" | "host_started" => {
            Some(torca_radio_coordinator::HostRadioLifecycle::Foreground)
        }
        "backgrounded" => Some(torca_radio_coordinator::HostRadioLifecycle::Background),
        "terminating" => Some(torca_radio_coordinator::HostRadioLifecycle::Terminating),
        _ => None,
    };
    if let Some(lifecycle) = radio_lifecycle {
        let _ = self.application_runtime.radio_lifecycle(lifecycle);
    }
    self.battery_policy.apply_system_event(event);
    if event == "foregrounded" || event == "host_started" {
        self.application_runtime.set_foreground(true);
    } else if event == "backgrounded" {
        self.application_runtime.set_foreground(false);
    }
    self.apply_battery_policy(false);
    if event == "network_changed" {
        if let Some(host) = &self.host {
            host.network_changed();
        } else {
            self.network_changed_pending = true;
        }
    }
    if event == "terminating" { self.close() } else { ABI_OK }
}

}
