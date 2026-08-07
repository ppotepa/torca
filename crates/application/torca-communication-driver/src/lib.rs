//! One communication supervisor over the process-owned authenticated peer link.
//!
//! This layer is deliberately transport-agnostic. Concrete adapters wrap PeerLink, SQLCipher
//! delivery workers, encrypted attachment transfer and read-state storage. Exactly one dispatcher
//! consumes inbound peer envelopes and routes them by message kind.

use core::fmt;

use torca_client_engine::EngineHandle;
use torca_contacts::ContactId;
use torca_foundation::{OpaqueId, Timestamp};
use torca_messaging::Message;
use torca_peer_link::{InboundPeerEnvelope, PeerConnectionState};
use torca_runtime_host::{CommunicationDriver, RuntimeDriverError};

pub const TEXT_MESSAGE_KIND: u16 = 1;
pub const RECEIPT_MESSAGE_KIND: u16 = 2;
pub const ATTACHMENT_MESSAGE_KIND: u16 = 3;
const INBOUND_BATCH: usize = 64;
const TEXT_BATCH: usize = 16;
const CONTROL_BATCH: usize = 16;
const ATTACHMENT_BATCH: usize = 2;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommunicationError {
    Peer,
    Text,
    Control,
    Inbound,
    Attachment,
    ReadState,
    Engine,
}
impl fmt::Display for CommunicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for CommunicationError {}

pub trait PeerLinkRuntime: Send {
    fn maintenance(
        &mut self,
        contacts: &[ContactId],
        now: Timestamp,
    ) -> Result<(), CommunicationError>;
    fn connection_state(&self, contact_id: ContactId) -> PeerConnectionState;
    fn take_inbound(&mut self) -> Result<Option<InboundPeerEnvelope>, CommunicationError>;
    fn reject(&mut self, envelope: &InboundPeerEnvelope) -> Result<(), CommunicationError>;
    fn shutdown(&mut self);
}

pub trait TextDeliveryRuntime: Send {
    fn recover(&mut self, now: Timestamp) -> Result<(), CommunicationError>;
    fn maintenance(&mut self, now: Timestamp, limit: usize) -> Result<(), CommunicationError>;
}

pub trait ControlDeliveryRuntime: Send {
    fn recover(&mut self, now: Timestamp) -> Result<(), CommunicationError>;
    fn maintenance(&mut self, now: Timestamp, limit: usize) -> Result<(), CommunicationError>;
}

pub trait InboundMessagingRuntime: Send {
    fn process(
        &mut self,
        envelope: InboundPeerEnvelope,
        now: Timestamp,
    ) -> Result<(), CommunicationError>;
}

pub trait AttachmentRuntime: Send {
    fn process_inbound(
        &mut self,
        envelope: InboundPeerEnvelope,
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

pub trait ReadStateRuntime: Send {
    fn mark_conversation_read(
        &mut self,
        conversation_id: OpaqueId,
        now: Timestamp,
    ) -> Result<(), CommunicationError>;
}

/// Final application communication driver. The `RuntimeHost` owns exactly one instance.
pub struct TorcaCommunicationDriver {
    engine: EngineHandle,
    peer: Box<dyn PeerLinkRuntime>,
    text: Box<dyn TextDeliveryRuntime>,
    control: Box<dyn ControlDeliveryRuntime>,
    inbound: Box<dyn InboundMessagingRuntime>,
    attachments: Box<dyn AttachmentRuntime>,
    read_state: Box<dyn ReadStateRuntime>,
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
        read_state: Box<dyn ReadStateRuntime>,
    ) -> Self {
        Self { engine, peer, text, control, inbound, attachments, read_state }
    }

    fn drain_inbound(&mut self, now: Timestamp) -> Result<(), CommunicationError> {
        for _ in 0..INBOUND_BATCH {
            let Some(envelope) = self.peer.take_inbound()? else {
                break;
            };
            match envelope.message_kind {
                TEXT_MESSAGE_KIND | RECEIPT_MESSAGE_KIND => {
                    self.inbound.process(envelope, now)?;
                }
                ATTACHMENT_MESSAGE_KIND => {
                    self.attachments.process_inbound(envelope, now)?;
                }
                _ => {
                    self.peer.reject(&envelope)?;
                }
            }
        }
        Ok(())
    }
}

impl CommunicationDriver for TorcaCommunicationDriver {
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
        self.control
            .maintenance(now, CONTROL_BATCH)
            .map_err(map_runtime)?;
        let snapshot = self.engine.snapshot().map_err(|_| RuntimeDriverError::Engine)?;
        self.attachments
            .maintenance_outgoing(&snapshot.messages, now, ATTACHMENT_BATCH)
            .map_err(map_runtime)?;
        Ok(())
    }

    fn connection_state(&self, contact_id: ContactId) -> PeerConnectionState {
        self.peer.connection_state(contact_id)
    }

    fn mark_conversation_read(
        &mut self,
        conversation_id: OpaqueId,
        now: Timestamp,
    ) -> Result<(), RuntimeDriverError> {
        self.read_state
            .mark_conversation_read(conversation_id, now)
            .map_err(map_runtime)
    }

    fn shutdown(&mut self) {
        self.attachments.shutdown();
        self.peer.shutdown();
    }
}

fn map_runtime(_: CommunicationError) -> RuntimeDriverError {
    RuntimeDriverError::Communication
}
