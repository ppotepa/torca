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
};
use std::thread;
use std::time::Duration;

use attachment_scheduler::AttachmentJobScheduler;
pub use error::{AttachmentFailureStage, CommunicationError};
pub use policy::{
    ATTACHMENT_MESSAGE_KIND, PROBE_MESSAGE_KIND, RADIO_CONTROL_MESSAGE_KIND,
    REACTION_MESSAGE_KIND, RECEIPT_MESSAGE_KIND, TEXT_MESSAGE_KIND, classify_peer_health,
    plan_read_receipts,
};
pub use ports::{
    AttachmentExportRuntime, AttachmentMaintenanceResult, AttachmentRuntime,
    ControlDeliveryRuntime, InboundEnvelope, InboundMessagingRuntime, PeerLinkRuntime,
    RadioInboundRuntime, ReadStateRuntime, RelationshipAdminRuntime, TextDeliveryRuntime,
};

use torca_attachments::AttachmentId;
use torca_battery::{BatteryProfile, MeteredTransferPolicy};
use torca_client_engine::EngineHandle;
use torca_contacts::ContactId;
use torca_conversations::ConversationId;
use torca_delivery::ReactionPayload;
use torca_foundation::{ClassifiedError, OpaqueId, Timestamp};
pub use torca_runtime::{PeerActivityEvidence, PeerConnectionStatus};
use torca_runtime::{
    AttachmentExportPort, AttachmentSendRequest, AttachmentTransferPort, AttachmentView,
    ContactVerificationSnapshot, ConversationReadPort, PeerSessionPort, RelationshipAdminPort,
    RuntimeDriverError,
};
pub use torca_runtime::{PeerHealthQuality, PeerHealthSnapshot};

type WakeCallback = Arc<dyn Fn() + Send + Sync>;
type WakeSlot = Arc<Mutex<Option<WakeCallback>>>;

const INBOUND_BATCH: usize = 64;
const TEXT_BATCH: usize = 16;
const CONTROL_BATCH: usize = 16;
// Process a bounded batch per worker turn. This keeps transport fairness while
// avoiding one OS thread per 64 KiB chunk for large attachments.
const ATTACHMENT_BATCH: usize = 8;
const MAX_DEFERRED_ATTACHMENTS: usize = 64;

pub struct TorcaCommunicationDriver {
    engine: EngineHandle,
    peer: Box<dyn PeerLinkRuntime>,
    text: Box<dyn TextDeliveryRuntime>,
    control: Box<dyn ControlDeliveryRuntime>,
    inbound: Box<dyn InboundMessagingRuntime>,
    attachments: Arc<Mutex<Box<dyn AttachmentRuntime>>>,
    attachment_job_active: Arc<AtomicBool>,
    attachment_job_result: Arc<Mutex<Option<AttachmentMaintenanceResult>>>,
    attachment_waker: WakeSlot,
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
            attachment_job_result: Arc::new(Mutex::new(None)),
            attachment_waker: Arc::new(Mutex::new(None)),
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
        self.peer.set_waker(Arc::clone(&waker));
        if let Ok(mut target) = self.attachment_waker.lock() {
            *target = Some(waker);
        }
    }

    fn attachment_runtime(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, Box<dyn AttachmentRuntime>>, CommunicationError> {
        self.attachments
            .lock()
            .map_err(|_| CommunicationError::Attachment)
    }

    pub fn peer_health(&self, contact_id: ContactId) -> PeerHealthSnapshot {
        self.peer.peer_health(contact_id)
    }

    /// Defers an attachment frame without letting it block text/control ingress.
    fn defer_attachment(&mut self, envelope: InboundEnvelope) {
        if self.deferred_attachments.iter().any(|queued| {
            queued.contact_id == envelope.contact_id
                && queued.envelope_id == envelope.envelope_id
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
                        refresh_attachment_cache(
                            &self.attachment_snapshot_cache,
                            &**attachments,
                        );
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
                    self.attachment_scheduler
                        .wake_after(now, Duration::from_millis(delay));
                } else {
                    self.attachment_scheduler.wake();
                }
            } else {
                self.attachment_scheduler.disarm();
            }
        }
        self.peer.maintenance(contacts, now).map_err(map_runtime)?;
        self.drain_inbound(now).map_err(map_runtime)?;
        self.text
            .maintenance(now, TEXT_BATCH)
            .map_err(map_runtime)?;
        self.control
            .maintenance(now, CONTROL_BATCH)
            .map_err(map_runtime)?;
        if let Some(radio) = self.radio.as_mut()
            && let Err(error) = radio.maintenance(now)
        {
            eprintln!("torca-radio: background maintenance failed code={error}");
        }
        if self.attachment_scheduler.due(now)
            && !self.attachment_job_active.swap(true, Ordering::AcqRel)
        {
            let attachments = Arc::clone(&self.attachments);
            let active = Arc::clone(&self.attachment_job_active);
            let result_slot = Arc::clone(&self.attachment_job_result);
            let waker = Arc::clone(&self.attachment_waker);
            let projection_cache = Arc::clone(&self.attachment_snapshot_cache);
            self.attachment_scheduler.disarm();
            thread::spawn(move || {
                let result = attachments
                    .lock()
                    .map_err(|_| CommunicationError::Attachment)
                    .and_then(|mut runtime| {
                        let outcome = runtime.maintenance_outgoing(now, ATTACHMENT_BATCH)?;
                        refresh_attachment_cache(&projection_cache, &**runtime);
                        Ok(outcome)
                    });
                let outcome = match result {
                    Ok(outcome) => outcome,
                    Err(error) => {
                        eprintln!("torca-attachment: maintenance failed code={error}");
                        AttachmentMaintenanceResult {
                            more_work: true,
                            policy_blocked: false,
                            retry_after_ms: Some(2_000),
                        }
                    }
                };
                if let Ok(mut slot) = result_slot.lock() {
                    *slot = Some(outcome);
                }
                active.store(false, Ordering::Release);
                let callback = waker.lock().ok().and_then(|target| target.clone());
                if let Some(callback) = callback {
                    callback();
                }
            });
        }
        Ok(())
    }

    fn next_maintenance_delay(&self, now: Timestamp) -> Option<Duration> {
        [
            self.peer.next_maintenance_delay(now),
            self.text.next_maintenance_delay(now),
            self.control.next_maintenance_delay(now),
            self.attachment_scheduler.next_delay(now),
            self.radio
                .as_ref()
                .and_then(|radio| radio.next_maintenance_delay(now)),
        ]
        .into_iter()
        .flatten()
        .min()
    }

    fn network_changed(&mut self, now: Timestamp) {
        self.peer.network_changed(now);
    }

    fn set_waker(&mut self, waker: Arc<dyn Fn() + Send + Sync>) {
        if let Ok(mut target) = self.attachment_waker.lock() {
            *target = Some(Arc::clone(&waker));
        }
        self.peer.set_waker(waker);
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

    fn peer_probe_eligible(&self, contact_id: ContactId) -> bool {
        self.peer.peer_probe_eligible(contact_id)
    }

    fn begin_peer_probe(
        &mut self,
        contact_id: ContactId,
        probe_id: OpaqueId,
        reported_rtt_ms: u64,
    ) -> Result<(), RuntimeDriverError> {
        self.peer
            .begin_probe(contact_id, probe_id, reported_rtt_ms)
            .map_err(map_runtime)
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
        self.text.database_write_count()
            + self.control.database_write_count()
            + self.inbound.database_write_count()
            + attachment_writes
    }

    fn blob_write_count(&self) -> u64 {
        self.attachments
            .lock()
            .map(|attachments| attachments.blob_write_count())
            .unwrap_or(0)
    }

    fn attachment_chunk_tx_count(&self) -> u64 {
        self.attachments
            .lock()
            .map(|attachments| attachments.chunk_tx_count())
            .unwrap_or(0)
    }

    fn attachment_policy_suppressed_count(&self) -> u64 {
        self.attachments
            .lock()
            .map(|attachments| attachments.policy_suppressed_count())
            .unwrap_or(0)
    }

    fn queue_reaction(
        &mut self,
        contact_id: ContactId,
        reaction: ReactionPayload,
        at: Timestamp,
    ) -> Result<(), RuntimeDriverError> {
        self.control
            .queue_reaction(contact_id, reaction, at)
            .map_err(map_runtime)
    }
}

impl RelationshipAdminPort for TorcaCommunicationDriver {
    fn contact_names(&self) -> Result<BTreeMap<ContactId, String>, RuntimeDriverError> {
        self.relationships.contact_names().map_err(map_runtime)
    }

    fn contact_verifications(
        &self,
    ) -> Result<BTreeMap<ContactId, ContactVerificationSnapshot>, RuntimeDriverError> {
        self.relationships
            .contact_verifications()
            .map_err(map_runtime)
    }

    fn verify_contact(
        &mut self,
        id: ContactId,
        now: Timestamp,
    ) -> Result<(), RuntimeDriverError> {
        self.relationships
            .verify_contact(id, now)
            .map_err(map_runtime)
    }

    fn reset_contact_verification(&mut self, id: ContactId) -> Result<(), RuntimeDriverError> {
        self.relationships
            .reset_contact_verification(id)
            .map_err(map_runtime)
    }

    fn rename_contact(
        &mut self,
        id: ContactId,
        name: String,
        now: Timestamp,
    ) -> Result<(), RuntimeDriverError> {
        self.relationships
            .rename_contact(id, name, now)
            .map_err(map_runtime)
    }

    fn block_contact(
        &mut self,
        id: ContactId,
        now: Timestamp,
    ) -> Result<(), RuntimeDriverError> {
        self.relationships
            .block_contact(id, now)
            .map_err(map_runtime)?;
        self.peer.shutdown();
        Ok(())
    }

    fn unblock_contact(
        &mut self,
        id: ContactId,
        now: Timestamp,
    ) -> Result<(), RuntimeDriverError> {
        self.relationships
            .unblock_contact(id, now)
            .map_err(map_runtime)
    }

    fn clear_conversation_history(
        &mut self,
        id: ConversationId,
    ) -> Result<(), RuntimeDriverError> {
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
        self.read_state
            .mark_conversation_read(id, now)
            .map_err(map_runtime)
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
        let projection_cache = Arc::clone(&self.attachment_snapshot_cache);
        let result = {
            let mut attachments = self.attachment_runtime().map_err(map_runtime)?;
            let result = attachments
                .prepare_outgoing(request, now)
                .map_err(map_runtime);
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

    fn retry_attachment(
        &mut self,
        id: OpaqueId,
        now: Timestamp,
    ) -> Result<(), RuntimeDriverError> {
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
        let snapshot = if let Some(snapshot) = attachments
            .snapshot_projection()
            .map_err(map_runtime)?
        {
            snapshot
        } else {
            let messages = self
                .engine
                .snapshot()
                .map_err(|_| RuntimeDriverError::Engine)?;
            attachments
                .snapshot(&messages.messages)
                .map_err(map_runtime)?
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
        self.attachment_export
            .export_attachment(id, destination)
            .map_err(map_runtime)
    }

    fn export_attachment_preview(
        &mut self,
        id: AttachmentId,
        destination: PathBuf,
    ) -> Result<(), RuntimeDriverError> {
        self.attachment_export
            .export_attachment_preview(id, destination)
            .map_err(map_runtime)
    }
}

fn refresh_attachment_cache(
    cache: &Mutex<Vec<AttachmentView>>,
    runtime: &dyn AttachmentRuntime,
) {
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
        RADIO_CONTROL_MESSAGE_KIND, REACTION_MESSAGE_KIND, RECEIPT_MESSAGE_KIND,
        TEXT_MESSAGE_KIND, map_runtime, plan_read_receipts,
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
        assert_eq!(
            kinds.iter().copied().collect::<BTreeSet<_>>().len(),
            kinds.len()
        );
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
        assert_eq!(
            error.descriptor().code().as_str(),
            "communication.peer_unavailable"
        );
    }

    #[test]
    fn attachment_delivery_failure_is_retryable() {
        let error = map_runtime(CommunicationError::Attachment);
        let descriptor = error.descriptor();
        assert_eq!(
            descriptor.code().as_str(),
            "communication.attachment_unavailable"
        );
        assert_eq!(
            descriptor.retry_advice(),
            torca_foundation::RetryAdvice::Backoff
        );
    }
}
