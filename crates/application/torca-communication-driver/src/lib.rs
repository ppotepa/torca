//! One communication supervisor over the process-owned authenticated peer link.

use core::fmt;
use std::collections::BTreeMap;
use std::path::PathBuf;

use torca_attachments::AttachmentId;
use torca_client_engine::EngineHandle;
use torca_contacts::ContactId;
use torca_conversations::ConversationId;
use torca_foundation::{OpaqueId, Timestamp};
use torca_messaging::Message;
use torca_peer_link::{InboundPeerEnvelope, PeerConnectionState};
use torca_runtime_host::{AttachmentSendRequest, AttachmentView, CommunicationDriver, RuntimeDriverError};

pub const TEXT_MESSAGE_KIND: u16 = 1;
pub const RECEIPT_MESSAGE_KIND: u16 = 2;
pub const ATTACHMENT_MESSAGE_KIND: u16 = 3;
const INBOUND_BATCH: usize = 64;
const TEXT_BATCH: usize = 16;
const CONTROL_BATCH: usize = 16;
const ATTACHMENT_BATCH: usize = 2;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommunicationError { Peer, Text, Control, Inbound, Attachment, ReadState, Relationship, Engine }
impl fmt::Display for CommunicationError { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{self:?}") } }
impl std::error::Error for CommunicationError {}

pub trait PeerLinkRuntime: Send {
    fn maintenance(&mut self, contacts: &[ContactId], now: Timestamp) -> Result<(), CommunicationError>;
    fn connection_state(&self, contact_id: ContactId) -> PeerConnectionState;
    fn take_inbound(&mut self) -> Result<Option<InboundPeerEnvelope>, CommunicationError>;
    fn reject(&mut self, envelope: &InboundPeerEnvelope) -> Result<(), CommunicationError>;
    fn shutdown(&mut self);
}
pub trait TextDeliveryRuntime: Send { fn recover(&mut self, now: Timestamp) -> Result<(), CommunicationError>; fn maintenance(&mut self, now: Timestamp, limit: usize) -> Result<(), CommunicationError>; }
pub trait ControlDeliveryRuntime: Send { fn recover(&mut self, now: Timestamp) -> Result<(), CommunicationError>; fn maintenance(&mut self, now: Timestamp, limit: usize) -> Result<(), CommunicationError>; }
pub trait InboundMessagingRuntime: Send { fn process(&mut self, envelope: InboundPeerEnvelope, now: Timestamp) -> Result<(), CommunicationError>; }
pub trait AttachmentRuntime: Send {
    fn prepare_outgoing(&mut self, request: &AttachmentSendRequest, now: Timestamp) -> Result<(), CommunicationError>;
    fn retry(&mut self, attachment_id: OpaqueId, now: Timestamp) -> Result<(), CommunicationError>;
    fn cancel(&mut self, attachment_id: OpaqueId, now: Timestamp) -> Result<(), CommunicationError>;
    fn snapshot(&self, messages: &[Message]) -> Result<Vec<AttachmentView>, CommunicationError>;
    fn process_inbound(&mut self, envelope: InboundPeerEnvelope, now: Timestamp) -> Result<(), CommunicationError>;
    fn maintenance_outgoing(&mut self, messages: &[Message], now: Timestamp, limit: usize) -> Result<(), CommunicationError>;
    fn shutdown(&mut self);
}
pub trait AttachmentExportRuntime: Send {
    fn export_attachment(&mut self, attachment_id: AttachmentId, destination: PathBuf) -> Result<(), CommunicationError>;
}
pub trait ReadStateRuntime: Send { fn mark_conversation_read(&mut self, conversation_id: OpaqueId, now: Timestamp) -> Result<(), CommunicationError>; }
pub trait RelationshipAdminRuntime: Send {
    fn contact_names(&self) -> Result<BTreeMap<ContactId, String>, CommunicationError>;
    fn rename_contact(&mut self, contact_id: ContactId, display_name: String, now: Timestamp) -> Result<(), CommunicationError>;
    fn block_contact(&mut self, contact_id: ContactId, now: Timestamp) -> Result<(), CommunicationError>;
    fn unblock_contact(&mut self, contact_id: ContactId, now: Timestamp) -> Result<(), CommunicationError>;
    fn clear_history(&mut self, conversation_id: ConversationId) -> Result<(), CommunicationError>;
    fn remove_contact(&mut self, contact_id: ContactId) -> Result<(), CommunicationError>;
}

pub struct TorcaCommunicationDriver {
    engine: EngineHandle,
    peer: Box<dyn PeerLinkRuntime>, text: Box<dyn TextDeliveryRuntime>, control: Box<dyn ControlDeliveryRuntime>,
    inbound: Box<dyn InboundMessagingRuntime>, attachments: Box<dyn AttachmentRuntime>,
    attachment_export: Box<dyn AttachmentExportRuntime>, read_state: Box<dyn ReadStateRuntime>,
    relationships: Box<dyn RelationshipAdminRuntime>,
}
impl TorcaCommunicationDriver {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        engine: EngineHandle, peer: Box<dyn PeerLinkRuntime>, text: Box<dyn TextDeliveryRuntime>,
        control: Box<dyn ControlDeliveryRuntime>, inbound: Box<dyn InboundMessagingRuntime>,
        attachments: Box<dyn AttachmentRuntime>, attachment_export: Box<dyn AttachmentExportRuntime>,
        read_state: Box<dyn ReadStateRuntime>, relationships: Box<dyn RelationshipAdminRuntime>,
    ) -> Self {
        Self { engine, peer, text, control, inbound, attachments, attachment_export, read_state, relationships }
    }
    fn drain_inbound(&mut self, now: Timestamp) -> Result<(), CommunicationError> {
        for _ in 0..INBOUND_BATCH {
            let Some(envelope) = self.peer.take_inbound()? else { break; };
            match envelope.message_kind {
                TEXT_MESSAGE_KIND | RECEIPT_MESSAGE_KIND => self.inbound.process(envelope, now)?,
                ATTACHMENT_MESSAGE_KIND => self.attachments.process_inbound(envelope, now)?,
                _ => self.peer.reject(&envelope)?,
            }
        }
        Ok(())
    }
}
impl CommunicationDriver for TorcaCommunicationDriver {
    fn recover(&mut self, now: Timestamp) -> Result<(), RuntimeDriverError> { self.text.recover(now).map_err(map_runtime)?; self.control.recover(now).map_err(map_runtime)?; Ok(()) }
    fn maintenance(&mut self, contacts: &[ContactId], now: Timestamp) -> Result<(), RuntimeDriverError> {
        self.peer.maintenance(contacts, now).map_err(map_runtime)?;
        self.drain_inbound(now).map_err(map_runtime)?;
        self.text.maintenance(now, TEXT_BATCH).map_err(map_runtime)?;
        self.control.maintenance(now, CONTROL_BATCH).map_err(map_runtime)?;
        let snapshot = self.engine.snapshot().map_err(|_| RuntimeDriverError::Engine)?;
        self.attachments.maintenance_outgoing(&snapshot.messages, now, ATTACHMENT_BATCH).map_err(map_runtime)?;
        Ok(())
    }
    fn connection_state(&self, contact_id: ContactId) -> PeerConnectionState { self.peer.connection_state(contact_id) }
    fn contact_names(&self) -> Result<BTreeMap<ContactId, String>, RuntimeDriverError> { self.relationships.contact_names().map_err(map_runtime) }
    fn rename_contact(&mut self, id: ContactId, name: String, now: Timestamp) -> Result<(), RuntimeDriverError> { self.relationships.rename_contact(id, name, now).map_err(map_runtime) }
    fn block_contact(&mut self, id: ContactId, now: Timestamp) -> Result<(), RuntimeDriverError> { self.relationships.block_contact(id, now).map_err(map_runtime)?; self.peer.shutdown(); Ok(()) }
    fn unblock_contact(&mut self, id: ContactId, now: Timestamp) -> Result<(), RuntimeDriverError> { self.relationships.unblock_contact(id, now).map_err(map_runtime) }
    fn clear_conversation_history(&mut self, id: ConversationId) -> Result<(), RuntimeDriverError> { self.relationships.clear_history(id).map_err(map_runtime) }
    fn remove_contact(&mut self, id: ContactId) -> Result<(), RuntimeDriverError> { self.relationships.remove_contact(id).map_err(map_runtime)?; self.peer.shutdown(); Ok(()) }
    fn mark_conversation_read(&mut self, id: OpaqueId, now: Timestamp) -> Result<(), RuntimeDriverError> { self.read_state.mark_conversation_read(id, now).map_err(map_runtime) }
    fn prepare_attachment(&mut self, request: &AttachmentSendRequest, now: Timestamp) -> Result<(), RuntimeDriverError> { self.attachments.prepare_outgoing(request, now).map_err(map_runtime) }
    fn retry_attachment(&mut self, id: OpaqueId, now: Timestamp) -> Result<(), RuntimeDriverError> { self.attachments.retry(id, now).map_err(map_runtime) }
    fn cancel_attachment(&mut self, id: OpaqueId, now: Timestamp) -> Result<(), RuntimeDriverError> { self.attachments.cancel(id, now).map_err(map_runtime) }
    fn export_attachment(&mut self, id: AttachmentId, destination: PathBuf) -> Result<(), RuntimeDriverError> { self.attachment_export.export_attachment(id, destination).map_err(map_runtime) }
    fn attachment_snapshot(&self) -> Result<Vec<AttachmentView>, RuntimeDriverError> { let snapshot = self.engine.snapshot().map_err(|_| RuntimeDriverError::Engine)?; self.attachments.snapshot(&snapshot.messages).map_err(map_runtime) }
    fn shutdown(&mut self) { self.attachments.shutdown(); self.peer.shutdown(); }
}
fn map_runtime(_: CommunicationError) -> RuntimeDriverError { RuntimeDriverError::Communication }
