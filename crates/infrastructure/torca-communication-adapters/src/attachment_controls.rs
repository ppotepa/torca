use std::fs::File;
use std::io::Read;

use crate::peer_envelope;
use torca_attachment_sqlite::{SqlCipherAttachmentProjection, SqlCipherAttachmentStore};
use torca_attachment_transfer::AttachmentTransfer;
use torca_attachments::{
    Attachment, AttachmentId, AttachmentName, AttachmentRepository, AttachmentStatus,
    MAX_ATTACHMENT_BYTES, MediaType,
};
use torca_communication_driver::{AttachmentRuntime, CommunicationError, InboundEnvelope};
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
        let maximum =
            usize::try_from(MAX_ATTACHMENT_BYTES).map_err(|_| CommunicationError::Attachment)?;
        let mut bytes = Vec::with_capacity(
            usize::try_from(request.size).map_err(|_| CommunicationError::Attachment)?,
        );
        File::open(&request.source_path)
            .map_err(|_| CommunicationError::Attachment)?
            .take(u64::try_from(maximum).unwrap_or(u64::MAX).saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|_| CommunicationError::Attachment)?;
        if bytes.len() > maximum || u64::try_from(bytes.len()).ok() != Some(request.size) {
            bytes.fill(0);
            return Err(CommunicationError::Attachment);
        }
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
        let result = self
            .transfer
            .prepare_outgoing(attachment, &bytes, now)
            .map(|_| ())
            .map_err(|_| CommunicationError::Attachment);
        bytes.fill(0);
        result
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
            .map_err(|_| CommunicationError::Attachment)
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
            .map_err(|_| CommunicationError::Attachment)
    }

    fn shutdown(&mut self) {}
}
