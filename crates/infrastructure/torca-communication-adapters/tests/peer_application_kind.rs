use std::collections::BTreeSet;

use torca_communication_driver::{
    ATTACHMENT_MESSAGE_KIND, PROBE_MESSAGE_KIND, RADIO_CONTROL_MESSAGE_KIND,
    REACTION_MESSAGE_KIND, RECEIPT_MESSAGE_KIND, TEXT_MESSAGE_KIND,
};
use torca_peer_protocol::PeerApplicationKind;

#[test]
fn application_and_wire_message_kind_registries_stay_in_sync() {
    let application = [
        TEXT_MESSAGE_KIND,
        RECEIPT_MESSAGE_KIND,
        ATTACHMENT_MESSAGE_KIND,
        PROBE_MESSAGE_KIND,
        RADIO_CONTROL_MESSAGE_KIND,
        REACTION_MESSAGE_KIND,
    ];
    let wire = PeerApplicationKind::ALL.map(PeerApplicationKind::as_u16);

    assert_eq!(application, wire);
    assert_eq!(application, [1, 2, 3, 4, 5, 6]);
    assert_eq!(
        application.into_iter().collect::<BTreeSet<_>>().len(),
        PeerApplicationKind::ALL.len()
    );
}
