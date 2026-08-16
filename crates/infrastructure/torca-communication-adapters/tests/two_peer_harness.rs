use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use torca_attachment_protocol::{
    AttachmentChunkFrame, AttachmentCodec, AttachmentFrame, AttachmentMetadataFrame,
};
use torca_attachments::{AttachmentId, AttachmentName, MediaType};
use torca_communication_driver::{
    ATTACHMENT_MESSAGE_KIND, REACTION_MESSAGE_KIND, RECEIPT_MESSAGE_KIND, TEXT_MESSAGE_KIND,
};
use torca_conversations::ConversationId;
use torca_delivery::{
    ApplicationPayload, ApplicationPayloadCodec, DeliveryAck, DeliveryReceiptKind,
    DeliveryTransport, DeliveryTransportError, DeliveryWorker, DurableDeliveryError,
    DurableDeliveryStore, InboundAcknowledger, InboundDeliveryHandler, InboundMessageStore,
    OutboxRecord, OutboxState, ReactionPayload, ReceiptPayload, TextPayload,
};
use torca_foundation::{CommandId, OpaqueId, Timestamp};
use torca_messaging::{Message, MessageBody, MessageDirection, MessageId, MessageStatus, RetryPolicy};
use torca_peer_protocol::{AckStatus, PeerCodec, PeerMessage};

const ACK_TIMEOUT: Duration = Duration::from_millis(80);
const WIRE_WAIT: Duration = Duration::from_millis(10);

fn ts(ms: i64) -> Timestamp {
    Timestamp::from_unix_millis(ms).expect("timestamp")
}

#[derive(Clone, Default)]
struct SharedStore {
    inner: Arc<Mutex<StoreState>>,
}

#[derive(Default)]
struct StoreState {
    outbox: BTreeMap<MessageId, OutboxRecord>,
    inbound_envelopes: BTreeSet<OpaqueId>,
    inbound_messages: BTreeMap<MessageId, Message>,
}

impl SharedStore {
    fn inbound(&self, id: MessageId) -> Option<Message> {
        self.inner
            .lock()
            .expect("store")
            .inbound_messages
            .get(&id)
            .cloned()
    }

    fn outbox_state(&self, id: MessageId) -> Option<OutboxState> {
        self.inner
            .lock()
            .expect("store")
            .outbox
            .get(&id)
            .map(|record| record.state)
    }

    fn outbox_attempts(&self, id: MessageId) -> Option<u32> {
        self.inner
            .lock()
            .expect("store")
            .outbox
            .get(&id)
            .map(|record| record.attempts)
    }
}

impl DurableDeliveryStore for SharedStore {
    fn queue_outbound(
        &mut self,
        message: Message,
        command_id: CommandId,
        next_attempt_at: Timestamp,
    ) -> Result<(), DurableDeliveryError> {
        let mut state = self.inner.lock().expect("store");
        if state.outbox.contains_key(&message.id()) {
            return Err(DurableDeliveryError::DuplicateMessage);
        }
        state.outbox.insert(
            message.id(),
            OutboxRecord {
                message,
                command_id,
                attempts: 0,
                next_attempt_at,
                claimed_at: None,
                state: OutboxState::Pending,
            },
        );
        Ok(())
    }

    fn claim_due(
        &mut self,
        now: Timestamp,
        limit: usize,
    ) -> Result<Vec<OutboxRecord>, DurableDeliveryError> {
        let mut state = self.inner.lock().expect("store");
        let ids = state
            .outbox
            .iter()
            .filter(|(_, record)| {
                record.state == OutboxState::Pending && record.next_attempt_at <= now
            })
            .take(limit)
            .map(|(id, _)| *id)
            .collect::<Vec<_>>();
        let mut claimed = Vec::with_capacity(ids.len());
        for id in ids {
            let record = state.outbox.get_mut(&id).expect("claimed outbox record");
            record.state = OutboxState::Claimed;
            record.claimed_at = Some(now);
            claimed.push(record.clone());
        }
        Ok(claimed)
    }

    fn reschedule(
        &mut self,
        message_id: MessageId,
        attempts: u32,
        next_attempt_at: Timestamp,
    ) -> Result<(), DurableDeliveryError> {
        let mut state = self.inner.lock().expect("store");
        let record = state.outbox.get_mut(&message_id).ok_or(DurableDeliveryError::NotFound)?;
        if record.state != OutboxState::Claimed {
            return Err(DurableDeliveryError::InvalidState);
        }
        record.attempts = attempts;
        record.next_attempt_at = next_attempt_at;
        record.claimed_at = None;
        record.state = OutboxState::Pending;
        Ok(())
    }

    fn complete(&mut self, message_id: MessageId) -> Result<(), DurableDeliveryError> {
        let mut state = self.inner.lock().expect("store");
        let record = state.outbox.get_mut(&message_id).ok_or(DurableDeliveryError::NotFound)?;
        if record.state != OutboxState::Claimed {
            return Err(DurableDeliveryError::InvalidState);
        }
        record.claimed_at = None;
        record.state = OutboxState::Completed;
        Ok(())
    }

    fn dead_letter(&mut self, message_id: MessageId) -> Result<(), DurableDeliveryError> {
        let mut state = self.inner.lock().expect("store");
        let record = state.outbox.get_mut(&message_id).ok_or(DurableDeliveryError::NotFound)?;
        record.claimed_at = None;
        record.state = OutboxState::DeadLetter;
        Ok(())
    }

    fn recover_stale_claims(
        &mut self,
        claimed_before: Timestamp,
    ) -> Result<usize, DurableDeliveryError> {
        let mut state = self.inner.lock().expect("store");
        let mut recovered = 0;
        for record in state.outbox.values_mut() {
            if record.state == OutboxState::Claimed
                && record.claimed_at.is_some_and(|claimed| claimed <= claimed_before)
            {
                record.state = OutboxState::Pending;
                record.claimed_at = None;
                recovered += 1;
            }
        }
        Ok(recovered)
    }

    fn record_inbound(&mut self, envelope_id: OpaqueId) -> Result<bool, DurableDeliveryError> {
        Ok(self
            .inner
            .lock()
            .expect("store")
            .inbound_envelopes
            .insert(envelope_id))
    }

    fn next_due(&self) -> Result<Option<Timestamp>, DurableDeliveryError> {
        Ok(self
            .inner
            .lock()
            .expect("store")
            .outbox
            .values()
            .filter(|record| record.state == OutboxState::Pending)
            .map(|record| record.next_attempt_at)
            .min())
    }
}

impl InboundMessageStore for SharedStore {
    fn persist_inbound(
        &mut self,
        envelope_id: OpaqueId,
        message: Message,
    ) -> Result<bool, DurableDeliveryError> {
        if message.direction() != MessageDirection::Inbound
            || message.status() != MessageStatus::Delivered
        {
            return Err(DurableDeliveryError::InvalidState);
        }
        let mut state = self.inner.lock().expect("store");
        if state.inbound_envelopes.contains(&envelope_id) {
            return Ok(false);
        }
        if state.inbound_messages.contains_key(&message.id()) {
            return Err(DurableDeliveryError::DuplicateMessage);
        }
        state.inbound_envelopes.insert(envelope_id);
        state.inbound_messages.insert(message.id(), message);
        Ok(true)
    }
}

#[derive(Default)]
struct FaultPlan {
    drop_next_ack: bool,
    duplicate_next_data: bool,
}

#[derive(Clone)]
struct WireSender {
    tx: SyncSender<Vec<u8>>,
    faults: Arc<Mutex<FaultPlan>>,
}

impl WireSender {
    fn send(&self, frame: Vec<u8>) -> Result<(), DeliveryTransportError> {
        let message = PeerCodec::decode(&frame)
            .map_err(|error| DeliveryTransportError(format!("decode outgoing frame: {error}")))?;
        let mut faults = self.faults.lock().expect("fault plan");
        if matches!(message, PeerMessage::Ack { .. }) && faults.drop_next_ack {
            faults.drop_next_ack = false;
            return Ok(());
        }
        if matches!(message, PeerMessage::Data { .. }) && faults.duplicate_next_data {
            faults.duplicate_next_data = false;
            self.tx
                .send(frame.clone())
                .map_err(|_| DeliveryTransportError("wire closed".into()))?;
        }
        self.tx
            .send(frame)
            .map_err(|_| DeliveryTransportError("wire closed".into()))
    }

    fn drop_next_ack(&self) {
        self.faults.lock().expect("fault plan").drop_next_ack = true;
    }

    fn duplicate_next_data(&self) {
        self.faults.lock().expect("fault plan").duplicate_next_data = true;
    }
}

#[derive(Clone)]
struct TestDeliveryTransport {
    remote_contact_id: OpaqueId,
    wire: WireSender,
    ack_rx: Arc<Mutex<Receiver<(OpaqueId, AckStatus)>>>,
}

impl TestDeliveryTransport {
    fn wait_for_ack(&self, envelope_id: OpaqueId) -> Result<DeliveryAck, DeliveryTransportError> {
        let receiver = self.ack_rx.lock().expect("ack receiver");
        loop {
            match receiver.recv_timeout(ACK_TIMEOUT) {
                Ok((received, status)) if received == envelope_id => {
                    return match status {
                        AckStatus::Accepted => Ok(DeliveryAck::Accepted),
                        AckStatus::Duplicate => Ok(DeliveryAck::Duplicate),
                        AckStatus::Rejected => Err(DeliveryTransportError("rejected".into())),
                    };
                }
                Ok(_) => continue,
                Err(_) => return Err(DeliveryTransportError("ack timeout".into())),
            }
        }
    }

    fn send_data(
        &self,
        envelope_id: OpaqueId,
        message_kind: u16,
        ciphertext: Vec<u8>,
    ) -> Result<DeliveryAck, DeliveryTransportError> {
        let frame = PeerCodec::encode(&PeerMessage::Data {
            envelope_id,
            message_kind,
            ciphertext,
        })
        .map_err(|error| DeliveryTransportError(format!("encode peer frame: {error}")))?;
        self.wire.send(frame)?;
        self.wait_for_ack(envelope_id)
    }
}

impl DeliveryTransport for TestDeliveryTransport {
    fn send(&mut self, message: &Message) -> Result<DeliveryAck, DeliveryTransportError> {
        let payload = ApplicationPayloadCodec::encode(&ApplicationPayload::Text(TextPayload {
            message_id: message.id().to_opaque(),
            conversation_id: message.conversation_id().to_opaque(),
            contact_id: self.remote_contact_id,
            body: message.body().as_str().to_owned(),
            reply_to: message.reply_to().map(|reply| reply.message_id.to_opaque()),
            sent_at: message.created_at(),
        }))
        .map_err(|error| DeliveryTransportError(format!("encode text payload: {error}")))?;
        self.send_data(message.id().to_opaque(), TEXT_MESSAGE_KIND, payload)
    }
}

struct WireAcknowledger {
    wire: WireSender,
}

impl InboundAcknowledger for WireAcknowledger {
    fn acknowledge(
        &mut self,
        envelope_id: OpaqueId,
        ack: DeliveryAck,
    ) -> Result<(), DeliveryTransportError> {
        let status = match ack {
            DeliveryAck::Accepted => AckStatus::Accepted,
            DeliveryAck::Duplicate => AckStatus::Duplicate,
        };
        let frame = PeerCodec::encode(&PeerMessage::Ack { envelope_id, status })
            .map_err(|error| DeliveryTransportError(format!("encode ack: {error}")))?;
        self.wire.send(frame)
    }
}

#[derive(Default)]
struct InboundArtifacts {
    controls: Vec<ApplicationPayload>,
    attachments: Vec<AttachmentFrame>,
}

struct Endpoint {
    store: SharedStore,
    transport: TestDeliveryTransport,
    artifacts: Arc<Mutex<InboundArtifacts>>,
    wire: WireSender,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl Endpoint {
    fn delivery_lane(&self) -> DeliveryLane {
        DeliveryLane { store: self.store.clone(), transport: self.transport.clone() }
    }

    fn send_payload(
        &self,
        envelope_id: OpaqueId,
        kind: u16,
        payload: Vec<u8>,
    ) -> DeliveryAck {
        self.transport
            .send_data(envelope_id, kind, payload)
            .expect("control/attachment ack")
    }

    fn controls(&self) -> Vec<ApplicationPayload> {
        self.artifacts.lock().expect("artifacts").controls.clone()
    }

    fn attachments(&self) -> Vec<AttachmentFrame> {
        self.artifacts.lock().expect("artifacts").attachments.clone()
    }
}

impl Drop for Endpoint {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[derive(Clone)]
struct DeliveryLane {
    store: SharedStore,
    transport: TestDeliveryTransport,
}

impl DeliveryLane {
    fn queue(&self, message: Message, at: Timestamp) {
        self.store
            .clone()
            .queue_outbound(message, CommandId::from_u128(900), at)
            .expect("queue outbound");
    }

    fn run(&self, now: Timestamp) {
        let policy = RetryPolicy {
            max_attempts: 4,
            base_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(4),
        };
        let mut worker = DeliveryWorker::new(self.store.clone(), self.transport.clone(), policy);
        worker.run_once(now, 8).expect("delivery maintenance");
    }
}

struct TestPair {
    a: Endpoint,
    b: Endpoint,
}

impl TestPair {
    fn new() -> Self {
        let (a_to_b_tx, a_to_b_rx) = mpsc::sync_channel(64);
        let (b_to_a_tx, b_to_a_rx) = mpsc::sync_channel(64);
        let a_wire = WireSender {
            tx: a_to_b_tx,
            faults: Arc::new(Mutex::new(FaultPlan::default())),
        };
        let b_wire = WireSender {
            tx: b_to_a_tx,
            faults: Arc::new(Mutex::new(FaultPlan::default())),
        };
        let a = spawn_endpoint(
            OpaqueId::from_u128(1),
            OpaqueId::from_u128(2),
            a_wire.clone(),
            b_wire.clone(),
            b_to_a_rx,
        );
        let b = spawn_endpoint(
            OpaqueId::from_u128(2),
            OpaqueId::from_u128(1),
            b_wire,
            a_wire,
            a_to_b_rx,
        );
        Self { a, b }
    }
}

fn spawn_endpoint(
    _local_contact_id: OpaqueId,
    remote_contact_id: OpaqueId,
    outbound_wire: WireSender,
    ack_wire: WireSender,
    inbound_rx: Receiver<Vec<u8>>,
) -> Endpoint {
    let store = SharedStore::default();
    let artifacts = Arc::new(Mutex::new(InboundArtifacts::default()));
    let stop = Arc::new(AtomicBool::new(false));
    let (ack_tx, ack_rx) = mpsc::sync_channel(64);
    let worker_store = store.clone();
    let worker_artifacts = Arc::clone(&artifacts);
    let worker_stop = Arc::clone(&stop);
    let worker_ack_wire = ack_wire.clone();
    let worker = thread::spawn(move || {
        while !worker_stop.load(Ordering::Acquire) {
            let frame = match inbound_rx.recv_timeout(WIRE_WAIT) {
                Ok(frame) => frame,
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => break,
            };
            let message = PeerCodec::decode(&frame).expect("decode peer wire");
            match message {
                PeerMessage::Ack { envelope_id, status } => {
                    let _ = ack_tx.send((envelope_id, status));
                }
                PeerMessage::Data { envelope_id, message_kind, ciphertext } => {
                    handle_inbound_data(
                        worker_store.clone(),
                        Arc::clone(&worker_artifacts),
                        worker_ack_wire.clone(),
                        envelope_id,
                        message_kind,
                        ciphertext,
                    );
                }
                _ => {}
            }
        }
    });

    Endpoint {
        store,
        transport: TestDeliveryTransport {
            remote_contact_id,
            wire: outbound_wire.clone(),
            ack_rx: Arc::new(Mutex::new(ack_rx)),
        },
        artifacts,
        wire: outbound_wire,
        stop,
        worker: Some(worker),
    }
}

fn handle_inbound_data(
    store: SharedStore,
    artifacts: Arc<Mutex<InboundArtifacts>>,
    ack_wire: WireSender,
    envelope_id: OpaqueId,
    message_kind: u16,
    ciphertext: Vec<u8>,
) {
    match message_kind {
        TEXT_MESSAGE_KIND => {
            let ApplicationPayload::Text(text) =
                ApplicationPayloadCodec::decode(&ciphertext).expect("decode text payload")
            else {
                panic!("text kind must contain text payload");
            };
            let message = Message::inbound(
                MessageId::from_opaque(text.message_id),
                ConversationId::from_opaque(text.conversation_id),
                MessageBody::new(text.body).expect("message body"),
                text.reply_to.map(|message_id| torca_messaging::ReplyReference {
                    message_id: MessageId::from_opaque(message_id),
                }),
                text.sent_at,
            );
            let mut handler = InboundDeliveryHandler::new(
                store,
                WireAcknowledger { wire: ack_wire },
            );
            handler.handle(envelope_id, message).expect("durable inbound text");
        }
        RECEIPT_MESSAGE_KIND | REACTION_MESSAGE_KIND => {
            let payload = ApplicationPayloadCodec::decode(&ciphertext).expect("control payload");
            artifacts.lock().expect("artifacts").controls.push(payload);
            send_ack(ack_wire, envelope_id, AckStatus::Accepted);
        }
        ATTACHMENT_MESSAGE_KIND => {
            let frame = AttachmentCodec::decode(&ciphertext).expect("attachment payload");
            artifacts.lock().expect("artifacts").attachments.push(frame);
            send_ack(ack_wire, envelope_id, AckStatus::Accepted);
        }
        _ => send_ack(ack_wire, envelope_id, AckStatus::Rejected),
    }
}

fn send_ack(wire: WireSender, envelope_id: OpaqueId, status: AckStatus) {
    let frame = PeerCodec::encode(&PeerMessage::Ack { envelope_id, status }).expect("encode ack");
    wire.send(frame).expect("send ack");
}

fn outbound_message(id: u128, conversation: u128, body: &str, at: i64) -> Message {
    Message::outbound(
        MessageId::from_u128(id),
        ConversationId::from_u128(conversation),
        MessageBody::new(body).expect("body"),
        None,
        ts(at),
    )
}

#[test]
fn two_peers_deliver_text_simultaneously_exactly_once() {
    let pair = TestPair::new();
    let a_lane = pair.a.delivery_lane();
    let b_lane = pair.b.delivery_lane();
    a_lane.queue(outbound_message(11, 101, "a to b", 10), ts(10));
    b_lane.queue(outbound_message(22, 202, "b to a", 10), ts(10));

    let a_run = thread::spawn(move || a_lane.run(ts(10)));
    let b_run = thread::spawn(move || b_lane.run(ts(10)));
    a_run.join().expect("a delivery thread");
    b_run.join().expect("b delivery thread");

    assert_eq!(pair.a.store.outbox_state(MessageId::from_u128(11)), Some(OutboxState::Completed));
    assert_eq!(pair.b.store.outbox_state(MessageId::from_u128(22)), Some(OutboxState::Completed));
    assert_eq!(
        pair.b.store.inbound(MessageId::from_u128(11)).expect("b received").body().as_str(),
        "a to b"
    );
    assert_eq!(
        pair.a.store.inbound(MessageId::from_u128(22)).expect("a received").body().as_str(),
        "b to a"
    );
}

#[test]
fn dropped_ack_retries_same_envelope_and_completes_as_duplicate() {
    let pair = TestPair::new();
    let lane = pair.a.delivery_lane();
    lane.queue(outbound_message(31, 301, "retry after lost ack", 20), ts(20));
    pair.b.wire.drop_next_ack();

    lane.run(ts(20));
    assert_eq!(pair.a.store.outbox_state(MessageId::from_u128(31)), Some(OutboxState::Pending));
    assert_eq!(pair.a.store.outbox_attempts(MessageId::from_u128(31)), Some(1));
    assert!(pair.b.store.inbound(MessageId::from_u128(31)).is_some());

    lane.run(ts(1_020));
    assert_eq!(pair.a.store.outbox_state(MessageId::from_u128(31)), Some(OutboxState::Completed));
    assert_eq!(
        pair.b.store.inbound(MessageId::from_u128(31)).expect("single durable inbound").body().as_str(),
        "retry after lost ack"
    );
}

#[test]
fn duplicated_wire_data_is_deduplicated_before_ack_completion() {
    let pair = TestPair::new();
    let lane = pair.a.delivery_lane();
    lane.queue(outbound_message(41, 401, "duplicate frame", 30), ts(30));
    pair.a.wire.duplicate_next_data();
    lane.run(ts(30));

    assert_eq!(pair.a.store.outbox_state(MessageId::from_u128(41)), Some(OutboxState::Completed));
    assert_eq!(
        pair.b.store.inbound(MessageId::from_u128(41)).expect("deduplicated inbound").body().as_str(),
        "duplicate frame"
    );
}

#[test]
fn receipt_reaction_and_attachment_use_the_same_peer_wire() {
    let pair = TestPair::new();
    let receipt = ApplicationPayload::Receipt(ReceiptPayload {
        receipt_id: OpaqueId::from_u128(51),
        message_id: OpaqueId::from_u128(52),
        contact_id: OpaqueId::from_u128(2),
        kind: DeliveryReceiptKind::Delivered,
        at: ts(40),
    });
    let reaction = ApplicationPayload::Reaction(ReactionPayload {
        reaction_id: OpaqueId::from_u128(61),
        message_id: OpaqueId::from_u128(52),
        conversation_id: OpaqueId::from_u128(62),
        actor_id: OpaqueId::from_u128(1),
        emoji: "👍".into(),
        active: true,
        at: ts(41),
    });
    assert_eq!(
        pair.a.send_payload(
            OpaqueId::from_u128(51),
            RECEIPT_MESSAGE_KIND,
            ApplicationPayloadCodec::encode(&receipt).expect("receipt encode"),
        ),
        DeliveryAck::Accepted
    );
    assert_eq!(
        pair.a.send_payload(
            OpaqueId::from_u128(61),
            REACTION_MESSAGE_KIND,
            ApplicationPayloadCodec::encode(&reaction).expect("reaction encode"),
        ),
        DeliveryAck::Accepted
    );

    let metadata = AttachmentFrame::Metadata(AttachmentMetadataFrame {
        attachment_id: AttachmentId::from_u128(71),
        message_id: OpaqueId::from_u128(52),
        name: AttachmentName::new("note.txt").expect("name"),
        media_type: MediaType::new("text/plain").expect("media"),
        size: 3,
        digest: [7; 32],
        preview: None,
    });
    let chunk = AttachmentFrame::Chunk(AttachmentChunkFrame {
        attachment_id: AttachmentId::from_u128(71),
        offset: 0,
        bytes: b"abc".to_vec(),
    });
    for (id, frame) in [(OpaqueId::from_u128(72), metadata.clone()), (OpaqueId::from_u128(73), chunk.clone())] {
        assert_eq!(
            pair.a.send_payload(
                id,
                ATTACHMENT_MESSAGE_KIND,
                AttachmentCodec::encode(&frame).expect("attachment encode"),
            ),
            DeliveryAck::Accepted
        );
    }

    assert_eq!(pair.b.controls(), vec![receipt, reaction]);
    assert_eq!(pair.b.attachments(), vec![metadata, chunk]);
}
