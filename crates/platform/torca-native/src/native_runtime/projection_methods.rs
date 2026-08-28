impl TorcaRuntime {
    pub(crate) fn refresh_snapshot(&mut self) -> i32 {
        if self.is_closed() {
            return ABI_CLOSED;
        }
        self.advance_runtime_start();
        if let Err(error) = self.application_runtime.advance_bootstrap() {
            self.log(
                "ffi",
                Level::Error,
                "snapshot",
                "SNAPSHOT_REFRESH_FAILED",
                &format!("bootstrap advance failed: {error}"),
            );
            self.last_result_json = error_result(&error.to_string());
            return ABI_ERROR;
        }
        let mut snapshot =
            match self.application_runtime.snapshot_context().map(bridge_snapshot_from_application)
            {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    self.log(
                        "ffi",
                        Level::Error,
                        "snapshot",
                        "SNAPSHOT_REFRESH_FAILED",
                        &format!("application snapshot failed: {error}"),
                    );
                    self.last_result_json = error_result(&error.to_string());
                    return ABI_ERROR;
                }
            };
        let _ = self.apply_history_summaries(&mut snapshot);
        let _ = self.apply_security_states(&mut snapshot);
        self.apply_navigation_badges(&mut snapshot);
        self.apply_host_state_hint(&mut snapshot);
        self.log_network_transitions(&snapshot);
        let mut value = bridge_snapshot_value(&snapshot);
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
        self.snapshot_value = value;
        self.snapshot_json = self.snapshot_value.to_string();
        ABI_OK
    }

    fn log_network_transitions(&mut self, snapshot: &torca_contract::BridgeSnapshot) {
        let incoming = snapshot
            .bootstrap_steps
            .iter()
            .find(|step| step.id == "incoming_reachability")
            .map(|step| (step.state.clone(), step.code.clone()));
        // A managed rendezvous is a Tor-specific capability. Direct
        // providers may retain the legacy compatibility step in the durable
        // bootstrap model, but must not emit relay/rendezvous diagnostics.
        // During the first snapshot the application projection can still carry
        // its legacy default before the selected provider has published its
        // commissioning state. Use the compiled provider as the authority so
        // a direct provider (Iroh/WebRTC) never emits Tor-only rendezvous
        // diagnostics during that short bootstrap window.
        let selected_provider = crate::transport_config::compiled_provider()
            .map(|provider| provider.as_str().to_owned())
            .unwrap_or_else(|_| snapshot.communication_provider.clone());
        let rendezvous = (selected_provider == "tor")
            .then(|| {
                snapshot
                    .bootstrap_steps
                    .iter()
                    .find(|step| step.id == "rendezvous")
                    .map(|step| (step.state.clone(), step.code.clone()))
            })
            .flatten();

        if let Some(current) = incoming.as_ref()
            && self.last_onion_log_state.as_ref() != Some(current)
        {
            let (level, code, message) = network_transition_event("INCOMING", current);
            self.log("communication", level, "incoming_reachability", &code, &message);
            self.last_onion_log_state = Some(current.clone());
        }
        if let Some(current) = rendezvous.as_ref()
            && self.last_relay_log_state.as_ref() != Some(current)
        {
            let (level, code, message) = network_transition_event("RENDEZVOUS", current);
            self.log("communication", level, "rendezvous", &code, &message);
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
        for message in &snapshot.messages {
            let current = (message.status.clone(), message.attempt_count);
            if self.last_message_log_state.get(&message.id) != Some(&current) {
                let level = match message.status.as_str() {
                    "failed" => Level::Error,
                    "queued" | "sending" => Level::Warn,
                    _ => Level::Info,
                };
                self.log(
                "messaging",
                level,
                "delivery",
                "MESSAGE_STATE_CHANGED",
                &format!(
                    "message={} conversation={} direction={} status={} attempt={} sent_at_ms={} delivered_at_ms={} read_at_ms={}",
                    message.id,
                    message.conversation_id,
                    message.direction,
                    message.status,
                    message.attempt_count,
                    message.sent_at_ms.map_or_else(|| "none".into(), |value| value.to_string()),
                    message.delivered_at_ms.map_or_else(|| "none".into(), |value| value.to_string()),
                    message.read_at_ms.map_or_else(|| "none".into(), |value| value.to_string()),
                ),
            );
                self.last_message_log_state.insert(message.id.clone(), current);
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
            || format!(
                "none transport_failure={}",
                snapshot.radio.last_transport_failure.as_deref().unwrap_or("none")
            ),
            |session| {
                format!(
                    "contact={} state={} floor={} burst_elapsed_ms={} transport_failure={}",
                    session.contact_id,
                    session.state,
                    session.floor,
                    session.burst_elapsed_ms,
                    snapshot.radio.last_transport_failure.as_deref().unwrap_or("none")
                )
            },
        );
        if self.last_radio_log_state.get("active_session") != Some(&session_state) {
            self.log("radio", Level::Info, "session", "RADIO_SESSION_CHANGED", &session_state);
            self.last_radio_log_state.insert("active_session".into(), session_state);
        }

        let communication = snapshot
            .bootstrap_steps
            .iter()
            .find(|step| step.id == "communication_runtime")
            .map(|step| (step.state.clone(), step.code.clone()));
        let network_ready = communication.as_ref().is_some_and(|(state, _)| state == "ready");
        if network_ready && !self.network_ready_logged {
            self.log(
                "bootstrap",
                Level::Info,
                "network",
                "COMMUNICATION_READY",
                "Communication runtime is ready; optional capabilities continue in the background",
            );
            self.network_ready_logged = true;
        }
    }

    pub(crate) fn reconcile_pending_operations(&mut self) -> bool {
        let before_snapshot = std::mem::take(&mut self.snapshot_json);
        let before_snapshot_value =
            std::mem::replace(&mut self.snapshot_value, serde_json::Value::Null);
        let before_cursor = self.notification_cursor;
        let _ = self.application_runtime.advance_pending_operations();
        if self.refresh_snapshot() != ABI_OK {
            self.snapshot_json = before_snapshot;
            self.snapshot_value = before_snapshot_value;
            return self.notification_cursor != before_cursor;
        }
        self.snapshot_json != before_snapshot || self.notification_cursor != before_cursor
    }

    pub(crate) fn next_pending_operation_delay(&self) -> Option<std::time::Duration> {
        let pending = self.application_runtime.next_pending_operation_delay();
        // Provider startup progress/finish wakes the actor through
        // ActorMessage::InternalWake. There must be no periodic 100ms poll
        // while a slow provider is bootstrapping. The real startup/retry
        // deadlines remain as one-shot safety deadlines.
        [
            pending,
            self.host_start_deadline
                .map(|deadline| deadline.saturating_duration_since(std::time::Instant::now())),
            self.host_retry_at
                .map(|deadline| deadline.saturating_duration_since(std::time::Instant::now())),
        ]
        .into_iter()
        .flatten()
        .min()
    }

    pub(crate) fn maintain_native_startup(&mut self) -> bool {
        if self.host_start.is_none() {
            return false;
        }
        let before = (
            self.host.is_some(),
            self.host_progress,
            self.host_status_code.clone(),
            self.host_state_hint,
            self.host_start.is_some(),
        );
        self.advance_runtime_start();
        let after = (
            self.host.is_some(),
            self.host_progress,
            self.host_status_code.clone(),
            self.host_state_hint,
            self.host_start.is_some(),
        );
        if before == after {
            return false;
        }
        let _ = self.refresh_snapshot();
        true
    }

    fn apply_host_state_hint(&self, snapshot: &mut torca_contract::BridgeSnapshot) {
        if !self.application_runtime.has_runtime() {
            snapshot.bootstrap_phase = projected_host_bootstrap_phase(self.host_state_hint).into();
        }
        let network_state = if self.host_progress >= 100 {
            "ready"
        } else if matches!(
            self.host_state_hint,
            CommunicationState::Degraded | CommunicationState::Failed
        ) {
            "failed"
        } else if self.host_retry_at.is_some() || self.host_start.is_some() {
            "running"
        } else {
            "pending"
        };
        debug_assert!(canonical_bootstrap_wire_state(network_state));
        if let Some(step) =
            snapshot.bootstrap_steps.iter_mut().find(|step| step.id == "communication_runtime")
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
        let incoming_stalled = self.host_incoming_attempt > 0
            && self.host_incoming_progress < 100
            && self.host_incoming_last_progress_at_ms.is_some_and(|last_progress| {
                unix_time_ms().ok().and_then(|now| now.checked_sub(last_progress)).is_some_and(
                    |elapsed_ms| {
                        elapsed_ms
                            >= i64::try_from(INCOMING_REACHABILITY_PROGRESS_STALL_AFTER.as_millis())
                                .unwrap_or(i64::MAX)
                    },
                )
            });
        let incoming_state = if self.host_progress < 100 {
            if network_state == "failed" { "blocked" } else { "pending" }
        } else if self.host_incoming_progress >= 100 {
            "ready"
        } else if matches!(
            self.host_state_hint,
            CommunicationState::Degraded | CommunicationState::Failed
        ) {
            "failed"
        } else if self.host_incoming_retry_at.is_some() {
            "running"
        } else if incoming_stalled {
            "verifying"
        } else if self.host_incoming_attempt > 0 {
            "running"
        } else {
            "pending"
        };
        debug_assert!(canonical_bootstrap_wire_state(incoming_state));
        if let Some(step) =
            snapshot.bootstrap_steps.iter_mut().find(|step| step.id == "incoming_reachability")
        {
            step.state = incoming_state.into();
            step.code = if incoming_state == "blocked" {
                Some("COMMUNICATION_RUNTIME_REQUIRED".into())
            } else if incoming_stalled {
                Some("INCOMING_REACHABILITY_STALLED".into())
            } else {
                self.host_incoming_status_code.clone()
            };
            step.progress = self.host_incoming_progress;
            step.attempt = self.host_incoming_attempt;
            step.started_at_ms = self.host_incoming_started_at_ms;
            step.last_progress_at_ms = self.host_incoming_last_progress_at_ms;
            step.retry_at_ms = self.host_incoming_retry_at.and_then(instant_to_unix_ms);
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
}
