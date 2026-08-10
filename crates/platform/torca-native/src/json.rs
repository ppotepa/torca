use serde_json::{Value, json};
use torca_contract::{BridgeMessagePage, BridgeResult, BridgeSnapshot, CONTRACT_VERSION};

pub(crate) fn success_result(kind: &str) -> String {
    bridge_result_json(&BridgeResult { ok: true, kind: kind.to_owned(), error: None })
}

pub(crate) fn error_result(error: &str) -> String {
    bridge_result_json(&BridgeResult {
        ok: false,
        kind: "error".into(),
        error: Some(error.to_owned()),
    })
}

pub(crate) fn bridge_result_json(result: &BridgeResult) -> String {
    let (kind, error) = if result.ok {
        (result.kind.clone(), None)
    } else {
        let code = classify_error(result.error.as_deref().unwrap_or_default());
        (format!("error:{code}"), Some(error_message(code)))
    };
    json!({ "ok": result.ok, "kind": kind, "error": error }).to_string()
}

fn classify_error(error: &str) -> &'static str {
    let value = error.to_ascii_lowercase();
    if value.contains("profile_not_ready") {
        "profile_not_ready"
    } else if value.contains("profile_snapshot_inconsistent") {
        "profile_snapshot_inconsistent"
    } else if value.contains("relay_degraded") {
        "relay_degraded"
    } else if value.contains("relay_not_ready") {
        "relay_not_ready"
    } else if value.contains("identity changed") || value.contains("re-verification") {
        "identity_changed"
    } else if value.contains("expired") {
        "pairing_expired"
    } else if value.contains("already") || value.contains("exists") {
        "already_exists"
    } else if value.contains("not found") || value.contains("missing") {
        "not_found"
    } else if value.contains("invalid") || value.contains("empty") || value.contains("utf-8") {
        "invalid_input"
    } else if value.contains("storage") || value.contains("database") || value.contains("sql") {
        "storage_failure"
    } else if value.contains("attachment") {
        "attachment_failure"
    } else if value.contains("tor")
        || value.contains("peer")
        || value.contains("connection")
        || value.contains("network")
    {
        "network_unavailable"
    } else if value.contains("not ready")
        || value.contains("unavailable")
        || value.contains("closed")
    {
        "runtime_unavailable"
    } else if value.contains("transition")
        || value.contains("blocked")
        || value.contains("conflict")
    {
        "operation_conflict"
    } else {
        "operation_failed"
    }
}

fn error_message(code: &str) -> &'static str {
    match code {
        "profile_not_ready" => "The secure runtime is not ready for profile setup.",
        "profile_snapshot_inconsistent" => {
            "The profile update was committed but the snapshot is inconsistent."
        }
        "relay_degraded" => "Pairing is temporarily blocked while the secure relay is degraded.",
        "relay_not_ready" => "Pairing is unavailable until the secure relay probe completes.",
        "identity_changed" => {
            "Contact identity changed. Verify the new Safety Number before sending."
        }
        "pairing_expired" => "The pairing invitation has expired.",
        "already_exists" => "This item already exists.",
        "not_found" => "The requested item is no longer available.",
        "invalid_input" => "The supplied value is not valid.",
        "storage_failure" => "Encrypted local storage could not complete the operation.",
        "attachment_failure" => "The attachment operation could not be completed.",
        "network_unavailable" => "The secure Tor peer connection is currently unavailable.",
        "runtime_unavailable" => "The secure Torca runtime is currently unavailable.",
        "operation_conflict" => "The operation is not valid in the current state.",
        _ => "The operation could not be completed.",
    }
}

pub(crate) fn empty_snapshot_json() -> String {
    json!({
        "contractVersion": CONTRACT_VERSION,
        "identity": Value::Null,
        "torState": "stopped",
        "transport": {
            "tor": { "state": "stopped", "code": "TOR_NOT_READY", "latencyMs": Value::Null, "lastActivityAtMs": Value::Null, "activitySequence": 0 },
            "relay": { "state": "unknown", "code": "RELAY_UNAVAILABLE", "latencyMs": Value::Null, "lastActivityAtMs": Value::Null, "activitySequence": 0 }
        },
        "onionAddress": Value::Null,
        "bootstrapPhase": "failed",
        "bootstrapSteps": [], "pairings": [], "contacts": [], "conversations": [],
        "messages": [], "attachments": [], "navigationBadges": { "unreadMessages": 0, "newContacts": 0, "pairingAttention": 0 },
        "notificationsEnabled": true
    }).to_string()
}

pub(crate) fn bridge_snapshot_json(snapshot: &BridgeSnapshot) -> String {
    let mut value = serde_json::to_value(snapshot).unwrap_or_else(|_| json!({}));
    let Some(object) = value.as_object_mut() else { return "{}".into() };
    object.insert("identity".into(), identity_value(snapshot));
    object.insert(
        "navigationBadges".into(),
        json!({
            "unreadMessages": snapshot.unread_messages_count,
            "newContacts": snapshot.new_contacts_count,
            "pairingAttention": snapshot.pairing_attention_count,
        }),
    );
    if let Some(Value::Array(conversations)) = object.get_mut("conversations") {
        conversations.sort_by_key(|conversation| {
            std::cmp::Reverse(
                conversation.get("lastActivityAtMs").and_then(Value::as_i64).unwrap_or_default(),
            )
        });
    }
    serde_json::to_string(&value).unwrap_or_else(|_| "{}".into())
}

fn identity_value(snapshot: &BridgeSnapshot) -> Value {
    match (&snapshot.identity_name, &snapshot.identity_fingerprint) {
        (None, None) => Value::Null,
        (display_name, fingerprint) => {
            json!({ "displayName": display_name, "fingerprint": fingerprint })
        }
    }
}

pub(crate) fn bridge_message_page_json(page: &BridgeMessagePage) -> String {
    serde_json::to_string(page).unwrap_or_else(|_| "{\"messages\":[],\"hasMore\":false}".into())
}
