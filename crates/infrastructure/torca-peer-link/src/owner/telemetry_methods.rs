struct TelemetryEvent {
    contact_id: ContactId,
    direction: Option<TransportDirection>,
    operation: TransportOperation,
    phase: OperationPhase,
    correlation_id: Option<OpaqueId>,
    at: Timestamp,
    stage: Option<TransportStage>,
    error_code: Option<torca_foundation::ErrorCode>,
}

impl<S, K> PeerLink<S, K>
where
    S: ContactRepository + PeerCredentialRepository,
    K: HandshakeSigner,
{

fn observe(
    &mut self,
    contact_id: ContactId,
    direction: Option<TransportDirection>,
    operation: TransportOperation,
    phase: OperationPhase,
    correlation_id: Option<OpaqueId>,
    at: Timestamp,
) {
    let stage = match operation {
        TransportOperation::Envelope => Some(TransportStage::Message),
        TransportOperation::Ack => Some(TransportStage::Receipt),
        _ => None,
    };
    self.observe_with_stage(TelemetryEvent {
        contact_id,
        direction,
        operation,
        phase,
        correlation_id,
        at,
        stage,
        error_code: None,
    });
}

fn observe_with_stage(&mut self, event: TelemetryEvent) {
    let TelemetryEvent {
        contact_id,
        direction,
        operation,
        phase,
        correlation_id,
        at,
        stage,
        error_code,
    } = event;
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
        observer.record_with_stage(
            TransportLayer::Peer(Some(contact_id.to_opaque())),
            direction,
            operation,
            phase,
            correlation_id,
            at,
            None,
            error_code,
            stage,
        );
        observer.record_with_stage(
            TransportLayer::Communication,
            direction,
            operation,
            phase,
            correlation_id,
            at,
            None,
            error_code,
            stage,
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
}
