use std::collections::BTreeMap;

use torca_connectivity::{OperationPhase, TransportDirection, TransportOperation};
use torca_contacts::ContactId;
use torca_foundation::Timestamp;

/// Cumulative, payload-free transport activity for one relationship. The
/// runtime samples monotonic counters and turns deltas into health evidence.
#[must_use]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PeerActivitySnapshot {
    pub sequence: u64,
    pub tx_frames: u64,
    pub rx_frames: u64,
    pub tx_acks: u64,
    pub rx_acks: u64,
    pub handshakes: u64,
    pub failures: u64,
    pub last_activity_at: Option<Timestamp>,
}

#[derive(Default)]
pub(super) struct ActivityLedger {
    peers: BTreeMap<ContactId, PeerActivitySnapshot>,
}

impl ActivityLedger {
    pub(super) fn snapshot(&self) -> BTreeMap<ContactId, PeerActivitySnapshot> {
        self.peers.clone()
    }

    pub(super) fn record(
        &mut self,
        contact_id: ContactId,
        direction: Option<TransportDirection>,
        operation: TransportOperation,
        phase: OperationPhase,
        at: Timestamp,
    ) {
        let activity = self.peers.entry(contact_id).or_default();
        let completed = phase == OperationPhase::Completed;
        if completed {
            match (direction, operation) {
                (Some(TransportDirection::Tx), TransportOperation::Envelope) => {
                    activity.tx_frames = activity.tx_frames.saturating_add(1);
                }
                (Some(TransportDirection::Rx), TransportOperation::Envelope) => {
                    activity.rx_frames = activity.rx_frames.saturating_add(1);
                }
                (Some(TransportDirection::Tx), TransportOperation::Ack) => {
                    activity.tx_acks = activity.tx_acks.saturating_add(1);
                }
                (Some(TransportDirection::Rx), TransportOperation::Ack) => {
                    activity.rx_acks = activity.rx_acks.saturating_add(1);
                }
                (Some(TransportDirection::Rx), TransportOperation::Handshake) => {
                    activity.handshakes = activity.handshakes.saturating_add(1);
                }
                _ => {}
            }
        }
        if matches!(phase, OperationPhase::Failed | OperationPhase::TimedOut) {
            activity.failures = activity.failures.saturating_add(1);
        }
        if direction.is_some()
            && (completed || matches!(phase, OperationPhase::Failed | OperationPhase::TimedOut))
        {
            activity.last_activity_at = Some(at);
        }
        activity.sequence = activity.sequence.saturating_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use torca_foundation::OpaqueId;

    #[test]
    fn recording_transport_activity_is_monotonic() {
        let mut ledger = ActivityLedger::default();
        let contact = ContactId::from_opaque(OpaqueId::from_u128(1));
        let at = Timestamp::from_unix_millis(1).expect("valid timestamp");
        ledger.record(
            contact,
            Some(TransportDirection::Tx),
            TransportOperation::Envelope,
            OperationPhase::Completed,
            at,
        );
        ledger.record(
            contact,
            Some(TransportDirection::Rx),
            TransportOperation::Ack,
            OperationPhase::Completed,
            at,
        );
        let snapshot = ledger.snapshot();
        let activity = snapshot.get(&contact).expect("contact activity");
        assert_eq!(activity.sequence, 2);
        assert_eq!(activity.tx_frames, 1);
        assert_eq!(activity.rx_acks, 1);
        assert_eq!(activity.last_activity_at, Some(at));
    }
}
