fn observe(
    &mut self,
    contact_id: ContactId,
    direction: Option<TransportDirection>,
    operation: TransportOperation,
    phase: OperationPhase,
    correlation_id: Option<OpaqueId>,
    at: Timestamp,
) {
    let activity = self.activity.entry(contact_id).or_default();
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
    if let Some(observer) = &self.connectivity {
        observer.record(
            TransportLayer::Peer(Some(contact_id.to_opaque())),
            direction,
            operation,
            phase,
            correlation_id,
            at,
            None,
            None,
        );
        observer.record(
            TransportLayer::Tor,
            direction,
            operation,
            phase,
            correlation_id,
            at,
            None,
            None,
        );
    }
}

fn observe_send_ack(
    &mut self,
    contact_id: ContactId,
    envelope_id: OpaqueId,
    result: &Result<(), PeerLinkError>,
) {
    if let Ok(now) = system_timestamp() {
        self.observe(
            contact_id,
            Some(TransportDirection::Tx),
            TransportOperation::Ack,
            if result.is_ok() {
                OperationPhase::Completed
            } else {
                OperationPhase::Failed
            },
            Some(envelope_id),
            now,
        );
    }
}
