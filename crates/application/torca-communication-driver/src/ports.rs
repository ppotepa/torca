use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use torca_attachments::AttachmentId;
use torca_battery::{BatteryProfile, MeteredTransferPolicy};
use torca_contacts::ContactId;
use torca_conversations::ConversationId;
use torca_delivery::ReactionPayload;
use torca_foundation::{OpaqueId, Timestamp};
use torca_messaging::Message;
use torca_runtime::{
    AttachmentSendRequest, AttachmentView, ContactVerificationSnapshot, PeerActivityEvidence,
    PeerConnectionStatus, PeerHealthSnapshot,
};

use crate::CommunicationError;

/// Provider-neutral inbound envelope owned by the application boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InboundEnvelope {
    pub contact_id: ContactId,
    pub envelope_id: OpaqueId,
    pub message_kind: u16,
    pub ciphertext: Vec<u8>,
}

pub trait PeerLinkRuntime: Send {
    fn maintenance(
        &mut self,
        contacts: &[ContactId],
        now: Timestamp,
    ) -> Result<(), CommunicationError>;

    fn next_maintenance_delay(&self, _now: Timestamp) -> Option<Duration> {
        None
    }

    fn network_changed(&mut self, _now: Timestamp) {}
    /// Requests a one-shot connection warm-up for relationships that were
    /// just created or restored. This is intentionally event-driven and is
    /// not a permanent keep-alive policy.
    fn prime_connections(&mut self) {}

    fn set_waker(&mut self, _waker: Arc<dyn Fn() + Send + Sync>) {}

    fn connection_state(&self, contact_id: ContactId) -> PeerConnectionStatus;

    fn take_inbound(&mut self) -> Result<Option<InboundEnvelope>, CommunicationError>;

    fn reject(&mut self, envelope: &InboundEnvelope) -> Result<(), CommunicationError>;

    fn shutdown(&mut self);

    fn peer_health(&self, contact_id: ContactId) -> PeerHealthSnapshot {
        PeerHealthSnapshot::from_connection_state(self.connection_state(contact_id))
    }

    fn peer_activity(&self) -> Vec<PeerActivityEvidence> {
        Vec::new()
    }

    fn close_idle_peers(
        &mut self,
        _retained: &[ContactId],
        _now: Timestamp,
    ) -> Result<usize, CommunicationError> {
        Ok(0)
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
    fn set_waker(&mut self, _waker: Arc<dyn Fn() + Send + Sync>) {}
    fn recover(&mut self, now: Timestamp) -> Result<(), CommunicationError>;
    fn maintenance(&mut self, now: Timestamp, limit: usize) -> Result<(), CommunicationError>;

    fn next_maintenance_delay(&self, _now: Timestamp) -> Option<Duration> {
        None
    }

    fn database_write_count(&self) -> u64 {
        0
    }
}

pub trait ControlDeliveryRuntime: Send {
    fn set_waker(&mut self, _waker: Arc<dyn Fn() + Send + Sync>) {}
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

    fn database_write_count(&self) -> u64 {
        0
    }
}

pub trait AttachmentRuntime: Send {
    fn set_battery_policy(
        &mut self,
        _profile: BatteryProfile,
        _metered_transfers: MeteredTransferPolicy,
        _metered_network: bool,
    ) {
    }

    fn prepare_outgoing(
        &mut self,
        request: &AttachmentSendRequest,
        now: Timestamp,
    ) -> Result<(), CommunicationError>;

    fn retry(&mut self, attachment_id: OpaqueId, now: Timestamp) -> Result<(), CommunicationError>;

    fn cancel(&mut self, attachment_id: OpaqueId, now: Timestamp)
    -> Result<(), CommunicationError>;

    fn snapshot(&self, messages: &[Message]) -> Result<Vec<AttachmentView>, CommunicationError>;

    fn snapshot_projection(&self) -> Result<Option<Vec<AttachmentView>>, CommunicationError> {
        Ok(None)
    }

    fn database_write_count(&self) -> u64 {
        0
    }

    fn blob_write_count(&self) -> u64 {
        0
    }

    fn chunk_tx_count(&self) -> u64 {
        0
    }

    fn policy_suppressed_count(&self) -> u64 {
        0
    }

    fn process_inbound(
        &mut self,
        envelope: InboundEnvelope,
        now: Timestamp,
    ) -> Result<(), CommunicationError>;

    fn maintenance_outgoing(
        &mut self,
        now: Timestamp,
        limit: usize,
    ) -> Result<AttachmentMaintenanceResult, CommunicationError>;

    fn shutdown(&mut self);
}

/// Result of one bounded attachment maintenance pass. The runtime uses this
/// to arm the next exact-ish retry only when the pass found work that can make
/// progress. An empty result must leave the scheduler asleep.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AttachmentMaintenanceResult {
    pub more_work: bool,
    pub policy_blocked: bool,
    pub retry_after_ms: Option<u64>,
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

    fn next_maintenance_delay(&self, _now: Timestamp) -> Option<Duration> {
        None
    }

    fn shutdown(&mut self) {}
}
