use torca_communication_driver::{
    ATTACHMENT_MESSAGE_KIND, PROBE_MESSAGE_KIND, RADIO_CONTROL_MESSAGE_KIND, REACTION_MESSAGE_KIND,
    RECEIPT_MESSAGE_KIND, TEXT_MESSAGE_KIND,
};
use torca_peer_protocol::PeerApplicationKind;

#[test]
fn application_message_registry_matches_peer_wire_registry() {
    assert_eq!(TEXT_MESSAGE_KIND, PeerApplicationKind::Text.as_u16());
    assert_eq!(RECEIPT_MESSAGE_KIND, PeerApplicationKind::Receipt.as_u16());
    assert_eq!(ATTACHMENT_MESSAGE_KIND, PeerApplicationKind::Attachment.as_u16());
    assert_eq!(PROBE_MESSAGE_KIND, PeerApplicationKind::Probe.as_u16());
    assert_eq!(RADIO_CONTROL_MESSAGE_KIND, PeerApplicationKind::RadioControl.as_u16());
    assert_eq!(REACTION_MESSAGE_KIND, PeerApplicationKind::Reaction.as_u16());
}
