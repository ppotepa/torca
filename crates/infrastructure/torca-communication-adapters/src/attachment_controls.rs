use std::fs::File;
use std::io::BufReader;

use crate::peer_envelope;
use torca_attachment_protocol::{AttachmentPreviewFrame, MAX_ATTACHMENT_PREVIEW};
use torca_attachment_sqlite::{SqlCipherAttachmentProjection, SqlCipherAttachmentStore};
use torca_attachment_transfer::{AttachmentTransfer, AttachmentTransferError};
use torca_attachments::{
    Attachment, AttachmentId, AttachmentName, AttachmentRepository, AttachmentStatus,
    MAX_ATTACHMENT_BYTES, MediaType,
};
use torca_communication_driver::{
    AttachmentFailureStage, AttachmentRuntime, CommunicationError, InboundEnvelope,
};
use torca_contacts::{ContactRepository, PeerCredentialRepository};
use torca_conversations::ConversationRepository;
use torca_crypto::{CryptoProvider, ProtectedSecretStore};
use torca_foundation::{OpaqueId, Timestamp};
use torca_messaging::{Message, MessageId, MessageRepository};
use torca_peer_protocol::HandshakeSigner;
use torca_runtime::{AttachmentSendRequest, AttachmentView};

/// Attachment adapter with separate SQLCipher control/projection connections. Transfer and user
/// controls operate on durable attachment rows; the UI projection no longer needs full message
/// history just to list attachment state.
pub struct AttachmentControlAdapter<R, M, S, K, C, P> {
    transfer: AttachmentTransfer<R, M, S, K, C, P>,
    control: SqlCipherAttachmentStore,
    projection: SqlCipherAttachmentProjection,
}
impl<R, M, S, K, C, P> AttachmentControlAdapter<R, M, S, K, C, P> {
    pub const fn new(
        transfer: AttachmentTransfer<R, M, S, K, C, P>,
        control: SqlCipherAttachmentStore,
        projection: SqlCipherAttachmentProjection,
    ) -> Self {
        Self { transfer, control, projection }
    }

    fn projection_snapshot(&self) -> Result<Vec<AttachmentView>, CommunicationError> {
        self.projection.list().map_err(|_| CommunicationError::Attachment).map(|rows| {
            rows.into_iter()
                .map(|row| AttachmentView {
                    id: row.id,
                    message_id: row.message_id,
                    name: row.name,
                    media_type: row.media_type,
                    size: row.size,
                    status: row.status,
                    offset: row.offset,
                    attempt_count: row.attempt_count,
                    updated_at_ms: row.updated_at_ms,
                    direction: row.direction,
                    last_error_code: row.last_error_code,
                })
                .collect()
        })
    }
}

impl<R, M, S, K, C, P> AttachmentRuntime for AttachmentControlAdapter<R, M, S, K, C, P>
where
    R: ContactRepository + ConversationRepository + PeerCredentialRepository + Send + 'static,
    M: MessageRepository + Send + 'static,
    S: ContactRepository + PeerCredentialRepository + Send + 'static,
    K: HandshakeSigner + Send + 'static,
    C: CryptoProvider + Send + 'static,
    P: ProtectedSecretStore + Send + 'static,
{
    fn prepare_outgoing(
        &mut self,
        request: &AttachmentSendRequest,
        now: Timestamp,
    ) -> Result<(), CommunicationError> {
        let metadata =
            std::fs::metadata(&request.source_path).map_err(|_| CommunicationError::Attachment)?;
        if !metadata.is_file()
            || metadata.len() != request.size
            || request.size == 0
            || request.size > MAX_ATTACHMENT_BYTES
        {
            return Err(CommunicationError::Attachment);
        }
        let preview = match &request.preview_source_path {
            Some(path) => {
                let preview = std::fs::read(path).map_err(|_| CommunicationError::Attachment)?;
                if preview.is_empty() || preview.len() > MAX_ATTACHMENT_PREVIEW {
                    return Err(CommunicationError::Attachment);
                }
                Some(AttachmentPreviewFrame {
                    media_type: MediaType::new("image/jpeg")
                        .map_err(|_| CommunicationError::Attachment)?,
                    bytes: preview,
                })
            }
            None => None,
        };
        let attachment = Attachment::prepare(
            AttachmentId::from_opaque(request.attachment_id),
            MessageId::from_opaque(request.message_id),
            AttachmentName::new(request.name.clone())
                .map_err(|_| CommunicationError::Attachment)?,
            MediaType::new(request.media_type.clone())
                .map_err(|_| CommunicationError::Attachment)?,
            request.size,
            now,
        )
        .map_err(|_| CommunicationError::Attachment)?;
        let source =
            File::open(&request.source_path).map_err(|_| CommunicationError::Attachment)?;
        self.transfer
            .prepare_outgoing_reader(
                attachment,
                torca_conversations::ConversationId::from_opaque(request.conversation_id),
                BufReader::new(source),
                preview,
                now,
            )
            .map(|_| ())
            .map_err(|_| CommunicationError::Attachment)
    }

    fn retry(&mut self, attachment_id: OpaqueId, now: Timestamp) -> Result<(), CommunicationError> {
        let id = AttachmentId::from_opaque(attachment_id);
        let mut attachment = self
            .control
            .get(id)
            .map_err(|_| CommunicationError::Attachment)?
            .ok_or(CommunicationError::Attachment)?;
        match attachment.status() {
            AttachmentStatus::Failed => {
                self.transfer.forget_outgoing(id);
                attachment.begin_transfer(now).map_err(|_| CommunicationError::Attachment)?;
                self.control.update(attachment).map_err(|_| CommunicationError::Attachment)
            }
            AttachmentStatus::Queued | AttachmentStatus::Transferring => Ok(()),
            _ => Err(CommunicationError::Attachment),
        }
    }

    fn cancel(
        &mut self,
        attachment_id: OpaqueId,
        now: Timestamp,
    ) -> Result<(), CommunicationError> {
        let id = AttachmentId::from_opaque(attachment_id);
        let mut attachment = self
            .control
            .get(id)
            .map_err(|_| CommunicationError::Attachment)?
            .ok_or(CommunicationError::Attachment)?;
        self.transfer.cancel_outgoing(id);
        attachment.cancel(now).map_err(|_| CommunicationError::Attachment)?;
        self.control.update(attachment).map_err(|_| CommunicationError::Attachment)
    }

    fn snapshot(&self, _messages: &[Message]) -> Result<Vec<AttachmentView>, CommunicationError> {
        self.projection_snapshot()
    }

    fn snapshot_projection(&self) -> Result<Option<Vec<AttachmentView>>, CommunicationError> {
        self.projection_snapshot().map(Some)
    }

    fn process_inbound(
        &mut self,
        envelope: InboundEnvelope,
        now: Timestamp,
    ) -> Result<(), CommunicationError> {
        self.transfer
            .process_inbound(peer_envelope(&envelope), now)
            .map(|_| ())
            .map_err(map_attachment_error)
    }

    fn maintenance_outgoing(
        &mut self,
        messages: &[Message],
        now: Timestamp,
        limit: usize,
    ) -> Result<(), CommunicationError> {
        self.transfer
            .maintenance_outgoing(messages, now, limit)
            .map(|_| ())
            .map_err(map_attachment_error)
    }

    fn shutdown(&mut self) {}
}

fn map_attachment_error(error: AttachmentTransferError) -> CommunicationError {
    let stage = match error {
        AttachmentTransferError::PeerAckTimeout => AttachmentFailureStage::AckTimeout,
        AttachmentTransferError::Peer => AttachmentFailureStage::PeerUnavailable,
        AttachmentTransferError::DigestMismatch => AttachmentFailureStage::Integrity,
        AttachmentTransferError::Storage | AttachmentTransferError::Io => {
            AttachmentFailureStage::Storage
        }
        AttachmentTransferError::Relationship
        | AttachmentTransferError::Message
        | AttachmentTransferError::InboundMessagePending => AttachmentFailureStage::Dependency,
        AttachmentTransferError::Protocol | AttachmentTransferError::OffsetMismatch => {
            AttachmentFailureStage::Protocol
        }
        _ => AttachmentFailureStage::Unknown,
    };
    CommunicationError::AttachmentStage(stage)
}
