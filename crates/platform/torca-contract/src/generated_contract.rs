// GENERATED FILE. DO NOT EDIT.
// Generated from: crates/platform/torca-contract/schema/torca_contract.json

pub const SCHEMA_VERSION: u16 = 1;
pub const CONTRACT_VERSION: u16 = 25;
pub const COMMANDS: &[&str] = &[
    "profile.set",
    "pairing.create",
    "pairing.join",
    "pairing.approve",
    "pairing.reject",
    "pairing.cancel",
    "contact.rename",
    "contact.verify",
    "contact.verification.reset",
    "contact.block",
    "contact.unblock",
    "contact.remove",
    "conversation.start",
    "conversation.clear",
    "conversation.archive",
    "conversation.restore",
    "message.send",
    "message.retry",
    "message.cancel",
    "message.delete",
    "message.edit",
    "message.reaction",
    "runtime.attention.set",
    "notifications.set",
    "privacy.read_receipts.set",
    "battery.preferences.set",
    "contact.availability.set",
    "contacts.acknowledge_new",
    "diagnostics.observation.start",
    "diagnostics.observation.stop",
    "diagnostics.observation.reset",
    "diagnostics.incident.mark",
    "conversation.read",
    "attachment.queue",
    "attachment.retry",
    "attachment.cancel",
    "attachment.export",
    "radio.set_enabled",
    "radio.audio.configure",
    "radio.transmission.begin",
    "radio.transmission.end",
    "provider.route.refresh",
];
pub const QUERIES: &[&str] = &[
    "snapshot.get",
    "conversation.page",
    "conversation.search",
    "notifications.poll",
    "runtime.poll",
    "diagnostics.get",
    "diagnostics.logs.tail",
    "pairing.parse",
    "pairing.encode",
    "avatars.get",
];

pub fn contains(kind: &str, name: &str) -> bool {
    match kind {
        "command" => COMMANDS.contains(&name),
        "query" => QUERIES.contains(&name),
        "lifecycle" => matches!(
            name,
            "host_started"
                | "foregrounded"
                | "backgrounded"
                | "network_changed"
                | "network_validated"
                | "flutter_gateway_ready"
                | "low_memory"
                | "terminating"
        ),
        _ => false,
    }
}
