pub(crate) fn refresh_snapshot(&mut self) -> i32 {
    if self.is_closed() {
        return ABI_CLOSED;
    }
    self.advance_runtime_start();
    if let Err(error) = self.application_runtime.advance_bootstrap() {
        self.last_result_json = error_result(&error.to_string());
        return ABI_ERROR;
    }
    let mut snapshot =
        match self.application_runtime.snapshot_context().map(bridge_snapshot_from_application)
        {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.last_result_json = error_result(&error.to_string());
                return ABI_ERROR;
            }
        };
    let _ = self.apply_history_summaries(&mut snapshot);
    let _ = self.apply_security_states(&mut snapshot);
    self.apply_navigation_badges(&mut snapshot);
    if !self.application_runtime.has_runtime() {
        snapshot.tor_state = torca_contract::tor_state_name(self.host_state_hint).into();
    }
    self.apply_host_state_hint(&mut snapshot);
    self.log_network_transitions(&snapshot);
    let snapshot_json = bridge_snapshot_json(&snapshot);
    self.snapshot_json = serde_json::from_str::<serde_json::Value>(&snapshot_json)
        .map(|mut value| {
            value["notificationsEnabled"] = serde_json::Value::Bool(self.notifications_enabled);
            value["readReceiptsEnabled"] = serde_json::Value::Bool(self.read_receipts_enabled);
            let (mode, background_sync, allow_delayed, metered, visual) =
                self.battery_policy.preferences.wire();
            value["batteryPreferences"] = json!({
                "mode": mode,
                "backgroundSync": background_sync,
                "allowDelayedBackgroundDelivery": allow_delayed,
                "meteredTransfers": metered,
                "visualActivity": visual,
            });
            value.to_string()
        })
        .unwrap_or(snapshot_json);
    ABI_OK
}

fn log_network_transitions(&mut self, snapshot: &torca_contract::BridgeSnapshot) {
    let onion = snapshot
        .bootstrap_steps
        .iter()
        .find(|step| step.id == "onion_service")
        .map(|step| (step.state.clone(), step.code.clone()));
    let relay = snapshot
        .bootstrap_steps
        .iter()
        .find(|step| step.id == "secure_relay")
        .map(|step| (step.state.clone(), step.code.clone()));

    if let Some(current) = onion.as_ref()
        && self.last_onion_log_state.as_ref() != Some(current)
    {
        let (level, code, message) = network_transition_event("ONION", current);
        self.log("tor", level, "onion_service", &code, &message);
        self.last_onion_log_state = Some(current.clone());
    }
    if let Some(current) = relay.as_ref()
        && self.last_relay_log_state.as_ref() != Some(current)
    {
        let (level, code, message) = network_transition_event("RELAY", current);
        self.log("relay", level, "relay_connection", &code, &message);
        self.last_relay_log_state = Some(current.clone());
    }
    for contact in &snapshot.contacts {
        let current =
            (contact.peer_health.state.clone(), contact.peer_health.reconnect_attempt);
        if self.last_peer_log_state.get(&contact.id) != Some(&current) {
            let code = format!("PEER_{}", current.0.to_ascii_uppercase());
            self.log(
                "messaging",
                if current.0 == "ready" { Level::Info } else { Level::Warn },
                "peer_connection",
                &code,
                &format!(
                    "contact={} state={} reconnect_attempt={}",
                    contact.id, current.0, current.1
                ),
            );
            self.last_peer_log_state.insert(contact.id.clone(), current);
        }
    }
    for attachment in &snapshot.attachments {
        let current = (attachment.status.clone(), attachment.offset, attachment.attempt_count);
        if self.last_attachment_log_state.get(&attachment.id) != Some(&current) {
            self.log(
                "messaging",
                if attachment.status == "failed" { Level::Error } else { Level::Info },
                "attachment_transfer",
                "ATTACHMENT_STATE_CHANGED",
                &format!(
                    "attachment={} direction={} status={} offset={}/{} attempt={} error={}",
                    attachment.id,
                    attachment.direction,
                    attachment.status,
                    attachment.offset,
                    attachment.size,
                    attachment.attempt_count,
                    attachment.last_error_code.as_deref().unwrap_or("none")
                ),
            );
            self.last_attachment_log_state.insert(attachment.id.clone(), current);
        }
    }

    for radio in &snapshot.radio.contacts {
        let current = format!(
            "local_enabled={} remote_state={} state={}",
            radio.local_enabled, radio.remote_state, radio.state
        );
        if self.last_radio_log_state.get(&radio.contact_id) != Some(&current) {
            let level = if radio.state == "ready" || radio.state == "receiving" {
                Level::Info
            } else if radio.state == "reconnecting" || radio.state == "unavailable" {
                Level::Warn
            } else {
                Level::Debug
            };
            self.log(
                "radio",
                level,
                "session",
                "RADIO_STATE_CHANGED",
                &format!("contact={} {}", radio.contact_id, current),
            );
            self.last_radio_log_state.insert(radio.contact_id.clone(), current);
        }
    }
    let session_state = snapshot.radio.session.as_ref().map_or_else(
        || "none".to_owned(),
        |session| {
            format!(
                "contact={} state={} floor={} burst_elapsed_ms={}",
                session.contact_id, session.state, session.floor, session.burst_elapsed_ms
            )
        },
    );
    if self.last_radio_log_state.get("active_session") != Some(&session_state) {
        self.log("radio", Level::Info, "session", "RADIO_SESSION_CHANGED", &session_state);
        self.last_radio_log_state.insert("active_session".into(), session_state);
    }

    let tor = snapshot
        .bootstrap_steps
        .iter()
        .find(|step| step.id == "tor_network")
        .map(|step| (step.state.clone(), step.code.clone()));
    let network_ready = tor.as_ref().is_some_and(|(state, _)| state == "ready");
    if network_ready && !self.network_ready_logged {
        self.log(
            "bootstrap",
            Level::Info,
            "network",
            "NETWORK_READY",
            "Tor runtime is ready; relay and local onion continue in the background",
        );
        self.network_ready_logged = true;
    }
}

pub(crate) fn reconcile_pending_operations(&mut self) -> bool {
    let before_snapshot = self.snapshot_json.clone();
    let before_cursor = self.notification_cursor;
    let _ = self.application_runtime.advance_pending_operations();
    let _ = self.refresh_snapshot();
    self.snapshot_json != before_snapshot || self.notification_cursor != before_cursor
}

pub(crate) fn next_pending_operation_delay(&self) -> Option<std::time::Duration> {
    self.application_runtime.next_pending_operation_delay()
}

fn apply_host_state_hint(&self, snapshot: &mut torca_contract::BridgeSnapshot) {
    if !self.application_runtime.has_runtime() {
        snapshot.bootstrap_phase = projected_host_bootstrap_phase(self.host_state_hint).into();
    }
    let network_state = if self.host_progress >= 100 {
        "ready"
    } else if matches!(self.host_state_hint, TorState::Degraded | TorState::Failed) {
        "failed"
    } else if self.host_retry_at.is_some() {
        "running"
    } else if self.host_start.is_some() {
        "running"
    } else {
        "pending"
    };
    debug_assert!(canonical_bootstrap_wire_state(network_state));
    if let Some(step) =
        snapshot.bootstrap_steps.iter_mut().find(|step| step.id == "tor_network")
    {
        step.state = network_state.into();
        step.code = self.host_status_code.clone();
        step.progress = self.host_progress;
        step.attempt = self.host_attempt;
        step.started_at_ms = self.host_start_started_at_ms;
        step.last_progress_at_ms = self.host_last_progress_at_ms;
        step.retry_at_ms = self.host_retry_at.and_then(instant_to_unix_ms);
    }
    if self.application_runtime.has_runtime() {
        return;
    }
    let onion_stalled = self.host_onion_attempt > 0
        && self.host_onion_progress < 100
        && self.host_onion_last_progress_at_ms.is_some_and(|last_progress| {
            unix_time_ms().ok().and_then(|now| now.checked_sub(last_progress)).is_some_and(
                |elapsed_ms| {
                    elapsed_ms
                        >= i64::try_from(ONION_PROGRESS_STALL_AFTER.as_millis())
                            .unwrap_or(i64::MAX)
                },
            )
        });
    let onion_state = if self.host_progress < 100 {
        if network_state == "failed" { "blocked" } else { "pending" }
    } else if self.host_onion_progress >= 100 {
        "ready"
    } else if matches!(self.host_state_hint, TorState::Degraded | TorState::Failed) {
        "failed"
    } else if self.host_onion_retry_at.is_some() {
        "running"
    } else if onion_stalled {
        "verifying"
    } else if self.host_onion_attempt > 0 {
        "running"
    } else {
        "pending"
    };
    debug_assert!(canonical_bootstrap_wire_state(onion_state));
    if let Some(step) =
        snapshot.bootstrap_steps.iter_mut().find(|step| step.id == "onion_service")
    {
        step.state = onion_state.into();
        step.code = if onion_state == "blocked" {
            Some("TOR_NETWORK_REQUIRED".into())
        } else if onion_stalled {
            Some("ONION_PUBLICATION_STALLED".into())
        } else {
            self.host_onion_status_code.clone()
        };
        step.progress = self.host_onion_progress;
        step.attempt = self.host_onion_attempt;
        step.started_at_ms = self.host_onion_started_at_ms;
        step.last_progress_at_ms = self.host_onion_last_progress_at_ms;
        step.retry_at_ms = self.host_onion_retry_at.and_then(instant_to_unix_ms);
    }
}

pub(crate) fn conversation_page(
    &mut self,
    conversation_id: &str,
    before_at_ms: Option<i64>,
    before_message_id: Option<&str>,
    limit: usize,
) -> i32 {
    if self.is_closed() {
        return ABI_CLOSED;
    }
    let conversation_id = match conversation_id.parse::<OpaqueId>() {
        Ok(value) => ConversationId::from_opaque(value),
        Err(_) => return self.query_error("invalid conversation id"),
    };
    let before = match (before_at_ms, before_message_id) {
        (Some(at_ms), Some(message_id)) => {
            let at = match Timestamp::from_unix_millis(at_ms) {
                Ok(value) => value,
                Err(_) => return self.query_error("invalid page timestamp"),
            };
            let message_id = match message_id.parse::<OpaqueId>() {
                Ok(value) => MessageId::from_opaque(value),
                Err(_) => return self.query_error("invalid page message id"),
            };
            Some((at, message_id))
        }
        (None, None) => None,
        _ => return self.query_error("incomplete page cursor"),
    };
    match self.read_models().history.page_for_conversation(conversation_id, before, limit) {
        Ok(page) => {
            let page = BridgeMessagePage {
                messages: page.messages.into_iter().map(bridge_message_from_domain).collect(),
                has_more: page.has_more,
            };
            self.query_json = bridge_message_page_json(&page);
            ABI_OK
        }
        Err(_) => self.query_error("conversation history unavailable"),
    }
}

pub(crate) fn search_messages(
    &mut self,
    conversation_id: &str,
    query: &str,
    limit: usize,
) -> i32 {
    if self.is_closed() {
        return ABI_CLOSED;
    }
    let conversation_id = match conversation_id.parse::<OpaqueId>() {
        Ok(value) => ConversationId::from_opaque(value),
        Err(_) => return self.query_error("invalid conversation id"),
    };
    match self.read_models().history.search_conversation(conversation_id, query, limit) {
        Ok(messages) => {
            let page = BridgeMessagePage {
                messages: messages.into_iter().map(bridge_message_from_domain).collect(),
                has_more: false,
            };
            self.query_json = bridge_message_page_json(&page);
            ABI_OK
        }
        Err(_) => self.query_error("conversation search unavailable"),
    }
}
