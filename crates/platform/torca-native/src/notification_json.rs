use core::fmt::Write as _;
use torca_bridge::BridgeSnapshot;

/// Redacted process projection for the Android foreground service.
/// It intentionally omits message bodies, onion addresses, safety numbers and all secret material.
pub(crate) fn notification_snapshot_json(snapshot: &BridgeSnapshot) -> String {
    let mut output = String::from("{\"contacts\":[");
    for (index, contact) in snapshot.contacts.iter().enumerate() {
        if index != 0 { output.push(','); }
        output.push_str("{\"id\":\"");
        push_json_string(&contact.id, &mut output);
        output.push_str("\",\"displayName\":\"");
        push_json_string(&contact.display_name, &mut output);
        output.push_str("\"}");
    }
    output.push_str("],\"conversations\":[");
    for (index, conversation) in snapshot.conversations.iter().enumerate() {
        if index != 0 { output.push(','); }
        output.push_str("{\"id\":\"");
        push_json_string(&conversation.id, &mut output);
        output.push_str("\",\"contactId\":\"");
        push_json_string(&conversation.contact_id, &mut output);
        output.push_str("\"}");
    }
    output.push_str("],\"messages\":[");
    for (index, message) in snapshot.messages.iter().enumerate() {
        if index != 0 { output.push(','); }
        output.push_str("{\"id\":\"");
        push_json_string(&message.id, &mut output);
        output.push_str("\",\"conversationId\":\"");
        push_json_string(&message.conversation_id, &mut output);
        output.push_str("\",\"direction\":\"");
        push_json_string(&message.direction, &mut output);
        output.push_str("\"}");
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
            character if character.is_control() => {
                let _ = write!(output, "\\u{:04x}", character as u32);
            }
            character => output.push(character),
        }
    }
}
