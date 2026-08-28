use serde_json::{Value, json};
use torca_contract::{BridgeMessagePage, BridgeResult, BridgeSnapshot, CONTRACT_VERSION};

pub(crate) fn success_result(kind: &str) -> String {
    bridge_result_json(&BridgeResult {
        ok: true,
        kind: kind.to_owned(),
        error: None,
        error_code: None,
        resource_id: None,
        invite_uri: None,
    })
}

pub(crate) fn error_result(_error: &str) -> String {
    // Native diagnostic text is intentionally not part of the user-facing
    // bridge contract. The diagnostic stream receives the original failure;
    // the UI resolves this stable code through localization.
    bridge_result_json(&BridgeResult {
        ok: false,
        kind: "error".into(),
        error: None,
        error_code: Some("operation_failed".into()),
        resource_id: None,
        invite_uri: None,
    })
}

pub(crate) fn bridge_result_json(result: &BridgeResult) -> String {
    let kind = if result.ok {
        result.kind.clone()
    } else {
        let code = result.error_code.as_deref().unwrap_or("operation_failed");
        format!("error:{code}")
    };
    json!({
        "ok": result.ok,
        "kind": kind,
        "error": Value::Null,
        "errorCode": result.error_code,
        "resourceId": result.resource_id,
        "inviteUri": result.invite_uri,
    })
    .to_string()
}

pub(crate) fn empty_snapshot_json() -> String {
    // The first snapshot can be requested before the provider composition
    // worker finishes. It must still identify the compiled provider; using
    // `tor` here creates false rendezvous/relay diagnostics for Iroh.
    let provider = crate::transport_config::compiled_provider()
        .map(|value| value.as_str().to_owned())
        .unwrap_or_else(|_| "unknown".to_owned());
    json!({
        "contractVersion": CONTRACT_VERSION,
        "identity": Value::Null,
        "communicationProvider": provider,
        "communicationState": "stopped",
        "endpointSummary": Value::Null,
        "torState": "stopped",
        "transport": {
            "communication": { "state": "stopped", "code": "COMMUNICATION_NOT_READY", "latencyMs": Value::Null, "lastActivityAtMs": Value::Null, "activitySequence": 0, "txSequence": 0, "rxSequence": 0, "inFlight": 0, "queued": 0 },
            "providerRouteState": "unavailable",
            "tor": { "state": "stopped", "code": "TOR_NOT_READY", "latencyMs": Value::Null, "lastActivityAtMs": Value::Null, "activitySequence": 0 },
            "relay": { "state": "unknown", "code": "RELAY_UNAVAILABLE", "latencyMs": Value::Null, "lastActivityAtMs": Value::Null, "activitySequence": 0 }
        },
        "onionAddress": Value::Null,
        // This is a provisional snapshot before the provider composition
        // worker has reported its first state. Reporting `failed` here makes
        // Flutter render a terminal error during every cold start.
        "bootstrapPhase": "starting",
        "bootstrapSteps": [], "pairings": [], "contacts": [], "conversations": [],
        "messages": [], "reactions": [], "attachments": [], "navigationBadges": { "unreadMessages": 0, "newContacts": 0, "pairingAttention": 0 },
        "radio": { "activeContactId": Value::Null, "contacts": [], "session": Value::Null },
        "notificationsEnabled": true
    }).to_string()
}

/// Builds the native projection once as a parsed value. Callers that need to
/// add runtime-owned fields can now mutate this value directly instead of
/// serializing and immediately reparsing the same snapshot.
pub(crate) fn bridge_snapshot_value(snapshot: &BridgeSnapshot) -> Value {
    let mut value = serde_json::to_value(snapshot).unwrap_or_else(|_| json!({}));
    let Some(object) = value.as_object_mut() else { return json!({}) };
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
    value
}

fn identity_value(snapshot: &BridgeSnapshot) -> Value {
    match (&snapshot.identity_id, &snapshot.identity_name, &snapshot.identity_fingerprint) {
        (None, None, None) => Value::Null,
        (identity_id, display_name, fingerprint) => {
            json!({ "id": identity_id, "displayName": display_name, "fingerprint": fingerprint })
        }
    }
}

pub(crate) fn bridge_message_page_json(page: &BridgeMessagePage) -> String {
    serde_json::to_string(page).unwrap_or_else(|_| "{\"messages\":[],\"hasMore\":false}".into())
}
