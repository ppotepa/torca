//! One communication supervisor over the process-owned authenticated peer link.

mod attachment_scheduler;
mod error;
mod policy;
mod ports;

use std::collections::{BTreeMap, VecDeque};
use std::path::PathBuf;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc::{Receiver, SyncSender, TrySendError, sync_channel},
};
use std::thread;
use std::time::Duration;

use attachment_scheduler::AttachmentJobScheduler;
pub use error::{AttachmentFailureStage, CommunicationError};
pub use policy::{
    ATTACHMENT_MESSAGE_KIND, PROBE_MESSAGE_KIND, RADIO_CONTROL_MESSAGE_KIND, REACTION_MESSAGE_KIND,
    RECEIPT_MESSAGE_KIND, TEXT_MESSAGE_KIND, classify_peer_health, plan_read_receipts,
};
pub use ports::{
    AttachmentExportRuntime, AttachmentMaintenanceResult, AttachmentRuntime,
    ControlDeliveryRuntime, InboundEnvelope, InboundMessagingRuntime, PeerLinkRuntime,
    RadioInboundRuntime, ReadStateRuntime, RelationshipAdminRuntime, TextDeliveryRuntime,
};

use torca_attachments::AttachmentId;
use torca_client_engine::EngineHandle;
use torca_contacts::ContactId;
use torca_conversations::ConversationId;
use torca_delivery::ReactionPayload;
use torca_foundation::{ClassifiedError, CommandId, OpaqueId, Timestamp};
use torca_messaging::Message;
use torca_runtime::{
    AttachmentExportPort, AttachmentSendRequest, AttachmentTransferPort, AttachmentView,
    ContactVerificationSnapshot, ConversationReadPort, PeerSessionPort, RelationshipAdminPort,
    RuntimeDriverError,
};
pub use torca_runtime::{PeerActivityEvidence, PeerConnectionStatus};
pub use torca_runtime::{PeerHealthQuality, PeerHealthSnapshot};
use torca_runtime_policy::{BatteryProfile, MeteredTransferPolicy};

type WakeCallback = Arc<dyn Fn() + Send + Sync>;
type WakeSlot = Arc<Mutex<Option<WakeCallback>>>;

const INBOUND_BATCH: usize = 64;
// Delivery transports are durable and may wait for a peer ACK. Keep each
// actor maintenance turn bounded; retries remain queued for later turns.
const TEXT_BATCH: usize = 1;
const CONTROL_BATCH: usize = 1;
// Process a bounded batch per worker turn. This keeps transport fairness while
// avoiding one OS thread per 64 KiB chunk for large attachments.
const ATTACHMENT_BATCH: usize = 8;
const MAX_DEFERRED_ATTACHMENTS: usize = 64;

enum AttachmentWork {
    Maintenance { now: Timestamp },
    Prepare { request: AttachmentSendRequest, now: Timestamp },
}

pub struct TorcaCommunicationDriver {
    engine: EngineHandle,
    peer: Box<dyn PeerLinkRuntime>,
    text: Box<dyn TextDeliveryRuntime>,
    control: Box<dyn ControlDeliveryRuntime>,
    inbound: Box<dyn InboundMessagingRuntime>,
    attachments: Arc<Mutex<Box<dyn AttachmentRuntime>>>,
    attachment_job_active: Arc<AtomicBool>,
    attachment_job_result: Arc<Mutex<Option<AttachmentMaintenanceResult>>>,
    attachment_job_error: Arc<Mutex<Option<CommunicationError>>>,
    attachment_waker: WakeSlot,
    attachment_snapshot_cache: Arc<Mutex<Vec<AttachmentView>>>,
    attachment_job_sender: SyncSender<AttachmentWork>,
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
        let attachments = Arc::new(Mutex::new(attachments));
        let attachment_job_result = Arc::new(Mutex::new(None));
        let attachment_job_error = Arc::new(Mutex::new(None));
        let attachment_job_active = Arc::new(AtomicBool::new(false));
        let attachment_waker = Arc::new(Mutex::new(None));
        let attachment_snapshot_cache = Arc::new(Mutex::new(Vec::new()));
        let (attachment_job_sender, attachment_job_receiver) = sync_channel(8);
        spawn_attachment_worker(
            Arc::clone(&attachments),
            Arc::clone(&attachment_job_active),
            Arc::clone(&attachment_job_result),
            Arc::clone(&attachment_job_error),
            Arc::clone(&attachment_waker),
            Arc::clone(&attachment_snapshot_cache),
            attachment_job_receiver,
        );
        let text = Box::new(TextDeliveryBridge::new(text));
        let control = Box::new(ControlDeliveryBridge::new(control));
        Self {
            engine,
            peer,
            text,
            control,
            inbound,
            attachments,
            attachment_job_active,
            attachment_job_result,
            attachment_job_error,
            attachment_waker,
            attachment_snapshot_cache,
            attachment_job_sender,
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
        self.peer.set_waker(Arc::clone(&waker));
        if let Ok(mut target) = self.attachment_waker.lock() {
            *target = Some(waker);
        }
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
    fn defer_attachment(&mut self, envelope: InboundEnvelope) {
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

    /// Processes one inbound attachment independently from the main inbound stream.
    fn process_attachment_inbound(
        &mut self,
        envelope: InboundEnvelope,
        now: Timestamp,
    ) -> Result<(), CommunicationError> {
        let contact_id = envelope.contact_id;
        let envelope_id = envelope.envelope_id;
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
            self.defer_attachment(envelope);
        }
        Ok(())
    }

    fn drain_inbound(&mut self, now: Timestamp) -> Result<(), CommunicationError> {
        for _ in 0..INBOUND_BATCH {
            let Some(envelope) = self.peer.take_inbound()? else {
                break;
            };
            match envelope.message_kind {
                TEXT_MESSAGE_KIND | RECEIPT_MESSAGE_KIND | REACTION_MESSAGE_KIND => {
                    self.inbound.process(envelope, now)?;
                }
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
        if let Ok(mut result) = self.attachment_job_result.lock()
            && let Some(result) = result.take()
        {
            if result.more_work && !result.policy_blocked {
                if let Some(delay) = result.retry_after_ms {
                    self.attachment_scheduler.wake_after(now, Duration::from_millis(delay));
                } else {
                    self.attachment_scheduler.wake();
                }
            } else {
                self.attachment_scheduler.disarm();
            }
        }
        if let Ok(mut error) = self.attachment_job_error.lock()
            && let Some(error) = error.take()
        {
            return Err(map_runtime(error));
        }
        self.peer.maintenance(contacts, now).map_err(map_runtime)?;
        self.drain_inbound(now).map_err(map_runtime)?;
        self.text.maintenance(now, TEXT_BATCH).map_err(map_runtime)?;
        self.control.maintenance(now, CONTROL_BATCH).map_err(map_runtime)?;
        if self.attachment_scheduler.due(now)
            && !self.attachment_job_active.swap(true, Ordering::AcqRel)
        {
            self.attachment_scheduler.disarm();
            if let Err(error) =
                self.attachment_job_sender.try_send(AttachmentWork::Maintenance { now })
                && matches!(error, TrySendError::Disconnected(_))
            {
                self.attachment_job_active.store(false, Ordering::Release);
            }
        }
        Ok(())
    }

    fn next_maintenance_delay(&self, now: Timestamp) -> Option<Duration> {
        [
            self.peer.next_maintenance_delay(now),
            self.text.next_maintenance_delay(now),
            self.control.next_maintenance_delay(now),
            self.attachment_scheduler.next_delay(now),
        ]
        .into_iter()
        .flatten()
        .min()
    }

    fn network_changed(&mut self, now: Timestamp) {
        self.peer.network_changed(now);
    }

    fn prime_connections(&mut self) {
        self.peer.prime_connections();
    }

    fn prime_contact(&mut self, contact_id: ContactId) {
        let _ = self.peer.prime_contact(contact_id);
    }

    fn set_waker(&mut self, waker: Arc<dyn Fn() + Send + Sync>) {
        if let Ok(mut target) = self.attachment_waker.lock() {
            *target = Some(Arc::clone(&waker));
        }
        self.peer.set_waker(Arc::clone(&waker));
        self.text.set_waker(Arc::clone(&waker));
        if let Some(radio) = self.radio.as_mut() {
            radio.set_waker(Arc::clone(&waker));
        }
        self.control.set_waker(waker);
    }

    fn connection_state(&self, contact_id: ContactId) -> PeerConnectionStatus {
        self.peer.connection_state(contact_id)
    }

    fn peer_health(&self, contact_id: ContactId) -> PeerHealthSnapshot {
        self.peer.peer_health(contact_id)
    }

    fn peer_activity(&self) -> Vec<PeerActivityEvidence> {
        self.peer.peer_activity()
    }

    fn close_idle_peers(
        &mut self,
        retained: &[ContactId],
        now: Timestamp,
    ) -> Result<usize, RuntimeDriverError> {
        self.peer.close_idle_peers(retained, now).map_err(map_runtime)
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
    fn queue_outbound(
        &mut self,
        message: Message,
        command_id: CommandId,
        next_attempt_at: Timestamp,
    ) -> Result<(), RuntimeDriverError> {
        self.text.queue_outbound(message, command_id, next_attempt_at).map_err(map_runtime)
    }

    fn wake_delivery(&mut self) {
        self.text.wake();
        self.control.wake();
    }

    fn maintain_radio(&mut self, now: Timestamp) -> Result<(), RuntimeDriverError> {
        if let Some(radio) = self.radio.as_mut()
            && let Err(error) = radio.maintenance(now)
        {
            eprintln!("torca-radio: maintenance failed code={error}");
        }
        Ok(())
    }

    fn next_radio_maintenance_delay(&self, now: Timestamp) -> Option<Duration> {
        self.radio.as_ref().and_then(|radio| radio.next_maintenance_delay(now))
    }

    fn database_write_count(&self) -> u64 {
        let attachment_writes = self
            .attachments
            .lock()
            .map(|attachments| attachments.database_write_count())
            .unwrap_or(0);
        self.text.database_write_count()
            + self.control.database_write_count()
            + self.inbound.database_write_count()
            + attachment_writes
    }

    fn blob_write_count(&self) -> u64 {
        self.attachments.lock().map(|attachments| attachments.blob_write_count()).unwrap_or(0)
    }

    fn attachment_chunk_tx_count(&self) -> u64 {
        self.attachments.lock().map(|attachments| attachments.chunk_tx_count()).unwrap_or(0)
    }

    fn attachment_policy_suppressed_count(&self) -> u64 {
        self.attachments
            .lock()
            .map(|attachments| attachments.policy_suppressed_count())
            .unwrap_or(0)
    }

    fn active_control_contacts(&self) -> Vec<ContactId> {
        self.control.active_contacts().unwrap_or_default()
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
    fn set_battery_policy(
        &mut self,
        profile: BatteryProfile,
        metered_transfers: MeteredTransferPolicy,
        metered_network: bool,
    ) {
        if let Ok(mut attachments) = self.attachments.lock() {
            attachments.set_battery_policy(profile, metered_transfers, metered_network);
        }
    }

    fn prepare_attachment(
        &mut self,
        request: &AttachmentSendRequest,
        now: Timestamp,
    ) -> Result<(), RuntimeDriverError> {
        self.attachment_job_sender
            .try_send(AttachmentWork::Prepare { request: request.clone(), now })
            .map_err(|_| {
                RuntimeDriverError::Classified(CommunicationError::Attachment.descriptor())
            })?;
        Ok(())
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

enum ControlWork {
    Recover(Timestamp),
    Maintenance { now: Timestamp, limit: usize },
    Reaction { contact_id: ContactId, reaction: ReactionPayload, at: Timestamp },
}

struct ControlDeliveryBridge {
    sender: SyncSender<ControlWork>,
    state: Arc<Mutex<ControlWorkerState>>,
}

struct ControlWorkerState {
    in_flight: bool,
    result: Option<Result<(), CommunicationError>>,
    next_delay: Option<Duration>,
    writes: u64,
    contacts: Vec<ContactId>,
    waker: Option<Arc<dyn Fn() + Send + Sync>>,
    wake_pending: bool,
}

impl ControlDeliveryBridge {
    fn new(runtime: Box<dyn ControlDeliveryRuntime>) -> Self {
        let (sender, receiver) = sync_channel(8);
        let state = Arc::new(Mutex::new(ControlWorkerState {
            in_flight: false,
            result: None,
            next_delay: None,
            writes: 0,
            contacts: Vec::new(),
            waker: None,
            wake_pending: false,
        }));
        let worker_state = Arc::clone(&state);
        thread::spawn(move || {
            let mut runtime = runtime;
            while let Ok(work) = receiver.recv() {
                let (result, now) = match work {
                    ControlWork::Recover(now) => (runtime.recover(now), now),
                    ControlWork::Maintenance { now, limit } => {
                        (runtime.maintenance(now, limit), now)
                    }
                    ControlWork::Reaction { contact_id, reaction, at } => {
                        (runtime.queue_reaction(contact_id, reaction, at), at)
                    }
                };
                let next_delay = runtime.next_maintenance_delay(now);
                let writes = runtime.database_write_count();
                let contacts = runtime.active_contacts().unwrap_or_default();
                let waker = worker_state.lock().ok().and_then(|mut value| {
                    value.in_flight = false;
                    value.result = Some(result);
                    value.next_delay = if value.wake_pending {
                        value.wake_pending = false;
                        Some(Duration::ZERO)
                    } else {
                        next_delay
                    };
                    value.writes = writes;
                    value.contacts = contacts;
                    value.waker.clone()
                });
                if let Some(waker) = waker {
                    waker();
                }
            }
        });
        Self { sender, state }
    }

    fn dispatch(&self, work: ControlWork) -> Result<(), CommunicationError> {
        if self.sender.try_send(work).is_err() {
            if let Ok(mut state) = self.state.lock() {
                state.in_flight = false;
            }
            return Err(CommunicationError::Control);
        }
        Ok(())
    }

    fn take_result(&self) -> Result<(), CommunicationError> {
        self.state.lock().map_err(|_| CommunicationError::Control)?.result.take().unwrap_or(Ok(()))
    }
}

impl ControlDeliveryRuntime for ControlDeliveryBridge {
    fn set_waker(&mut self, waker: Arc<dyn Fn() + Send + Sync>) {
        if let Ok(mut state) = self.state.lock() {
            state.waker = Some(waker);
        }
    }

    fn recover(&mut self, now: Timestamp) -> Result<(), CommunicationError> {
        self.take_result()?;
        if let Ok(mut state) = self.state.lock() {
            if state.in_flight {
                return Ok(());
            }
            state.in_flight = true;
        }
        self.dispatch(ControlWork::Recover(now))
    }

    fn wake(&mut self) {
        if let Ok(mut state) = self.state.lock() {
            if state.in_flight {
                state.wake_pending = true;
            } else {
                state.wake_pending = false;
                state.next_delay = Some(Duration::ZERO);
            }
        }
    }

    fn maintenance(&mut self, now: Timestamp, limit: usize) -> Result<(), CommunicationError> {
        self.take_result()?;
        if let Ok(mut state) = self.state.lock() {
            if state.in_flight {
                return Ok(());
            }
            // A completion wake is also used to deliver the worker result
            // back to the runtime actor.  Once that result has been consumed,
            // do not immediately enqueue another empty maintenance pass: the
            // next-due deadline is the authority for the next real run.
            let due = state.next_delay.is_some_and(|delay| delay.is_zero());
            if !due {
                return Ok(());
            }
            state.in_flight = true;
        }
        self.dispatch(ControlWork::Maintenance { now, limit })
    }

    fn next_maintenance_delay(&self, _now: Timestamp) -> Option<Duration> {
        let state = self.state.lock().ok()?;
        if state.in_flight { None } else { state.next_delay }
    }

    fn database_write_count(&self) -> u64 {
        self.state.lock().map(|state| state.writes).unwrap_or(0)
    }

    fn active_contacts(&self) -> Result<Vec<ContactId>, CommunicationError> {
        self.state
            .lock()
            .map(|state| state.contacts.clone())
            .map_err(|_| CommunicationError::Control)
    }

    fn queue_reaction(
        &mut self,
        contact_id: ContactId,
        reaction: ReactionPayload,
        at: Timestamp,
    ) -> Result<(), CommunicationError> {
        self.dispatch(ControlWork::Reaction { contact_id, reaction, at })?;
        if let Ok(mut state) = self.state.lock()
            && !state.contacts.contains(&contact_id)
        {
            state.contacts.push(contact_id);
        }
        Ok(())
    }
}

enum TextWork {
    Recover(Timestamp),
    Queue { message: Message, command_id: CommandId, next_attempt_at: Timestamp },
    Maintenance { now: Timestamp, limit: usize },
}

struct TextWorkerState {
    in_flight: bool,
    result: Option<Result<(), CommunicationError>>,
    next_delay: Option<Duration>,
    writes: u64,
    waker: Option<Arc<dyn Fn() + Send + Sync>>,
    wake_pending: bool,
    queued: VecDeque<TextWork>,
}

struct TextDeliveryBridge {
    sender: SyncSender<TextWork>,
    state: Arc<Mutex<TextWorkerState>>,
}

impl TextDeliveryBridge {
    fn new(runtime: Box<dyn TextDeliveryRuntime>) -> Self {
        let (sender, receiver) = sync_channel(1);
        let state = Arc::new(Mutex::new(TextWorkerState {
            in_flight: false,
            result: None,
            next_delay: None,
            writes: 0,
            waker: None,
            wake_pending: false,
            queued: VecDeque::new(),
        }));
        let worker_state = Arc::clone(&state);
        let worker_sender = sender.clone();
        thread::spawn(move || {
            let mut runtime = runtime;
            while let Ok(work) = receiver.recv() {
                let (result, now) = match work {
                    TextWork::Recover(now) => (runtime.recover(now), now),
                    TextWork::Queue { message, command_id, next_attempt_at } => (
                        runtime.queue_outbound(message, command_id, next_attempt_at),
                        next_attempt_at,
                    ),
                    TextWork::Maintenance { now, limit } => (runtime.maintenance(now, limit), now),
                };
                let next_delay = runtime.next_maintenance_delay(now);
                let writes = runtime.database_write_count();
                let (waker, next_work) = worker_state
                    .lock()
                    .ok()
                    .map(|mut value| {
                        value.in_flight = false;
                        value.result = Some(result);
                        let next_work = value.queued.pop_front();
                        value.in_flight = next_work.is_some();
                        value.next_delay = if next_work.is_some() {
                            None
                        } else if value.wake_pending {
                            value.wake_pending = false;
                            Some(Duration::ZERO)
                        } else {
                            next_delay
                        };
                        value.writes = writes;
                        (value.waker.clone(), next_work)
                    })
                    .unwrap_or((None, None));
                if let Some(next_work) = next_work {
                    // The receiver is the same thread, so a blocking send
                    // would deadlock when the bounded channel is full. The
                    // completed item has just been removed; try_send is the
                    // correct non-blocking hand-off here.
                    if let Err(TrySendError::Full(work)) = worker_sender.try_send(next_work)
                        && let Ok(mut value) = worker_state.lock()
                    {
                        value.queued.push_front(work);
                        value.in_flight = false;
                        value.next_delay = Some(Duration::ZERO);
                    }
                }
                if let Some(waker) = waker {
                    waker();
                }
            }
        });
        Self { sender, state }
    }

    fn take_result(&self) -> Result<(), CommunicationError> {
        self.state.lock().map_err(|_| CommunicationError::Text)?.result.take().unwrap_or(Ok(()))
    }
}

impl TextDeliveryRuntime for TextDeliveryBridge {
    fn queue_outbound(
        &mut self,
        message: Message,
        command_id: CommandId,
        next_attempt_at: Timestamp,
    ) -> Result<(), CommunicationError> {
        self.take_result()?;
        if let Ok(mut state) = self.state.lock() {
            if state.in_flight {
                if state.queued.len() >= 64 {
                    return Err(CommunicationError::Text);
                }
                state.queued.push_back(TextWork::Queue { message, command_id, next_attempt_at });
                return Ok(());
            }
            state.in_flight = true;
        }
        if self.sender.try_send(TextWork::Queue { message, command_id, next_attempt_at }).is_err() {
            if let Ok(mut state) = self.state.lock() {
                state.in_flight = false;
            }
            return Err(CommunicationError::Text);
        }
        Ok(())
    }

    fn set_waker(&mut self, waker: Arc<dyn Fn() + Send + Sync>) {
        if let Ok(mut state) = self.state.lock() {
            state.waker = Some(waker);
        }
    }

    fn recover(&mut self, now: Timestamp) -> Result<(), CommunicationError> {
        self.take_result()?;
        if let Ok(mut state) = self.state.lock() {
            if state.in_flight {
                return Ok(());
            }
            state.in_flight = true;
        }
        if self.sender.try_send(TextWork::Recover(now)).is_err() {
            if let Ok(mut state) = self.state.lock() {
                state.in_flight = false;
            }
            return Err(CommunicationError::Text);
        }
        Ok(())
    }

    fn wake(&mut self) {
        if let Ok(mut state) = self.state.lock() {
            if state.in_flight {
                state.wake_pending = true;
            } else {
                state.wake_pending = false;
                state.next_delay = Some(Duration::ZERO);
            }
        }
    }

    fn maintenance(&mut self, now: Timestamp, limit: usize) -> Result<(), CommunicationError> {
        self.take_result()?;
        if let Ok(mut state) = self.state.lock() {
            if state.in_flight {
                return Ok(());
            }
            // See the control worker above: a worker completion must not
            // turn into an unbounded empty dispatch loop.
            let due = state.next_delay.is_some_and(|delay| delay.is_zero());
            if !due {
                return Ok(());
            }
            state.in_flight = true;
        }
        if self.sender.try_send(TextWork::Maintenance { now, limit }).is_err() {
            if let Ok(mut state) = self.state.lock() {
                state.in_flight = false;
            }
            return Err(CommunicationError::Text);
        }
        Ok(())
    }

    fn next_maintenance_delay(&self, _now: Timestamp) -> Option<Duration> {
        let state = self.state.lock().ok()?;
        if state.in_flight { None } else { state.next_delay }
    }

    fn database_write_count(&self) -> u64 {
        self.state.lock().map(|state| state.writes).unwrap_or(0)
    }
}

fn spawn_attachment_worker(
    attachments: Arc<Mutex<Box<dyn AttachmentRuntime>>>,
    active: Arc<AtomicBool>,
    result_slot: Arc<Mutex<Option<AttachmentMaintenanceResult>>>,
    error_slot: Arc<Mutex<Option<CommunicationError>>>,
    waker: WakeSlot,
    projection_cache: Arc<Mutex<Vec<AttachmentView>>>,
    receiver: Receiver<AttachmentWork>,
) {
    thread::spawn(move || {
        while let Ok(work) = receiver.recv() {
            let result = attachments.lock().map_err(|_| CommunicationError::Attachment).and_then(
                |mut runtime| match work {
                    AttachmentWork::Maintenance { now } => {
                        let outcome = runtime.maintenance_outgoing(now, ATTACHMENT_BATCH)?;
                        refresh_attachment_cache(&projection_cache, &**runtime);
                        Ok(Some(outcome))
                    }
                    AttachmentWork::Prepare { request, now } => {
                        runtime.prepare_outgoing(&request, now)?;
                        refresh_attachment_cache(&projection_cache, &**runtime);
                        // Preparation creates a durable outbound job.  Publish
                        // that fact through the same completion lane as a
                        // maintenance turn so the actor arms the transfer
                        // scheduler immediately instead of waiting for an
                        // unrelated timer or lifecycle event.
                        Ok(Some(AttachmentMaintenanceResult {
                            more_work: true,
                            policy_blocked: false,
                            retry_after_ms: Some(0),
                        }))
                    }
                },
            );
            let outcome = match result {
                Ok(Some(outcome)) => outcome,
                Err(error) => {
                    eprintln!("torca-attachment: worker failed code={error}");
                    if let Ok(mut slot) = error_slot.lock() {
                        *slot = Some(error);
                    }
                    AttachmentMaintenanceResult {
                        more_work: true,
                        policy_blocked: false,
                        retry_after_ms: Some(2_000),
                    }
                }
                Ok(None) => unreachable!(),
            };
            if let Ok(mut slot) = result_slot.lock() {
                *slot = Some(outcome);
            }
            active.store(false, Ordering::Release);
            if let Some(callback) = waker.lock().ok().and_then(|target| target.clone()) {
                callback();
            }
        }
    });
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
    use super::{
        ATTACHMENT_MESSAGE_KIND, CommunicationError, PROBE_MESSAGE_KIND,
        RADIO_CONTROL_MESSAGE_KIND, REACTION_MESSAGE_KIND, RECEIPT_MESSAGE_KIND, TEXT_MESSAGE_KIND,
        map_runtime, plan_read_receipts,
    };
    use std::collections::BTreeSet;
    use torca_control_delivery::ReadCandidate;
    use torca_foundation::{ClassifiedError, OpaqueId, Timestamp};

    #[test]
    fn peer_application_message_kinds_are_unique_and_stable() {
        let kinds = [
            TEXT_MESSAGE_KIND,
            RECEIPT_MESSAGE_KIND,
            ATTACHMENT_MESSAGE_KIND,
            PROBE_MESSAGE_KIND,
            RADIO_CONTROL_MESSAGE_KIND,
            REACTION_MESSAGE_KIND,
        ];
        assert_eq!(kinds.iter().copied().collect::<BTreeSet<_>>().len(), kinds.len());
        assert_eq!(kinds, [1, 2, 3, 4, 5, 6]);
    }

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
