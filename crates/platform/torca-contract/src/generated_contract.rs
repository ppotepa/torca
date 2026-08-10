// GENERATED FILE. DO NOT EDIT.
// Generated from: crates/platform/torca-contract/schema/torca_contract.json

pub const SCHEMA_VERSION: u16 = 1;
pub const CONTRACT_VERSION: u16 = 15;
pub const COMMANDS: &[&str] = &["profile.set", "pairing.create", "pairing.join", "pairing.approve", "pairing.reject", "pairing.cancel", "contact.rename", "contact.verify", "contact.verification.reset", "contact.block", "contact.unblock", "contact.remove", "conversation.clear", "message.send", "message.retry", "notifications.set", "contacts.acknowledge_new", "conversation.read", "attachment.queue", "attachment.retry", "attachment.cancel", "attachment.export"];
pub const QUERIES: &[&str] = &["snapshot.get", "conversation.page", "conversation.search", "notifications.poll", "pairing.parse"];

pub fn contains(kind: &str, name: &str) -> bool {
    match kind {
        "command" => COMMANDS.contains(&name),
        "query" => QUERIES.contains(&name),
        "lifecycle" => matches!(name, "host_started" | "foregrounded" | "backgrounded" | "network_changed" | "low_memory" | "terminating"),
        _ => false,
    }
}
