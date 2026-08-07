use std::fs::File;
use std::io::Read;

use torca_attachment_sqlite::SqlCipherAttachmentStore;
use torca_attachment_transfer::AttachmentTransfer;
use torca_attachments::{
    Attachment, AttachmentId, AttachmentName, AttachmentRepository, AttachmentStatus,
    MAX_ATTACHMENT_BYTES, MediaType,
};
use torca_communication_driver::{AttachmentRuntime, CommunicationError};
use torca_contacts::{ContactRepository, PeerCredentialRepository};
use torca_conversations::ConversationRepository;
use torca_crypto::{CryptoProvider, ProtectedSecretStore};
use torca_foundation::{OpaqueId, Timestamp};
use torca_messaging::{Message, MessageId, MessageRepository};
use torca_peer_link::InboundPeerEnvelope;
use torca_peer_protocol::HandshakeSigner;
use torca_runtime_host::{AttachmentSendRequest, AttachmentView};

/// Attachment adapter with a separate SQLCipher control connection. Transfer and user controls
/// still operate on the same durable attachment rows and the same process-owned PeerLink.
pub struct AttachmentControlAdapter<R, M, S, K, C, P> {
    transfer: AttachmentTransfer<R, M, S, K, C, P>,
    control: SqlCipherAttachmentStore,
}
impl<R, M, S, K, C, P> AttachmentControlAdapter<R, M, S, K, C, P> {
    pub const fn new(
        transfer: AttachmentTransfer<R, M, S, K, C, P>,
        control: SqlCipherAttachmentStore,
    ) -> Self {
        Self { transfer, control }
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
        let metadata = std::fs::metadata(&request.source_path)
            .map_err(|_| CommunicationError::Attachment)?;
        if !metadata.is_file()
            || metadata.len() != request.size
            || request.size == 0
            || request.size > MAX_ATTACHMENT_BYTES
        {
            return Err(CommunicationError::Attachment);
        }
        let maximum = usize::try_from(MAX_ATTACHMENT_BYTES)
            .map_err(|_| CommunicationError::Attachment)?;
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
            AttachmentName::new(request.name.clone()).map_err(|_| CommunicationError::Attachment)?,
            MediaType::new(request.media_type.clone()).map_err(|_| CommunicationError::Attachment)?,
            request.size,
            now,
        ).map_err(|_| CommunicationError::Attachment)?;
        let result = self.transfer.prepare_outgoing(attachment, &bytes, now)
            .map(|_| ())
            .map_err(|_| CommunicationError::Attachment);
        bytes.fill(0);
        result
    }

    fn retry(&mut self, attachment_id: OpaqueId, now: Timestamp) -> Result<(), CommunicationError> {
        let id = AttachmentId::from_opaque(attachment_id);
        let mut attachment = self.control.get(id)
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

    fn cancel(&mut self, attachment_id: OpaqueId, now: Timestamp) -> Result<(), CommunicationError> {
        let id = AttachmentId::from_opaque(attachment_id);
        let mut attachment = self.control.get(id)
            .map_err(|_| CommunicationError::Attachment)?
            .ok_or(CommunicationError::Attachment)?;
        attachment.cancel(now).map_err(|_| CommunicationError::Attachment)?;
        self.control.update(attachment).map_err(|_| CommunicationError::Attachment)
    }

    fn snapshot(&self, messages: &[Message]) -> Result<Vec<AttachmentView>, CommunicationError> {
        let mut views = Vec::new();
        for message in messages {
            for attachment in self.control.for_message(message.id())
                .map_err(|_| CommunicationError::Attachment)?
            {
                let offset = self.control.transfer_state(attachment.id())
                    .map_err(|_| CommunicationError::Attachment)?
                    .map_or(0, |state| state.offset);
                views.push(AttachmentView {
                    id: attachment.id().to_opaque(),
                    message_id: attachment.message_id().to_opaque(),
                    name: attachment.name().as_str().to_owned(),
                    media_type: attachment.media_type().as_str().to_owned(),
                    size: attachment.size(),
                    status: format!("{:?}", attachment.status()).to_lowercase(),
                    offset,
                });
            }
        }
        Ok(views)
    }

    fn process_inbound(
        &mut self,
        envelope: InboundPeerEnvelope,
        now: Timestamp,
    ) -> Result<(), CommunicationError> {
        self.transfer.process_inbound(envelope, now)
            .map(|_| ())
            .map_err(|_| CommunicationError::Attachment)
    }

    fn maintenance_outgoing(
        &mut self,
        messages: &[Message],
        now: Timestamp,
        limit: usize,
    ) -> Result<(), CommunicationError> {
        self.transfer.maintenance_outgoing(messages, now, limit)
            .map(|_| ())
            .map_err(|_| CommunicationError::Attachment)
    }

    fn shutdown(&mut self) {}
}
