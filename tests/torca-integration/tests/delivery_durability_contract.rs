use std::sync::{Arc, Mutex};

use torca_conversations::ConversationId;
use torca_delivery::{
    DeliveryAck, DeliveryTransportError, DurableDeliveryError, InboundAcknowledger,
    InboundDeliveryError, InboundDeliveryHandler, InboundMessageStore,
};
use torca_foundation::{OpaqueId, Timestamp};
use torca_messaging::{Message, MessageBody, MessageId};

#[derive(Clone)]
struct RecordingStore {
    events: Arc<Mutex<Vec<&'static str>>>,
    fail: bool,
}

impl InboundMessageStore for RecordingStore {
    fn persist_inbound(
        &mut self,
        _envelope_id: OpaqueId,
        _message: Message,
    ) -> Result<bool, DurableDeliveryError> {
        self.events.lock().expect("events").push("persist");
        if self.fail {
            return Err(DurableDeliveryError::Storage("injected".into()));
        }
        Ok(true)
    }
}

struct RecordingAcknowledger {
    events: Arc<Mutex<Vec<&'static str>>>,
}

impl InboundAcknowledger for RecordingAcknowledger {
    fn acknowledge(
        &mut self,
        _envelope_id: OpaqueId,
        _ack: DeliveryAck,
    ) -> Result<(), DeliveryTransportError> {
        self.events.lock().expect("events").push("ack");
        Ok(())
    }
}

fn inbound_message() -> Message {
    Message::inbound(
        MessageId::from_u128(1),
        ConversationId::from_u128(2),
        MessageBody::new("durable before ack").expect("body"),
        None,
        Timestamp::from_unix_millis(3).expect("timestamp"),
    )
}

#[test]
fn accepted_ack_is_emitted_only_after_durable_persist() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut handler = InboundDeliveryHandler::new(
        RecordingStore { events: Arc::clone(&events), fail: false },
        RecordingAcknowledger { events: Arc::clone(&events) },
    );

    assert_eq!(
        handler
            .handle(OpaqueId::from_u128(10), inbound_message())
            .expect("inbound delivery"),
        DeliveryAck::Accepted
    );
    assert_eq!(&*events.lock().expect("events"), &["persist", "ack"]);
}

#[test]
fn failed_persist_never_emits_transport_ack() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut handler = InboundDeliveryHandler::new(
        RecordingStore { events: Arc::clone(&events), fail: true },
        RecordingAcknowledger { events: Arc::clone(&events) },
    );

    assert!(matches!(
        handler.handle(OpaqueId::from_u128(11), inbound_message()),
        Err(InboundDeliveryError::Store(_))
    ));
    assert_eq!(&*events.lock().expect("events"), &["persist"]);
}
