use torca_contacts::ContactId;
use torca_foundation::OpaqueId;
use torca_peer_protocol::{AckStatus, PeerMessage};

use crate::{InboundPeerEnvelope, LinkAck, PeerLinkError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum AckWaitAction {
    Complete(Result<LinkAck, PeerLinkError>),
    Store {
        envelope_id: OpaqueId,
        ack: Result<LinkAck, PeerLinkError>,
    },
    QueueInbound(InboundPeerEnvelope),
    Ignore,
}

pub(crate) fn link_ack(status: AckStatus) -> Result<LinkAck, PeerLinkError> {
    match status {
        AckStatus::Accepted => Ok(LinkAck::Accepted),
        AckStatus::Duplicate => Ok(LinkAck::Duplicate),
        AckStatus::Rejected => Err(PeerLinkError::AckRejected),
    }
}

pub(crate) fn classify_ack_wait_message(
    contact_id: ContactId,
    expected_envelope_id: OpaqueId,
    message: PeerMessage,
) -> AckWaitAction {
    match message {
        PeerMessage::Ack { envelope_id, status } => {
            let ack = link_ack(status);
            if envelope_id == expected_envelope_id {
                AckWaitAction::Complete(ack)
            } else {
                AckWaitAction::Store { envelope_id, ack }
            }
        }
        PeerMessage::Data {
            envelope_id,
            message_kind,
            ciphertext,
        } => AckWaitAction::QueueInbound(InboundPeerEnvelope {
            contact_id,
            envelope_id,
            message_kind,
            ciphertext,
        }),
        _ => AckWaitAction::Ignore,
    }
}

#[cfg(test)]
mod tests {
    use super::{AckWaitAction, classify_ack_wait_message};
    use crate::LinkAck;
    use torca_contacts::ContactId;
    use torca_foundation::OpaqueId;
    use torca_peer_protocol::{AckStatus, PeerMessage};

    #[test]
    fn ram_only_inbound_never_completes_transport_receipt() {
        let contact_id = ContactId::from_u128(1);
        let expected = OpaqueId::from_u128(2);
        let incoming = OpaqueId::from_u128(3);
        let action = classify_ack_wait_message(
            contact_id,
            expected,
            PeerMessage::Data {
                envelope_id: incoming,
                message_kind: 7,
                ciphertext: vec![1, 2, 3],
            },
        );
        let AckWaitAction::QueueInbound(envelope) = action else {
            panic!("inbound application data must yield to durable ingress");
        };
        assert_eq!(envelope.contact_id, contact_id);
        assert_eq!(envelope.envelope_id, incoming);
        assert_eq!(envelope.message_kind, 7);
        assert_eq!(envelope.ciphertext, vec![1, 2, 3]);
    }

    #[test]
    fn simultaneous_bidirectional_data_yields_both_ack_waiters() {
        for (contact, expected, incoming) in [
            (ContactId::from_u128(10), OpaqueId::from_u128(11), OpaqueId::from_u128(12)),
            (ContactId::from_u128(20), OpaqueId::from_u128(21), OpaqueId::from_u128(22)),
        ] {
            let action = classify_ack_wait_message(
                contact,
                expected,
                PeerMessage::Data {
                    envelope_id: incoming,
                    message_kind: 1,
                    ciphertext: vec![9],
                },
            );
            assert!(matches!(action, AckWaitAction::QueueInbound(_)));
        }
    }

    #[test]
    fn matching_ack_completes_the_wait() {
        let expected = OpaqueId::from_u128(31);
        let action = classify_ack_wait_message(
            ContactId::from_u128(30),
            expected,
            PeerMessage::Ack {
                envelope_id: expected,
                status: AckStatus::Accepted,
            },
        );
        assert_eq!(action, AckWaitAction::Complete(Ok(LinkAck::Accepted)));
    }

    #[test]
    fn unrelated_ack_is_preserved_for_its_own_sender() {
        let expected = OpaqueId::from_u128(41);
        let other = OpaqueId::from_u128(42);
        let action = classify_ack_wait_message(
            ContactId::from_u128(40),
            expected,
            PeerMessage::Ack {
                envelope_id: other,
                status: AckStatus::Duplicate,
            },
        );
        assert_eq!(
            action,
            AckWaitAction::Store {
                envelope_id: other,
                ack: Ok(LinkAck::Duplicate),
            }
        );
    }
}
