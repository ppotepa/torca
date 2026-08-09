use core::fmt::Write as _;
use std::cmp::Reverse;

use torca_contract::{
    BridgeMessage, BridgeMessagePage, BridgeResult, BridgeSnapshot, CONTRACT_VERSION,
};

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
        let raw = result.error.as_deref().unwrap_or_default();
        let code = classify_error(raw);
        (format!("error:{code}"), Some(error_message(code)))
    };
    let mut output = String::from("{\"ok\":");
    output.push_str(if result.ok { "true" } else { "false" });
    output.push_str(",\"kind\":\"");
    push_json_string(&kind, &mut output);
    output.push_str("\",\"error\":");
    match error {
        Some(value) => {
            output.push('"');
            push_json_string(value, &mut output);
            output.push('"');
        }
        None => output.push_str("null"),
    }
    output.push('}');
    output
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
    format!(
        "{{\"contractVersion\":{CONTRACT_VERSION},\"identity\":null,\"torState\":\"stopped\",\"onionAddress\":null,\"bootstrapPhase\":\"failed\",\"bootstrapSteps\":[],\"pairings\":[],\"contacts\":[],\"conversations\":[],\"messages\":[],\"attachments\":[],\"notificationsEnabled\":true}}"
    )
}

pub(crate) fn bridge_snapshot_json(snapshot: &BridgeSnapshot) -> String {
    let mut output = String::new();
    let _ = write!(output, "{{\"contractVersion\":{}", snapshot.contract_version);
    output.push_str(",\"bootstrapPhase\":\"");
    push_json_string(&snapshot.bootstrap_phase, &mut output);
    output.push_str("\",\"bootstrapSteps\":[");
    for (index, step) in snapshot.bootstrap_steps.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        output.push_str("{\"id\":\"");
        push_json_string(&step.id, &mut output);
        output.push_str("\",\"state\":\"");
        push_json_string(&step.state, &mut output);
        output.push_str("\",\"code\":");
        push_optional_string(&step.code, &mut output);
        output.push('}');
    }
    output.push(']');
    output.push_str(",\"identity\":");
    match (&snapshot.identity_name, &snapshot.identity_fingerprint) {
        (Some(name), fingerprint) => {
            output.push_str("{\"displayName\":\"");
            push_json_string(name, &mut output);
            output.push_str("\",\"fingerprint\":");
            push_optional_string(fingerprint, &mut output);
            output.push('}');
        }
        (None, Some(fingerprint)) => {
            output.push_str("{\"displayName\":null,\"fingerprint\":");
            push_optional_string(&Some(fingerprint.clone()), &mut output);
            output.push('}');
        }
        (None, None) => output.push_str("null"),
    }
    output.push_str(",\"torState\":\"");
    push_json_string(&snapshot.tor_state, &mut output);
    output.push('"');
    output.push_str(",\"onionAddress\":");
    match &snapshot.onion_address {
        Some(value) => {
            output.push('"');
            push_json_string(value, &mut output);
            output.push('"');
        }
        None => output.push_str("null"),
    }

    output.push_str(",\"pairings\":[");
    for (index, pairing) in snapshot.pairings.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        output.push_str("{\"id\":\"");
        push_json_string(&pairing.id, &mut output);
        output.push_str("\",\"code\":\"");
        push_json_string(&pairing.code, &mut output);
        output.push_str("\",\"role\":\"");
        push_json_string(&pairing.role, &mut output);
        output.push_str("\",\"state\":\"");
        push_json_string(&pairing.state, &mut output);
        let _ = write!(
            output,
            "\",\"expiresAtMs\":{},\"localApproved\":{},\"remoteApproved\":{}}}",
            pairing.expires_at_ms, pairing.local_approved, pairing.remote_approved
        );
    }

    output.push_str("],\"contacts\":[");
    for (index, contact) in snapshot.contacts.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        output.push_str("{\"id\":\"");
        push_json_string(&contact.id, &mut output);
        output.push_str("\",\"displayName\":\"");
        push_json_string(&contact.display_name, &mut output);
        output.push_str("\",\"onionAddress\":\"");
        push_json_string(&contact.onion_address, &mut output);
        output.push_str("\",\"status\":\"");
        push_json_string(&contact.status, &mut output);
        output.push_str("\",\"connectionState\":\"");
        push_json_string(&contact.connection_state, &mut output);
        output.push_str("\",\"safetyNumber\":\"");
        push_json_string(&contact.safety_number, &mut output);
        output.push_str("\",\"verificationStatus\":\"");
        push_json_string(&contact.verification_status, &mut output);
        output.push_str("\",\"verifiedAtMs\":");
        match contact.verified_at_ms {
            Some(value) => {
                let _ = write!(output, "{value}");
            }
            None => output.push_str("null"),
        }
        output.push_str(",\"peerHealth\":{\"state\":\"");
        push_json_string(&contact.peer_health.state, &mut output);
        output.push_str("\",\"quality\":\"");
        push_json_string(&contact.peer_health.quality, &mut output);
        output.push_str("\",\"rttMs\":");
        match contact.peer_health.rtt_ms {
            Some(value) => {
                let _ = write!(output, "{value}");
            }
            None => output.push_str("null"),
        }
        output.push_str(",\"lastSuccessAtMs\":");
        match contact.peer_health.last_success_at_ms {
            Some(value) => {
                let _ = write!(output, "{value}");
            }
            None => output.push_str("null"),
        }
        let _ = write!(
            output,
            ",\"consecutiveFailures\":{},\"reconnectAttempt\":{}}}}}",
            contact.peer_health.consecutive_failures, contact.peer_health.reconnect_attempt
        );
    }

    output.push_str("],\"conversations\":[");
    let mut conversations = snapshot.conversations.iter().collect::<Vec<_>>();
    conversations.sort_by_key(|conversation| Reverse(conversation.last_activity_at_ms));
    for (index, conversation) in conversations.into_iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        output.push_str("{\"id\":\"");
        push_json_string(&conversation.id, &mut output);
        output.push_str("\",\"contactId\":\"");
        push_json_string(&conversation.contact_id, &mut output);
        output.push_str("\",\"status\":\"");
        push_json_string(&conversation.status, &mut output);
        let _ = write!(
            output,
            "\",\"unreadCount\":{},\"lastActivityAtMs\":{},\"lastMessageBody\":",
            conversation.unread_count, conversation.last_activity_at_ms
        );
        push_optional_string(&conversation.last_message_body, &mut output);
        output.push_str(",\"lastMessageDirection\":");
        push_optional_string(&conversation.last_message_direction, &mut output);
        output.push_str(",\"lastMessageStatus\":");
        push_optional_string(&conversation.last_message_status, &mut output);
        output.push('}');
    }

    output.push_str("],\"messages\":[");
    for (index, message) in snapshot.messages.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        push_bridge_message(message, &mut output);
    }

    output.push_str("],\"attachments\":[");
    for (index, attachment) in snapshot.attachments.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        output.push_str("{\"id\":\"");
        push_json_string(&attachment.id, &mut output);
        output.push_str("\",\"messageId\":\"");
        push_json_string(&attachment.message_id, &mut output);
        output.push_str("\",\"name\":\"");
        push_json_string(&attachment.name, &mut output);
        output.push_str("\",\"mediaType\":\"");
        push_json_string(&attachment.media_type, &mut output);
        let _ = write!(output, "\",\"size\":{},\"status\":\"", attachment.size);
        push_json_string(&attachment.status, &mut output);
        let _ = write!(output, "\",\"offset\":{}}}", attachment.offset);
    }
    output.push_str("]}");
    output
}

pub(crate) fn bridge_message_page_json(page: &BridgeMessagePage) -> String {
    let mut output = String::from("{\"messages\":[");
    for (index, message) in page.messages.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        push_bridge_message(message, &mut output);
    }
    output.push_str("],\"hasMore\":");
    output.push_str(if page.has_more { "true" } else { "false" });
    output.push('}');
    output
}

fn push_bridge_message(message: &BridgeMessage, output: &mut String) {
    output.push_str("{\"id\":\"");
    push_json_string(&message.id, output);
    output.push_str("\",\"conversationId\":\"");
    push_json_string(&message.conversation_id, output);
    output.push_str("\",\"body\":\"");
    push_json_string(&message.body, output);
    output.push_str("\",\"direction\":\"");
    push_json_string(&message.direction, output);
    output.push_str("\",\"status\":\"");
    push_json_string(&message.status, output);
    output.push_str("\",\"replyToMessageId\":");
    push_optional_string(&message.reply_to_message_id, output);
    let _ = write!(
        output,
        ",\"createdAtMs\":{},\"updatedAtMs\":{},\"attemptCount\":{}}}",
        message.created_at_ms, message.updated_at_ms, message.attempt_count
    );
}

fn push_optional_string(value: &Option<String>, output: &mut String) {
    match value {
        Some(value) => {
            output.push('"');
            push_json_string(value, output);
            output.push('"');
        }
        None => output.push_str("null"),
    }
}

fn push_json_string(value: &str, output: &mut String) {
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character.is_control() => {
                let _ = write!(output, "\\u{:04x}", character as u32);
            }
            character => output.push(character),
        }
    }
}
