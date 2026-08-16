use std::collections::BTreeSet;

use torca_communication_driver::{
    ATTACHMENT_MESSAGE_KIND, PROBE_MESSAGE_KIND, RADIO_CONTROL_MESSAGE_KIND,
    REACTION_MESSAGE_KIND, RECEIPT_MESSAGE_KIND, TEXT_MESSAGE_KIND,
};
use torca_delivery::{ApplicationPayload, ApplicationPayloadCodec, ReactionPayload};
use torca_foundation::{OpaqueId, Timestamp};
use torca_peer_protocol::PeerApplicationKind;

#[test]
fn production_peer_kinds_match_the_protocol_registry() {
    let aliases = [
        TEXT_MESSAGE_KIND,
        RECEIPT_MESSAGE_KIND,
        ATTACHMENT_MESSAGE_KIND,
        PROBE_MESSAGE_KIND,
        RADIO_CONTROL_MESSAGE_KIND,
        REACTION_MESSAGE_KIND,
    ];
    let protocol = PeerApplicationKind::ALL.map(PeerApplicationKind::as_u16);

    assert_eq!(aliases, protocol);
    assert_eq!(
        aliases.iter().copied().collect::<BTreeSet<_>>().len(),
        aliases.len()
    );
}

#[test]
fn reaction_payload_has_a_dedicated_wire_kind_and_round_trips() {
    assert_ne!(REACTION_MESSAGE_KIND, ATTACHMENT_MESSAGE_KIND);

    let reaction = ReactionPayload {
        reaction_id: OpaqueId::from_u128(1),
        message_id: OpaqueId::from_u128(2),
        conversation_id: OpaqueId::from_u128(3),
        actor_id: OpaqueId::from_u128(4),
        emoji: "👍".to_owned(),
        active: true,
        at: Timestamp::from_unix_millis(5).expect("timestamp"),
    };
    let encoded = ApplicationPayloadCodec::encode(&ApplicationPayload::Reaction(reaction.clone()))
        .expect("encode reaction");
    assert_eq!(
        ApplicationPayloadCodec::decode(&encoded).expect("decode reaction"),
        ApplicationPayload::Reaction(reaction)
    );
}
