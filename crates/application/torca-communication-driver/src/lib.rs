//! One communication supervisor over the process-owned authenticated peer link.

use core::fmt;
use std::collections::{BTreeMap, VecDeque};
use std::path::PathBuf;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread;

mod attachment_scheduler;
use attachment_scheduler::AttachmentJobScheduler;
use std::time::Duration;

use torca_attachments::AttachmentId;
use torca_client_engine::EngineHandle;
use torca_contacts::ContactId;
use torca_control_delivery::{ControlKind, PendingControlJob, ReadCandidate};
use torca_conversations::ConversationId;
use torca_delivery::{
    ApplicationPayload, ApplicationPayloadCodec, DeliveryReceiptKind, ReactionPayload,
    ReceiptPayload,
};
use torca_foundation::{
    ClassifiedError, ErrorCategory, ErrorCode, ErrorDescriptor, OpaqueId, RetryAdvice, Timestamp,
};
use torca_messaging::Message;
use torca_receipts::{ReceiptId, ReceiptKind};
pub use torca_runtime::PeerConnectionStatus;
use torca_runtime::{
    AttachmentExportPort, AttachmentSendRequest, AttachmentTransferPort, AttachmentView,
    ContactVerificationSnapshot, ConversationReadPort, PeerSessionPort, RelationshipAdminPort,
    RuntimeDriverError,
};
pub use torca_runtime::{PeerHealthQuality, PeerHealthSnapshot};

pub const TEXT_MESSAGE_KIND: u16 = 1;
pub const RECEIPT_MESSAGE_KIND: u16 = 2;
pub const REACTION_MESSAGE_KIND: u16 = 3;
pub const ATTACHMENT_MESSAGE_KIND: u16 = 3;
pub const PROBE_MESSAGE_KIND: u16 = 4;
pub const RADIO_CONTROL_MESSAGE_KIND: u16 = 5;
const INBOUND_BATCH: usize = 64;
const TEXT_BATCH: usize = 16;
const CONTROL_BATCH: usize = 16;
// One durable chunk per worker turn keeps progress monotonic and preserves
// fair access to the shared peer transport.
const ATTACHMENT_BATCH: usize = 1;
const MAX_DEFERRED_ATTACHMENTS: usize = 64;

/// Provider-neutral inbound envelope owned by the application boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InboundEnvelope {
    pub contact_id: ContactId,
    pub envelope_id: OpaqueId,
    pub message_kind: u16,
    pub ciphertext: Vec<u8>,
}

pub fn classify_peer_health(
    rtt_ms: Option<u64>,
    consecutive_failures: u32,
    sample_age: Option<Duration>,
) -> PeerHealthQuality {
    match torca_presence::classify_health(rtt_ms, consecutive_failures, sample_age) {
        torca_presence::PresenceQuality::Unknown => PeerHealthQuality::Unknown,
        torca_presence::PresenceQuality::Excellent => PeerHealthQuality::Excellent,
        torca_presence::PresenceQuality::Good => PeerHealthQuality::Good,
        torca_presence::PresenceQuality::Fair => PeerHealthQuality::Fair,
        torca_presence::PresenceQuality::Poor => PeerHealthQuality::Poor,
    }
}

/// Application policy that turns read candidates into durable control jobs.
/// Storage receives the resulting jobs and never decides whether a receipt is required.
pub fn plan_read_receipts(
    candidates: &[ReadCandidate],
    at: Timestamp,
) -> Result<Vec<PendingControlJob>, CommunicationError> {
    candidates
        .iter()
        .map(|candidate| {
            let message_id = torca_messaging::MessageId::from_opaque(candidate.message_id);
            let receipt_id =
                ReceiptId::deterministic_for(message_id, ReceiptKind::Read).to_opaque();
            let payload =
                ApplicationPayloadCodec::encode(&ApplicationPayload::Receipt(ReceiptPayload {
                    receipt_id,
                    message_id: candidate.message_id,
                    contact_id: candidate.contact_id,
                    kind: DeliveryReceiptKind::Read,
                    at,
                }))
                .map_err(|_| CommunicationError::ReadState)?;
            Ok(PendingControlJob {
                job_id: receipt_id,
                contact_id: candidate.contact_id,
                message_id: Some(candidate.message_id),
                kind: ControlKind::Receipt,
                payload,
                next_attempt_at: at,
            })
        })
        .collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommunicationError {
    Peer,
    Text,
    Control,
    Inbound,
    Attachment,
    AttachmentStage(AttachmentFailureStage),
    ReadState,
    Relationship,
    Engine,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttachmentFailureStage {
    AckTimeout,
    PeerUnavailable,
    Integrity,
    Storage,
    Dependency,
    Protocol,
    Unknown,
}
impl fmt::Display for CommunicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for CommunicationError {}

impl ClassifiedError for CommunicationError {
    fn descriptor(&self) -> ErrorDescriptor {
        let (code, category, retry) = match self {
            Self::Peer => {
                ("communication.peer_unavailable", ErrorCategory::Unavailable, RetryAdvice::Backoff)
            }
            Self::Text => {
                ("communication.text_failed", ErrorCategory::Unavailable, RetryAdvice::Backoff)
            }
            Self::Control => {
                ("communication.control_failed", ErrorCategory::Unavailable, RetryAdvice::Backoff)
            }
            Self::Inbound => {
                ("communication.inbound_invalid", ErrorCategory::InvalidInput, RetryAdvice::Never)
            }
            Self::Attachment => {
                // Attachment delivery is durable and has its own persisted
                // retry schedule.  Treat a failed transfer attempt as a
                // recoverable availability issue at the application boundary;
                // a permanent validation error is rejected before queueing.
                (
                    "communication.attachment_unavailable",
                    ErrorCategory::Unavailable,
                    RetryAdvice::Backoff,
                )
            }
            Self::AttachmentStage(stage) => match stage {
                AttachmentFailureStage::AckTimeout => (
                    "communication.attachment_ack_timeout",
                    ErrorCategory::Unavailable,
                    RetryAdvice::Backoff,
                ),
                AttachmentFailureStage::PeerUnavailable => (
                    "communication.attachment_peer_unavailable",
                    ErrorCategory::Unavailable,
                    RetryAdvice::Backoff,
                ),
                AttachmentFailureStage::Integrity => (
                    "communication.attachment_integrity_failed",
                    ErrorCategory::Conflict,
                    RetryAdvice::Never,
                ),
                AttachmentFailureStage::Storage => (
                    "communication.attachment_storage_failed",
                    ErrorCategory::Internal,
                    RetryAdvice::Never,
                ),
                AttachmentFailureStage::Dependency => (
                    "communication.attachment_dependency_missing",
                    ErrorCategory::Unavailable,
                    RetryAdvice::Backoff,
                ),
                AttachmentFailureStage::Protocol | AttachmentFailureStage::Unknown => (
                    "communication.attachment_protocol_failed",
                    ErrorCategory::InvalidInput,
                    RetryAdvice::Never,
                ),
            },
            Self::ReadState => {
                ("communication.read_state_failed", ErrorCategory::Internal, RetryAdvice::Never)
            }
            Self::Relationship => {
                ("communication.relationship_failed", ErrorCategory::Conflict, RetryAdvice::Never)
            }
            Self::Engine => {
                ("communication.engine_failed", ErrorCategory::Internal, RetryAdvice::Never)
            }
        };
        ErrorDescriptor::new(ErrorCode::new(code), category, retry)
    }
}

pub trait PeerLinkRuntime: Send {
    fn maintenance(
        &mut self,
        contacts: &[ContactId],
        now: Timestamp,
    ) -> Result<(), CommunicationError>;
    fn network_changed(&mut self, _now: Timestamp) {}
    fn set_waker(&mut self, _waker: Arc<dyn Fn() + Send + Sync>) {}
    fn connection_state(&self, contact_id: ContactId) -> PeerConnectionStatus;
    fn take_inbound(&mut self) -> Result<Option<InboundEnvelope>, CommunicationError>;
    fn reject(&mut self, envelope: &InboundEnvelope) -> Result<(), CommunicationError>;
    fn shutdown(&mut self);
    fn peer_health(&self, contact_id: ContactId) -> PeerHealthSnapshot {
        PeerHealthSnapshot::from_connection_state(self.connection_state(contact_id))
    }
    fn peer_probe_eligible(&self, _contact_id: ContactId) -> bool {
        true
    }
    fn begin_probe(
        &mut self,
        _contact_id: ContactId,
        _probe_id: OpaqueId,
        _reported_rtt_ms: u64,
    ) -> Result<(), CommunicationError> {
        Ok(())
    }
    fn take_probe_completion(
        &mut self,
        _now: Timestamp,
    ) -> Result<Option<ContactId>, CommunicationError> {
        Ok(None)
    }
    fn accept_probe(
        &mut self,
        _envelope: &InboundEnvelope,
        _now: Timestamp,
    ) -> Result<(), CommunicationError> {
        Err(CommunicationError::Peer)
    }
}

pub trait TextDeliveryRuntime: Send {
    fn recover(&mut self, now: Timestamp) -> Result<(), CommunicationError>;
    fn maintenance(&mut self, now: Timestamp, limit: usize) -> Result<(), CommunicationError>;
    fn next_maintenance_delay(&self, _now: Timestamp) -> Option<Duration> {
        None
    }

    /// Cumulative durable write operations performed by this worker. The
    /// runtime samples the value and records only the delta in diagnostics.
    fn database_write_count(&self) -> u64 {
        0
    }
}
pub trait ControlDeliveryRuntime: Send {
    fn recover(&mut self, now: Timestamp) -> Result<(), CommunicationError>;
    fn maintenance(&mut self, now: Timestamp, limit: usize) -> Result<(), CommunicationError>;
    fn next_maintenance_delay(&self, _now: Timestamp) -> Option<Duration> {
        None
    }
    fn database_write_count(&self) -> u64 {
        0
    }
    fn queue_reaction(
        &mut self,
        _contact_id: ContactId,
        _reaction: ReactionPayload,
        _at: Timestamp,
    ) -> Result<(), CommunicationError> {
        Err(CommunicationError::Control)
    }
}
pub trait InboundMessagingRuntime: Send {
    fn process(
        &mut self,
        envelope: InboundEnvelope,
        now: Timestamp,
    ) -> Result<(), CommunicationError>;
}
pub trait AttachmentRuntime: Send {
    fn prepare_outgoing(
        &mut self,
        request: &AttachmentSendRequest,
        now: Timestamp,
    ) -> Result<(), CommunicationError>;
    fn retry(&mut self, attachment_id: OpaqueId, now: Timestamp) -> Result<(), CommunicationError>;
    fn cancel(&mut self, attachment_id: OpaqueId, now: Timestamp)
    -> Result<(), CommunicationError>;
    fn snapshot(&self, messages: &[Message]) -> Result<Vec<AttachmentView>, CommunicationError>;
    /// Production adapters can provide a storage-owned projection without loading message history.
    /// Legacy/test adapters may return `None` and retain the original fallback contract.
    fn snapshot_projection(&self) -> Result<Option<Vec<AttachmentView>>, CommunicationError> {
        Ok(None)
    }
    /// Cumulative attachment metadata writes known by the adapter.
    fn database_write_count(&self) -> u64 {
        0
    }
    fn blob_write_count(&self) -> u64 {
        0
    }
    fn process_inbound(
        &mut self,
        envelope: InboundEnvelope,
        now: Timestamp,
    ) -> Result<(), CommunicationError>;
    fn maintenance_outgoing(
        &mut self,
        messages: &[Message],
        now: Timestamp,
        limit: usize,
    ) -> Result<(), CommunicationError>;
    fn shutdown(&mut self);
}
pub trait AttachmentExportRuntime: Send {
    fn export_attachment(
        &mut self,
        attachment_id: AttachmentId,
        destination: PathBuf,
    ) -> Result<(), CommunicationError>;
    fn export_attachment_preview(
        &mut self,
        attachment_id: AttachmentId,
        destination: PathBuf,
    ) -> Result<(), CommunicationError>;
}
pub trait ReadStateRuntime: Send {
    fn mark_conversation_read(
        &mut self,
        conversation_id: OpaqueId,
        now: Timestamp,
    ) -> Result<(), CommunicationError>;
}
pub trait RelationshipAdminRuntime: Send {
    fn contact_names(&self) -> Result<BTreeMap<ContactId, String>, CommunicationError>;
    fn contact_verifications(
        &self,
    ) -> Result<BTreeMap<ContactId, ContactVerificationSnapshot>, CommunicationError>;
    fn verify_contact(
        &mut self,
        contact_id: ContactId,
        now: Timestamp,
    ) -> Result<(), CommunicationError>;
    fn reset_contact_verification(
        &mut self,
        contact_id: ContactId,
    ) -> Result<(), CommunicationError>;
    fn rename_contact(
        &mut self,
        contact_id: ContactId,
        display_name: String,
        now: Timestamp,
    ) -> Result<(), CommunicationError>;
    fn block_contact(
        &mut self,
        contact_id: ContactId,
        now: Timestamp,
    ) -> Result<(), CommunicationError>;
    fn unblock_contact(
        &mut self,
        contact_id: ContactId,
        now: Timestamp,
    ) -> Result<(), CommunicationError>;
    fn clear_history(&mut self, conversation_id: ConversationId) -> Result<(), CommunicationError>;
    fn remove_contact(&mut self, contact_id: ContactId) -> Result<(), CommunicationError>;
}

/// Optional Radio Mode ingress/maintenance boundary. The communication
/// supervisor only owns authenticated envelope routing; product state stays
/// in the dedicated application coordinator.
pub trait RadioInboundRuntime: Send {
    fn process_control(
        &mut self,
        envelope: InboundEnvelope,
        now: Timestamp,
    ) -> Result<(), CommunicationError>;

    fn maintenance(&mut self, now: Timestamp) -> Result<(), CommunicationError>;

    /// Returns a deadline only while radio control or state-sync work exists.
    fn next_maintenance_delay(&self, _now: Timestamp) -> Option<Duration> {
        None
    }

    fn shutdown(&mut self) {}
}

pub struct TorcaCommunicationDriver {
    engine: EngineHandle,
    peer: Box<dyn PeerLinkRuntime>,
    text: Box<dyn TextDeliveryRuntime>,
    control: Box<dyn ControlDeliveryRuntime>,
    inbound: Box<dyn InboundMessagingRuntime>,
    attachments: Arc<Mutex<Box<dyn AttachmentRuntime>>>,
    attachment_job_active: Arc<AtomicBool>,
    attachment_snapshot_cache: Arc<Mutex<Vec<AttachmentView>>>,
    deferred_attachments: VecDeque<InboundEnvelope>,
    attachment_export: Box<dyn AttachmentExportRuntime>,
    read_state: Box<dyn ReadStateRuntime>,
    relationships: Box<dyn RelationshipAdminRuntime>,
    radio: Option<Box<dyn RadioInboundRuntime>>,
    attachment_scheduler: AttachmentJobScheduler,
}
impl TorcaCommunicationDriver {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        engine: EngineHandle,
        peer: Box<dyn PeerLinkRuntime>,
        text: Box<dyn TextDeliveryRuntime>,
        control: Box<dyn ControlDeliveryRuntime>,
        inbound: Box<dyn InboundMessagingRuntime>,
        attachments: Box<dyn AttachmentRuntime>,
        attachment_export: Box<dyn AttachmentExportRuntime>,
        read_state: Box<dyn ReadStateRuntime>,
        relationships: Box<dyn RelationshipAdminRuntime>,
    ) -> Self {
        Self {
            engine,
            peer,
            text,
            control,
            inbound,
            attachments: Arc::new(Mutex::new(attachments)),
            attachment_job_active: Arc::new(AtomicBool::new(false)),
            attachment_snapshot_cache: Arc::new(Mutex::new(Vec::new())),
            deferred_attachments: VecDeque::new(),
            attachment_export,
            read_state,
            relationships,
            radio: None,
            attachment_scheduler: AttachmentJobScheduler::new(),
        }
    }

    pub fn with_radio(mut self, radio: Box<dyn RadioInboundRuntime>) -> Self {
        self.radio = Some(radio);
        self
    }

    /// Connects infrastructure listener events to the single runtime owner.
    pub fn set_waker(&mut self, waker: Arc<dyn Fn() + Send + Sync>) {
        self.peer.set_waker(waker);
    }

    fn attachment_runtime(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, Box<dyn AttachmentRuntime>>, CommunicationError> {
        self.attachments.lock().map_err(|_| CommunicationError::Attachment)
    }

    pub fn peer_health(&self, contact_id: ContactId) -> PeerHealthSnapshot {
        self.peer.peer_health(contact_id)
    }

    /// Defers an attachment frame without letting it block text/control ingress.
    ///
    /// An attachment metadata frame is allowed to arrive before its companion
    /// text message.  In that case the attachment runtime deliberately returns
    /// a dependency error because it cannot safely bind the file to a
    /// conversation yet.  Treating that as a communication-tick failure used
    /// to starve the following text frames indefinitely.
    fn defer_attachment(&mut self, envelope: InboundEnvelope) {
        // A peer can retransmit while a metadata envelope is already waiting
        // for its companion message. Keep exactly one copy; otherwise a
        // short ordering race can fill the bounded queue with the same frame
        // and delay unrelated attachments.
        if self.deferred_attachments.iter().any(|queued| {
            queued.contact_id == envelope.contact_id && queued.envelope_id == envelope.envelope_id
        }) {
            return;
        }
        if self.deferred_attachments.len() >= MAX_DEFERRED_ATTACHMENTS {
            eprintln!(
                "torca-attachment: deferred inbound queue full; awaiting peer retransmission contact={} envelope={}",
                envelope.contact_id, envelope.envelope_id
            );
            return;
        }
        self.deferred_attachments.push_back(envelope);
    }

    /// Processes one inbound attachment independently from the main inbound
    /// stream.  A failed attachment is a durable transfer concern, not a
    /// reason to stop message, receipt, probe or pairing processing.
    fn process_attachment_inbound(
        &mut self,
        envelope: InboundEnvelope,
        now: Timestamp,
    ) -> Result<(), CommunicationError> {
        let contact_id = envelope.contact_id;
        let envelope_id = envelope.envelope_id;
        // Keep the lock-result and its potential guard in a small scope.  A
        // `TryLockError` can itself retain a guard, so mutating the deferred
        // queue inside the match would overlap the immutable mutex borrow.
        let should_defer = {
            match self.attachments.try_lock() {
                Ok(mut attachments) => match attachments.process_inbound(envelope.clone(), now) {
                    Ok(()) => {
                        refresh_attachment_cache(&self.attachment_snapshot_cache, &**attachments);
                        false
                    }
                    Err(CommunicationError::AttachmentStage(
                        AttachmentFailureStage::Dependency,
                    )) => true,
                    Err(error) => {
                        // Do not ACK malformed/temporary failures.  The peer-side
                        // durable sender will retry after its own ACK deadline.
                        // Crucially, leave the rest of the inbound batch running.
                        eprintln!(
                            "torca-attachment: inbound failed contact={} envelope={} code={error}",
                            contact_id, envelope_id
                        );
                        false
                    }
                },
                Err(std::sync::TryLockError::WouldBlock) => true,
                Err(std::sync::TryLockError::Poisoned(_)) => {
                    return Err(CommunicationError::Attachment);
                }
            }
        };
        if should_defer {
            // The text frame may still be waiting in the peer inbox and will
            // be consumed in this same maintenance turn.
            self.defer_attachment(envelope);
        }
        Ok(())
    }

    fn drain_inbound(&mut self, now: Timestamp) -> Result<(), CommunicationError> {
        for _ in 0..INBOUND_BATCH {
            let Some(envelope) = self.peer.take_inbound()? else { break };
            match envelope.message_kind {
                TEXT_MESSAGE_KIND | RECEIPT_MESSAGE_KIND => self.inbound.process(envelope, now)?,
                ATTACHMENT_MESSAGE_KIND => self.process_attachment_inbound(envelope, now)?,
                PROBE_MESSAGE_KIND => self.peer.accept_probe(&envelope, now)?,
                RADIO_CONTROL_MESSAGE_KIND => {
                    if let Some(radio) = self.radio.as_mut() {
                        radio.process_control(envelope, now)?;
                    } else {
                        self.peer.reject(&envelope)?;
                    }
                }
                _ => self.peer.reject(&envelope)?,
            }
        }

        // Run deferred attachment frames only after text/control frames from
        // this batch.  This resolves the normal "attachment metadata arrived
        // first" race without delaying messages behind a file transfer.
        let deferred = self.deferred_attachments.len().min(INBOUND_BATCH);
        for _ in 0..deferred {
            let Some(envelope) = self.deferred_attachments.pop_front() else {
                break;
            };
            self.process_attachment_inbound(envelope, now)?;
        }
        Ok(())
    }
}

impl PeerSessionPort for TorcaCommunicationDriver {
    fn recover(&mut self, now: Timestamp) -> Result<(), RuntimeDriverError> {
        self.text.recover(now).map_err(map_runtime)?;
        self.control.recover(now).map_err(map_runtime)?;
        Ok(())
    }

    fn maintenance(
        &mut self,
        contacts: &[ContactId],
        now: Timestamp,
    ) -> Result<(), RuntimeDriverError> {
        self.peer.maintenance(contacts, now).map_err(map_runtime)?;
        self.drain_inbound(now).map_err(map_runtime)?;
        self.text.maintenance(now, TEXT_BATCH).map_err(map_runtime)?;
        self.control.maintenance(now, CONTROL_BATCH).map_err(map_runtime)?;
        if let Some(radio) = self.radio.as_mut()
            && let Err(error) = radio.maintenance(now)
        {
            eprintln!("torca-radio: background maintenance failed code={error}");
        }
        // Attachment delivery is a durable, independently retryable job.  A
        // single peer/ACK failure must not abort the communication tick and
        // starve text/control delivery.  The adapter persists Failed and its
        // next-attempt time before returning the error.
        if self.attachment_scheduler.due(now) {
            if !self.attachment_job_active.load(Ordering::Acquire) {
                let snapshot = self.engine.snapshot().map_err(|_| RuntimeDriverError::Engine)?;
                if !self.attachment_job_active.swap(true, Ordering::AcqRel) {
                    let attachments = Arc::clone(&self.attachments);
                    let active = Arc::clone(&self.attachment_job_active);
                    let projection_cache = Arc::clone(&self.attachment_snapshot_cache);
                    let messages = snapshot.messages.clone();
                    thread::spawn(move || {
                        let result = attachments
                            .lock()
                            .map_err(|_| CommunicationError::Attachment)
                            .and_then(|mut runtime| {
                                runtime.maintenance_outgoing(&messages, now, ATTACHMENT_BATCH)?;
                                refresh_attachment_cache(&projection_cache, &**runtime);
                                Ok(())
                            });
                        if let Err(error) = result {
                            eprintln!("torca-attachment: maintenance failed code={error}");
                        }
                        active.store(false, Ordering::Release);
                    });
                }
            }
            self.attachment_scheduler.record_attempt(now);
        }
        Ok(())
    }

    fn next_maintenance_delay(&self, now: Timestamp) -> Option<Duration> {
        [
            self.text.next_maintenance_delay(now),
            self.control.next_maintenance_delay(now),
            self.attachment_scheduler.next_delay(now),
            self.radio.as_ref().and_then(|radio| radio.next_maintenance_delay(now)),
        ]
        .into_iter()
        .flatten()
        .min()
    }

    fn network_changed(&mut self, now: Timestamp) {
        self.peer.network_changed(now);
    }

    fn set_waker(&mut self, waker: Arc<dyn Fn() + Send + Sync>) {
        self.peer.set_waker(waker);
    }

    fn connection_state(&self, contact_id: ContactId) -> PeerConnectionStatus {
        self.peer.connection_state(contact_id)
    }
    fn peer_health(&self, contact_id: ContactId) -> PeerHealthSnapshot {
        self.peer.peer_health(contact_id)
    }
    fn peer_probe_eligible(&self, contact_id: ContactId) -> bool {
        self.peer.peer_probe_eligible(contact_id)
    }
    fn begin_peer_probe(
        &mut self,
        contact_id: ContactId,
        probe_id: OpaqueId,
        reported_rtt_ms: u64,
    ) -> Result<(), RuntimeDriverError> {
        self.peer.begin_probe(contact_id, probe_id, reported_rtt_ms).map_err(map_runtime)
    }
    fn take_peer_probe_completion(
        &mut self,
        now: Timestamp,
    ) -> Result<Option<ContactId>, RuntimeDriverError> {
        self.peer.take_probe_completion(now).map_err(map_runtime)
    }

    fn shutdown(&mut self) {
        if let Some(radio) = self.radio.as_mut() {
            radio.shutdown();
        }
        if let Ok(mut attachments) = self.attachments.lock() {
            attachments.shutdown();
        }
        self.peer.shutdown();
    }
}

impl torca_runtime::CommunicationDriver for TorcaCommunicationDriver {
    fn database_write_count(&self) -> u64 {
        let attachment_writes = self
            .attachments
            .lock()
            .map(|attachments| attachments.database_write_count())
            .unwrap_or(0);
        self.text.database_write_count() + self.control.database_write_count() + attachment_writes
    }

    fn blob_write_count(&self) -> u64 {
        self.attachments.lock().map(|attachments| attachments.blob_write_count()).unwrap_or(0)
    }

    fn queue_reaction(
        &mut self,
        contact_id: ContactId,
        reaction: ReactionPayload,
        at: Timestamp,
    ) -> Result<(), RuntimeDriverError> {
        self.control.queue_reaction(contact_id, reaction, at).map_err(map_runtime)
    }
}

impl RelationshipAdminPort for TorcaCommunicationDriver {
    fn contact_names(&self) -> Result<BTreeMap<ContactId, String>, RuntimeDriverError> {
        self.relationships.contact_names().map_err(map_runtime)
    }
    fn contact_verifications(
        &self,
    ) -> Result<BTreeMap<ContactId, ContactVerificationSnapshot>, RuntimeDriverError> {
        self.relationships.contact_verifications().map_err(map_runtime)
    }
    fn verify_contact(&mut self, id: ContactId, now: Timestamp) -> Result<(), RuntimeDriverError> {
        self.relationships.verify_contact(id, now).map_err(map_runtime)
    }
    fn reset_contact_verification(&mut self, id: ContactId) -> Result<(), RuntimeDriverError> {
        self.relationships.reset_contact_verification(id).map_err(map_runtime)
    }
    fn rename_contact(
        &mut self,
        id: ContactId,
        name: String,
        now: Timestamp,
    ) -> Result<(), RuntimeDriverError> {
        self.relationships.rename_contact(id, name, now).map_err(map_runtime)
    }
    fn block_contact(&mut self, id: ContactId, now: Timestamp) -> Result<(), RuntimeDriverError> {
        self.relationships.block_contact(id, now).map_err(map_runtime)?;
        self.peer.shutdown();
        Ok(())
    }
    fn unblock_contact(&mut self, id: ContactId, now: Timestamp) -> Result<(), RuntimeDriverError> {
        self.relationships.unblock_contact(id, now).map_err(map_runtime)
    }
    fn clear_conversation_history(&mut self, id: ConversationId) -> Result<(), RuntimeDriverError> {
        self.relationships.clear_history(id).map_err(map_runtime)
    }
    fn remove_contact(&mut self, id: ContactId) -> Result<(), RuntimeDriverError> {
        self.relationships.remove_contact(id).map_err(map_runtime)?;
        self.peer.shutdown();
        Ok(())
    }
}

impl ConversationReadPort for TorcaCommunicationDriver {
    fn mark_conversation_read(
        &mut self,
        id: OpaqueId,
        now: Timestamp,
    ) -> Result<(), RuntimeDriverError> {
        self.read_state.mark_conversation_read(id, now).map_err(map_runtime)
    }
}

impl AttachmentTransferPort for TorcaCommunicationDriver {
    fn prepare_attachment(
        &mut self,
        request: &AttachmentSendRequest,
        now: Timestamp,
    ) -> Result<(), RuntimeDriverError> {
        let projection_cache = Arc::clone(&self.attachment_snapshot_cache);
        let result = {
            let mut attachments = self.attachment_runtime().map_err(map_runtime)?;
            let result = attachments.prepare_outgoing(request, now).map_err(map_runtime);
            if result.is_ok() {
                refresh_attachment_cache(&projection_cache, &**attachments);
            }
            result
        };
        if result.is_ok() {
            self.attachment_scheduler.wake();
        }
        result
    }
    fn retry_attachment(&mut self, id: OpaqueId, now: Timestamp) -> Result<(), RuntimeDriverError> {
        let projection_cache = Arc::clone(&self.attachment_snapshot_cache);
        let result = {
            let mut attachments = self.attachment_runtime().map_err(map_runtime)?;
            let result = attachments.retry(id, now).map_err(map_runtime);
            if result.is_ok() {
                refresh_attachment_cache(&projection_cache, &**attachments);
            }
            result
        };
        if result.is_ok() {
            self.attachment_scheduler.wake();
        }
        result
    }
    fn cancel_attachment(
        &mut self,
        id: OpaqueId,
        now: Timestamp,
    ) -> Result<(), RuntimeDriverError> {
        let projection_cache = Arc::clone(&self.attachment_snapshot_cache);
        let mut attachments = self.attachment_runtime().map_err(map_runtime)?;
        attachments.cancel(id, now).map_err(map_runtime)?;
        refresh_attachment_cache(&projection_cache, &**attachments);
        Ok(())
    }
    fn attachment_snapshot(&self) -> Result<Vec<AttachmentView>, RuntimeDriverError> {
        let Ok(attachments) = self.attachments.try_lock() else {
            return Ok(self
                .attachment_snapshot_cache
                .lock()
                .map(|snapshot| snapshot.clone())
                .unwrap_or_default());
        };
        let snapshot =
            if let Some(snapshot) = attachments.snapshot_projection().map_err(map_runtime)? {
                snapshot
            } else {
                let messages = self.engine.snapshot().map_err(|_| RuntimeDriverError::Engine)?;
                attachments.snapshot(&messages.messages).map_err(map_runtime)?
            };
        if let Ok(mut cache) = self.attachment_snapshot_cache.lock() {
            *cache = snapshot.clone();
        }
        Ok(snapshot)
    }
}

impl AttachmentExportPort for TorcaCommunicationDriver {
    fn export_attachment(
        &mut self,
        id: AttachmentId,
        destination: PathBuf,
    ) -> Result<(), RuntimeDriverError> {
        self.attachment_export.export_attachment(id, destination).map_err(map_runtime)
    }
    fn export_attachment_preview(
        &mut self,
        id: AttachmentId,
        destination: PathBuf,
    ) -> Result<(), RuntimeDriverError> {
        self.attachment_export.export_attachment_preview(id, destination).map_err(map_runtime)
    }
}

fn refresh_attachment_cache(cache: &Mutex<Vec<AttachmentView>>, runtime: &dyn AttachmentRuntime) {
    let Ok(Some(snapshot)) = runtime.snapshot_projection() else {
        return;
    };
    if let Ok(mut cached) = cache.lock() {
        *cached = snapshot;
    }
}

fn map_runtime(error: CommunicationError) -> RuntimeDriverError {
    RuntimeDriverError::Classified(error.descriptor())
}

#[cfg(test)]
mod tests {
    use super::{CommunicationError, map_runtime, plan_read_receipts};
    use torca_control_delivery::ReadCandidate;
    use torca_foundation::{ClassifiedError, OpaqueId, Timestamp};

    #[test]
    fn read_receipt_planner_creates_one_idempotent_job_per_candidate() {
        let jobs = plan_read_receipts(
            &[ReadCandidate {
                contact_id: OpaqueId::from_u128(1),
                message_id: OpaqueId::from_u128(2),
            }],
            Timestamp::from_unix_millis(10).expect("timestamp"),
        )
        .expect("planner");
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].message_id, Some(OpaqueId::from_u128(2)));
        assert!(!jobs[0].payload.is_empty());
    }

    #[test]
    fn runtime_error_preserves_communication_descriptor() {
        let error = map_runtime(CommunicationError::Peer);
        assert_eq!(error.descriptor().code().as_str(), "communication.peer_unavailable");
    }

    #[test]
    fn attachment_delivery_failure_is_retryable() {
        let error = map_runtime(CommunicationError::Attachment);
        let descriptor = error.descriptor();
        assert_eq!(descriptor.code().as_str(), "communication.attachment_unavailable");
        assert_eq!(descriptor.retry_advice(), torca_foundation::RetryAdvice::Backoff);
    }
}
