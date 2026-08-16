fn network_transition_event(
    component: &str,
    (state, diagnostic): &(String, Option<String>),
) -> (Level, String, String) {
    let suffix = match state.as_str() {
        "ready" => "READY",
        "failed" => "FAILED",
        "degraded" => "DEGRADED",
        "retrying" => "RETRYING",
        "running" | "checking" => "CONNECTING",
        _ => "PENDING",
    };
    let level = match suffix {
        "FAILED" => Level::Error,
        "DEGRADED" | "RETRYING" => Level::Warn,
        _ => Level::Info,
    };
    let code = format!("{component}_{suffix}");
    let detail = diagnostic.as_deref().unwrap_or("no diagnostic code");
    let message = format!("{component} state changed to {state} ({detail})");
    (level, code, message)
}

fn canonical_bootstrap_wire_state(state: &str) -> bool {
    matches!(
        state,
        "pending" | "running" | "verifying" | "ready" | "degraded" | "failed" | "blocked"
    )
}

const fn projected_host_bootstrap_phase(host_state: TorState) -> &'static str {
    if matches!(host_state, TorState::Degraded | TorState::Failed) { "degraded" } else { "running" }
}

fn notification_event_json(event: &torca_contract::NotificationEvent) -> serde_json::Value {
    serde_json::json!({
        "cursor": event.cursor,
        "eventId": event.event_id,
        "kind": event.kind,
        "resourceId": event.resource_id,
        "conversationId": event.conversation_id,
        "contactDisplayName": event.contact_display_name,
        "createdAtMs": event.created_at_ms,
    })
}

fn snapshot_contact_name(
    snapshot: &torca_contract::BridgeSnapshot,
    contact_names: &HashMap<String, String>,
    conversation_id: &str,
) -> String {
    snapshot
        .conversations
        .iter()
        .find(|conversation| conversation.id == conversation_id)
        .and_then(|conversation| contact_names.get(&conversation.contact_id))
        .cloned()
        .unwrap_or_default()
}

fn panic_message(payload: Box<dyn Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_owned()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "non-string panic payload".to_owned()
    }
}

fn unix_time_ms() -> Result<i64, ()> {
    let elapsed = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| ())?;
    i64::try_from(elapsed.as_millis()).map_err(|_| ())
}

fn profile_for_preferences(preferences: BatteryPreferences) -> BatteryProfile {
    match preferences.mode {
        RequestedBatteryMode::Automatic | RequestedBatteryMode::AlwaysAvailable => {
            BatteryProfile::AlwaysAvailable
        }
        RequestedBatteryMode::Balanced => BatteryProfile::Balanced,
        RequestedBatteryMode::BatterySaver => BatteryProfile::BatterySaver,
    }
}

fn instant_to_unix_ms(deadline: Instant) -> Option<i64> {
    let remaining = deadline.checked_duration_since(Instant::now()).unwrap_or_default();
    let remaining_ms = i64::try_from(remaining.as_millis()).ok()?;
    unix_time_ms().ok()?.checked_add(remaining_ms)
}

impl Drop for TorcaRuntime {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

#[cfg(test)]
mod tests {
    use super::{
        TorState, canonical_bootstrap_wire_state, notification_event_json,
        projected_host_bootstrap_phase,
    };

    #[test]
    fn host_projection_cannot_unlock_profile_before_runtime_bootstrap_finishes() {
        assert_eq!(projected_host_bootstrap_phase(TorState::Starting), "running");
        assert_eq!(projected_host_bootstrap_phase(TorState::Ready), "running");
        assert_eq!(projected_host_bootstrap_phase(TorState::Failed), "degraded");
    }

    #[test]
    fn host_bootstrap_projection_uses_only_contract_states() {
        for state in ["pending", "running", "verifying", "ready", "degraded", "failed", "blocked"] {
            assert!(canonical_bootstrap_wire_state(state));
        }
        assert!(!canonical_bootstrap_wire_state("retrying"));
        assert!(!canonical_bootstrap_wire_state("stalled"));
    }

    #[test]
    fn notification_wire_uses_created_at_ms() {
        let event = torca_contract::NotificationEvent {
            cursor: 11,
            event_id: "event-11".into(),
            kind: "message_received".into(),
            resource_id: "conversation-1".into(),
            conversation_id: "conversation-1".into(),
            contact_display_name: "Alice".into(),
            created_at_ms: 1_700_000_000_123,
            title: "New message".into(),
            body: "Private message received".into(),
        };
        let value = notification_event_json(&event);
        assert_eq!(value["createdAtMs"], 1_700_000_000_123_i64);
        assert!(value.get("createdAt").is_none());
    }
}
