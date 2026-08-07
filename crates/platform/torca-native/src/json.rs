use core::fmt::Write as _;
use torca_bridge::{BridgeResult, BridgeSnapshot, CONTRACT_VERSION};

pub(crate) fn success_result(kind: &str) -> String {
    bridge_result_json(&BridgeResult { ok: true, kind: kind.to_owned(), error: None })
}
pub(crate) fn error_result(error: &str) -> String {
    bridge_result_json(&BridgeResult { ok: false, kind: "error".into(), error: Some(error.to_owned()) })
}
pub(crate) fn bridge_result_json(result: &BridgeResult) -> String {
    let mut output = String::from("{\"ok\":");
    output.push_str(if result.ok { "true" } else { "false" });
    output.push_str(",\"kind\":\"");
    push_json_string(&result.kind, &mut output);
    output.push_str("\",\"error\":");
    match &result.error {
        Some(error) => { output.push('"'); push_json_string(error, &mut output); output.push('"'); }
        None => output.push_str("null"),
    }
    output.push('}');
    output
}
pub(crate) fn empty_snapshot_json() -> String {
    format!("{{\"contractVersion\":{CONTRACT_VERSION},\"identity\":null,\"torState\":\"stopped\",\"onionAddress\":null,\"pairings\":[],\"contacts\":[],\"conversations\":[],\"messages\":[],\"attachments\":[]}}")
}
pub(crate) fn bridge_snapshot_json(snapshot: &BridgeSnapshot) -> String {
    let mut output = String::new();
    let _ = write!(output, "{{\"contractVersion\":{}", snapshot.contract_version);
    output.push_str(",\"identity\":");
    match &snapshot.identity_name {
        Some(name) => {
            output.push_str("{\"displayName\":\"");
            push_json_string(name, &mut output);
            output.push_str("\"}");
        }
        None => output.push_str("null"),
    }
    output.push_str(",\"torState\":\"");
    push_json_string(&snapshot.tor_state, &mut output);
    output.push('"');
    output.push_str(",\"onionAddress\":");
    match &snapshot.onion_address {
        Some(value) => { output.push('"'); push_json_string(value, &mut output); output.push('"'); }
        None => output.push_str("null"),
    }

    output.push_str(",\"pairings\":[");
    for (index, pairing) in snapshot.pairings.iter().enumerate() {
        if index != 0 { output.push(','); }
        output.push_str("{\"id\":\""); push_json_string(&pairing.id, &mut output);
        output.push_str("\",\"code\":\""); push_json_string(&pairing.code, &mut output);
        output.push_str("\",\"role\":\""); push_json_string(&pairing.role, &mut output);
        output.push_str("\",\"state\":\""); push_json_string(&pairing.state, &mut output);
        let _ = write!(output, "\",\"expiresAtMs\":{},\"localApproved\":{},\"remoteApproved\":{}}}", pairing.expires_at_ms, pairing.local_approved, pairing.remote_approved);
    }
    output.push_str("],\"contacts\":[");
    for (index, contact) in snapshot.contacts.iter().enumerate() {
        if index != 0 { output.push(','); }
        output.push_str("{\"id\":\""); push_json_string(&contact.id, &mut output);
        output.push_str("\",\"displayName\":\""); push_json_string(&contact.display_name, &mut output);
        output.push_str("\",\"onionAddress\":\""); push_json_string(&contact.onion_address, &mut output);
        output.push_str("\",\"status\":\""); push_json_string(&contact.status, &mut output);
        output.push_str("\",\"connectionState\":\""); push_json_string(&contact.connection_state, &mut output);
        output.push_str("\",\"safetyNumber\":\""); push_json_string(&contact.safety_number, &mut output);
        output.push_str("\"}");
    }
    output.push_str("],\"conversations\":[");
    for (index, conversation) in snapshot.conversations.iter().enumerate() {
        if index != 0 { output.push(','); }
        output.push_str("{\"id\":\""); push_json_string(&conversation.id, &mut output);
        output.push_str("\",\"contactId\":\""); push_json_string(&conversation.contact_id, &mut output);
        output.push_str("\",\"status\":\""); push_json_string(&conversation.status, &mut output);
        output.push_str("\"}");
    }
    output.push_str("],\"messages\":[");
    for (index, message) in snapshot.messages.iter().enumerate() {
        if index != 0 { output.push(','); }
        output.push_str("{\"id\":\""); push_json_string(&message.id, &mut output);
        output.push_str("\",\"conversationId\":\""); push_json_string(&message.conversation_id, &mut output);
        output.push_str("\",\"body\":\""); push_json_string(&message.body, &mut output);
        output.push_str("\",\"direction\":\""); push_json_string(&message.direction, &mut output);
        output.push_str("\",\"status\":\""); push_json_string(&message.status, &mut output);
        output.push_str("\",\"replyToMessageId\":");
        match &message.reply_to_message_id {
            Some(value) => { output.push('"'); push_json_string(value, &mut output); output.push('"'); }
            None => output.push_str("null"),
        }
        output.push('}');
    }
    output.push_str("],\"attachments\":[");
    for (index, attachment) in snapshot.attachments.iter().enumerate() {
        if index != 0 { output.push(','); }
        output.push_str("{\"id\":\""); push_json_string(&attachment.id, &mut output);
        output.push_str("\",\"messageId\":\""); push_json_string(&attachment.message_id, &mut output);
        output.push_str("\",\"name\":\""); push_json_string(&attachment.name, &mut output);
        output.push_str("\",\"mediaType\":\""); push_json_string(&attachment.media_type, &mut output);
        let _ = write!(output, "\",\"size\":{},\"status\":\"", attachment.size);
        push_json_string(&attachment.status, &mut output);
        let _ = write!(output, "\",\"offset\":{}}}", attachment.offset);
    }
    output.push_str("]}");
    output
}

fn push_json_string(value: &str, output: &mut String) {
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character.is_control() => { let _ = write!(output, "\\u{:04x}", character as u32); }
            character => output.push(character),
        }
    }
}
