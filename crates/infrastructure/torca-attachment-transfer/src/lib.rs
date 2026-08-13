//! Resumable attachment transfer over the single authenticated PeerLink.
//!
//! Source and received attachment bytes are encrypted at rest with the existing protected pairwise
//! peer secret. Incoming chunks are written atomically before SQL progress advances, making ACK
//! loss/process death safe without a second message transport.

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use sha2::{Digest, Sha256};
use torca_attachment_protocol::{
    AttachmentCancelFrame, AttachmentChunkFrame, AttachmentCodec, AttachmentCompleteFrame, AttachmentFrame,
    AttachmentMetadataFrame, AttachmentPreviewFrame, AttachmentResumeFrame, MAX_ATTACHMENT_CHUNK,
};
use torca_attachment_sqlite::SqlCipherAttachmentStore;
use torca_attachments::{
    Attachment, AttachmentError, AttachmentId, AttachmentRepository, AttachmentStatus,
};
use torca_contacts::{
    Contact, ContactId, ContactRepository, PeerCredential, PeerCredentialRepository,
};
use torca_conversations::{ConversationId, ConversationRepository};
use torca_crypto::{Ciphertext, CryptoProvider, ManagedPeerSecrets, Nonce, ProtectedSecretStore};
use torca_file_storage::{BlobStore, FileBlobStore};
use torca_foundation::{ErrorCode, OpaqueId, Timestamp};
use torca_messaging::{Message, MessageDirection, MessageId, MessageRepository};
use torca_peer_link::{InboundPeerEnvelope, PeerLinkError};
use torca_peer_protocol::{AckStatus, HandshakeSigner};
use torca_peer_shared::SharedPeerLink;

pub const ATTACHMENT_MESSAGE_KIND: u16 = 3;
const NONCE_BYTES: usize = 24;
const PEER_AAD_LABEL: &[u8] = b"TORCA-PEER-DATA-V1";
const CACHE_AAD_LABEL: &[u8] = b"TORCA-ATTACHMENT-CACHE-V1";
const STAGING_AAD_LABEL: &[u8] = b"TORCA-ATTACHMENT-STAGING-V1";
const OUTGOING_STAGING_AAD_LABEL: &[u8] = b"TORCA-ATTACHMENT-OUTGOING-STAGING-V1";
const PREVIEW_AAD_LABEL: &[u8] = b"TORCA-ATTACHMENT-PREVIEW-V1";
const FINAL_CHUNK_AAD_LABEL: &[u8] = b"TORCA-ATTACHMENT-FINAL-CHUNK-V1";
const FINAL_MANIFEST: &[u8] = b"TORCA-ATTACHMENT-CHUNKS-V1";
const RETRY_MAX_DELAY: Duration = Duration::from_secs(60);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachmentTransferError {
    Relationship,
    Message,
    InboundMessagePending,
    Attachment,
    Storage,
    Crypto,
    Peer,
    PeerAckTimeout,
    Protocol,
    InvalidState,
    DigestMismatch,
    OffsetMismatch,
    Io,
    Clock,
}
impl fmt::Display for AttachmentTransferError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for AttachmentTransferError {}

#[must_use]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AttachmentTransferReport {
    pub attempted: usize,
    pub chunks_sent: usize,
    pub completed: usize,
    pub failed: usize,
}

#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InboundAttachmentResult {
    Accepted,
    Duplicate,
    Completed,
}

pub struct AttachmentTransfer<R, M, S, K, C, P> {
    relationships: R,
    messages: M,
    link: SharedPeerLink<S, K>,
    secrets: ManagedPeerSecrets<C, P>,
    metadata: SqlCipherAttachmentStore,
    cache: FileBlobStore,
    staging_root: PathBuf,
    local_identity_id: OpaqueId,
    ack_timeout: Duration,
    pending_outgoing: BTreeMap<AttachmentId, PendingOutgoingFrame>,
    metadata_acked: BTreeSet<AttachmentId>,
    cancel_confirmed: BTreeSet<AttachmentId>,
}

#[derive(Clone, Copy)]
struct PendingOutgoingFrame {
    contact_id: ContactId,
    envelope_id: OpaqueId,
    sent_at: Timestamp,
    phase: OutgoingFramePhase,
}

#[derive(Clone, Copy)]
enum OutgoingFramePhase {
    Metadata,
    Chunk { next_offset: u64, digest: [u8; 32] },
    Complete,
    Cancel,
}

impl<R, M, S, K, C, P> AttachmentTransfer<R, M, S, K, C, P>
where
    R: ContactRepository + ConversationRepository + PeerCredentialRepository,
    M: MessageRepository,
    S: ContactRepository + PeerCredentialRepository,
    K: HandshakeSigner,
    C: CryptoProvider,
    P: ProtectedSecretStore,
{
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        relationships: R,
        messages: M,
        link: SharedPeerLink<S, K>,
        secrets: ManagedPeerSecrets<C, P>,
        metadata: SqlCipherAttachmentStore,
        cache: FileBlobStore,
        staging_root: impl Into<PathBuf>,
        local_identity_id: OpaqueId,
        ack_timeout: Duration,
    ) -> Result<Self, AttachmentTransferError> {
        let staging_root = staging_root.into();
        fs::create_dir_all(&staging_root).map_err(|_| AttachmentTransferError::Io)?;
        Ok(Self {
            relationships,
            messages,
            link,
            secrets,
            metadata,
            cache,
            staging_root,
            local_identity_id,
            ack_timeout,
            pending_outgoing: BTreeMap::new(),
            metadata_acked: BTreeSet::new(),
            cancel_confirmed: BTreeSet::new(),
        })
    }

    /// Copies a selected source into the app-private encrypted cache and makes transfer state
    /// durable before network work begins.
    pub fn prepare_outgoing(
        &mut self,
        mut attachment: Attachment,
        conversation_id: ConversationId,
        plaintext: &[u8],
        preview: Option<AttachmentPreviewFrame>,
        at: Timestamp,
    ) -> Result<[u8; 32], AttachmentTransferError> {
        if attachment.status() != AttachmentStatus::Prepared
            || usize::try_from(attachment.size()).ok() != Some(plaintext.len())
        {
            return Err(AttachmentTransferError::InvalidState);
        }
        self.prepare_outgoing_reader(
            attachment,
            conversation_id,
            std::io::Cursor::new(plaintext),
            preview,
            at,
        )
    }

    /// Stages an attachment from a reader without loading the source blob into
    /// the communication adapter's heap.  Each source chunk is authenticated
    /// and committed before the next one is read, so a picker/content-provider
    /// stream can be closed as soon as staging finishes while retry continues
    /// from the durable app-owned chunks.
    pub fn prepare_outgoing_reader<T: Read>(
        &mut self,
        mut attachment: Attachment,
        conversation_id: ConversationId,
        mut source: T,
        preview: Option<AttachmentPreviewFrame>,
        at: Timestamp,
    ) -> Result<[u8; 32], AttachmentTransferError> {
        if attachment.status() != AttachmentStatus::Prepared || attachment.size() == 0 {
            return Err(AttachmentTransferError::InvalidState);
        }
        // The attachment is staged before its companion text message is
        // committed.  Resolving the recipient through that future message
        // made every new attachment fail with `Message` on the first send.
        // The command already carries the existing conversation, which is
        // the authoritative owner of the recipient relationship.
        let conversation = ConversationRepository::get(&self.relationships, conversation_id)
            .map_err(|_| AttachmentTransferError::Relationship)?
            .ok_or(AttachmentTransferError::Relationship)?;
        let contact = ContactRepository::get(&self.relationships, conversation.contact_id())
            .map_err(|_| AttachmentTransferError::Relationship)?
            .ok_or(AttachmentTransferError::Relationship)?;
        let credential = self.credential(contact.id())?;
        attachment.begin_encryption(at).map_err(map_attachment)?;
        // Promote the durable staging chunks into the final encrypted cache.
        // Each record is independently authenticated, so retries and export
        // never need to materialize the complete attachment in memory.
        let staging_result = self.stage_outgoing_reader(
            credential.secret_handle(),
            attachment.id(),
            attachment.size(),
            &mut source,
        );
        let digest = match staging_result {
            Ok(digest) => digest,
            Err(error) => {
                self.remove_outgoing_staging(attachment.id());
                self.remove_final_chunks(attachment.id());
                return Err(error);
            }
        };
        if let Err(error) = self.promote_outgoing_chunks(
            credential.secret_handle(),
            attachment.id(),
            attachment.size(),
        ) {
            self.remove_outgoing_staging(attachment.id());
            self.remove_final_chunks(attachment.id());
            return Err(error);
        }
        if let Err(error) = attachment.mark_queued(at) {
            self.remove_outgoing_staging(attachment.id());
            self.remove_final_chunks(attachment.id());
            let _ = self.cache.remove(attachment.id());
            return Err(map_attachment(error));
        }
        if let Err(error) = self.metadata.insert(attachment.clone()) {
            self.remove_outgoing_staging(attachment.id());
            self.remove_final_chunks(attachment.id());
            let _ = self.cache.remove(attachment.id());
            return Err(map_attachment(error));
        }
        if let Err(error) = self
            .metadata
            .update_transfer_progress(attachment.id(), 0, Some(digest), at)
        {
            self.remove_outgoing_staging(attachment.id());
            self.remove_final_chunks(attachment.id());
            let _ = self.cache.remove(attachment.id());
            return Err(map_attachment(error));
        }
        if let Some(preview) = preview {
            if let Err(error) =
                self.store_preview(credential.secret_handle(), attachment.id(), &preview)
            {
                self.remove_outgoing_staging(attachment.id());
                self.remove_final_chunks(attachment.id());
                let _ = self.cache.remove(attachment.id());
                return Err(error);
            }
        }
        Ok(digest)
    }

    /// Sends at most one chunk for each eligible attachment, keeping each runtime tick bounded.
    pub fn maintenance_outgoing(
        &mut self,
        messages: &[Message],
        now: Timestamp,
        max_attachments: usize,
    ) -> Result<AttachmentTransferReport, AttachmentTransferError> {
        let mut report = AttachmentTransferReport::default();
        for message in
            messages.iter().filter(|message| message.direction() == MessageDirection::Outbound)
        {
            if report.attempted >= max_attachments {
                break;
            }
            let attachments = self.metadata.for_message(message.id()).map_err(map_attachment)?;
            for attachment in attachments {
                if report.attempted >= max_attachments {
                    break;
                }
                if !matches!(
                    attachment.status(),
                    AttachmentStatus::Queued
                        | AttachmentStatus::Failed
                        | AttachmentStatus::Transferring
                        | AttachmentStatus::Cancelled
                ) {
                    continue;
                }
                if attachment.status() == AttachmentStatus::Failed && !retry_due(&attachment, now) {
                    continue;
                }
                report.attempted += 1;
                let attachment_id = attachment.id();
                match self.advance_outgoing(attachment, now) {
                    Ok(AdvanceOutcome::Waiting) => {}
                    Ok(AdvanceOutcome::Chunk) => report.chunks_sent += 1,
                    Ok(AdvanceOutcome::Completed) => report.completed += 1,
                    Err(error) => {
                        // A failed peer/frame attempt used to be reported only
                        // in memory. That left the durable row in
                        // `Transferring`, so maintenance retried it on every
                        // tick with no backoff and no visible attempt count.
                        self.record_outgoing_failure(attachment_id, now, &error)?;
                        report.failed += 1;
                    }
                }
            }
        }
        Ok(report)
    }

    /// Processes exactly one attachment envelope selected by the central communication dispatcher.
    /// The protocol ACK is sent only after staging/progress/final-cache durability requirements are
    /// met for the frame.
    pub fn process_inbound(
        &mut self,
        envelope: InboundPeerEnvelope,
        now: Timestamp,
    ) -> Result<InboundAttachmentResult, AttachmentTransferError> {
        if envelope.message_kind != ATTACHMENT_MESSAGE_KIND {
            return Err(AttachmentTransferError::Protocol);
        }
        let contact = ContactRepository::get(&self.relationships, envelope.contact_id)
            .map_err(|_| AttachmentTransferError::Relationship)?
            .ok_or(AttachmentTransferError::Relationship)?;
        let credential = self.credential(contact.id())?;
        let plaintext = self.open_wire(
            credential.secret_handle(),
            envelope.envelope_id,
            contact.remote_identity().identity_id().to_opaque(),
            &envelope.ciphertext,
        )?;
        let frame =
            AttachmentCodec::decode(&plaintext).map_err(|_| AttachmentTransferError::Protocol)?;
        let outcome = match frame {
            AttachmentFrame::Metadata(metadata) => {
                self.accept_metadata(contact.id(), metadata, now)?
            }
            AttachmentFrame::Chunk(chunk) => {
                self.accept_chunk(contact.id(), credential, chunk, now)?
            }
            AttachmentFrame::Complete(complete) => {
                self.accept_complete(contact.id(), credential, complete, now)?
            }
            AttachmentFrame::Cancel(cancel) => self.accept_cancel(cancel, now)?,
            AttachmentFrame::Resume(AttachmentResumeFrame { .. }) => {
                InboundAttachmentResult::Duplicate
            }
        };
        let ack = match outcome {
            InboundAttachmentResult::Duplicate => AckStatus::Duplicate,
            InboundAttachmentResult::Accepted | InboundAttachmentResult::Completed => {
                AckStatus::Accepted
            }
        };
        self.link.send_ack(contact.id(), envelope.envelope_id, ack).map_err(map_peer)?;
        Ok(outcome)
    }

    pub fn attachment_repository(&self) -> &SqlCipherAttachmentStore {
        &self.metadata
    }

    pub fn forget_outgoing(&mut self, attachment_id: AttachmentId) {
        self.pending_outgoing.remove(&attachment_id);
        self.metadata_acked.remove(&attachment_id);
        self.cancel_confirmed.remove(&attachment_id);
    }

    /// Requests an idempotent remote cancellation. The durable attachment row
    /// is transitioned to `Cancelled` by the control adapter immediately
    /// afterwards; maintenance observes that state and retries the wire frame
    /// until the peer acknowledges it.
    pub fn cancel_outgoing(&mut self, attachment_id: AttachmentId) {
        self.forget_outgoing(attachment_id);
    }

    fn advance_outgoing(
        &mut self,
        mut attachment: Attachment,
        now: Timestamp,
    ) -> Result<AdvanceOutcome, AttachmentTransferError> {
        if let Some(pending) = self.pending_outgoing.get(&attachment.id()).copied() {
            match self
                .link
                .poll_envelope_ack(pending.contact_id, pending.envelope_id)
                .map_err(map_peer)
            {
                Ok(Some(_)) => {
                    self.pending_outgoing.remove(&attachment.id());
                    return match pending.phase {
                        OutgoingFramePhase::Metadata => {
                            self.metadata_acked.insert(attachment.id());
                            Ok(AdvanceOutcome::Waiting)
                        }
                        OutgoingFramePhase::Chunk { next_offset, digest } => {
                            self.metadata
                                .update_transfer_progress(
                                    attachment.id(),
                                    next_offset,
                                    Some(digest),
                                    now,
                                )
                                .map_err(map_attachment)?;
                            Ok(AdvanceOutcome::Chunk)
                        }
                        OutgoingFramePhase::Complete => {
                            attachment.mark_available(now).map_err(map_attachment)?;
                            self.metadata.update(attachment.clone()).map_err(map_attachment)?;
                            self.metadata_acked.remove(&attachment.id());
                            self.remove_outgoing_staging(attachment.id());
                            Ok(AdvanceOutcome::Completed)
                        }
                        OutgoingFramePhase::Cancel => {
                            self.metadata_acked.remove(&attachment.id());
                            self.remove_outgoing_staging(attachment.id());
                            let _ = self.cache.remove(attachment.id());
                            self.remove_final_chunks(attachment.id());
                            let _ = self.cache.remove(preview_blob_id(attachment.id()));
                            self.cancel_confirmed.insert(attachment.id());
                            Ok(AdvanceOutcome::Waiting)
                        }
                    };
                }
                Ok(None) => {
                    if now
                        .duration_since(pending.sent_at)
                        .is_some_and(|elapsed| elapsed >= self.ack_timeout)
                    {
                        self.pending_outgoing.remove(&attachment.id());
                        return self.fail_outgoing(
                            attachment,
                            now,
                            AttachmentTransferError::PeerAckTimeout,
                        );
                    }
                    return Ok(AdvanceOutcome::Waiting);
                }
                Err(error) => {
                    self.pending_outgoing.remove(&attachment.id());
                    return self.fail_outgoing(attachment, now, error);
                }
            }
        }
        if attachment.status() == AttachmentStatus::Cancelled {
            if self.cancel_confirmed.contains(&attachment.id()) {
                return Ok(AdvanceOutcome::Waiting);
            }
            let contact = self.contact_for_message(attachment.message_id())?;
            let credential = self.credential(contact.id())?;
            self.start_frame(
                attachment.id(),
                &contact,
                &credential,
                stable_frame_id(attachment.id(), 5, 0),
                AttachmentFrame::Cancel(AttachmentCancelFrame {
                    attachment_id: attachment.id(),
                }),
                OutgoingFramePhase::Cancel,
                now,
            )?;
            return Ok(AdvanceOutcome::Waiting);
        }
        if matches!(attachment.status(), AttachmentStatus::Queued | AttachmentStatus::Failed) {
            attachment.begin_transfer(now).map_err(map_attachment)?;
            self.metadata.update(attachment.clone()).map_err(map_attachment)?;
        }
        // Persist `Transferring` before resolving the peer.  All subsequent
        // failures can then transition this exact attempt to `Failed` and use
        // the normal durable retry schedule.
        let contact = self.contact_for_message(attachment.message_id())?;
        let credential = self.credential(contact.id())?;
        let state = self
            .metadata
            .transfer_state(attachment.id())
            .map_err(map_attachment)?
            .ok_or(AttachmentTransferError::InvalidState)?;
        let digest = state.content_digest.ok_or(AttachmentTransferError::InvalidState)?;
        if state.offset == 0 && !self.metadata_acked.contains(&attachment.id()) {
            let preview = self.load_preview(credential.secret_handle(), attachment.id())?;
            let frame = AttachmentFrame::Metadata(AttachmentMetadataFrame {
                attachment_id: attachment.id(),
                message_id: attachment.message_id().to_opaque(),
                name: attachment.name().clone(),
                media_type: attachment.media_type().clone(),
                size: attachment.size(),
                digest,
                preview,
            });
            self.start_frame(
                attachment.id(),
                &contact,
                &credential,
                stable_frame_id(attachment.id(), 1, 0),
                frame,
                OutgoingFramePhase::Metadata,
                now,
            )?;
            return Ok(AdvanceOutcome::Waiting);
        }

        if state.offset < attachment.size() {
            let bytes = self.load_outgoing_chunk(
                credential.secret_handle(),
                attachment.id(),
                state.offset,
                attachment.size(),
            )?;
            let chunk_len = u64::try_from(bytes.len())
                .map_err(|_| AttachmentTransferError::OffsetMismatch)?;
            let next = state
                .offset
                .checked_add(chunk_len)
                .ok_or(AttachmentTransferError::OffsetMismatch)?;
            if bytes.is_empty() || next > attachment.size() {
                return self.fail_outgoing(attachment, now, AttachmentTransferError::OffsetMismatch);
            }
            let frame = AttachmentFrame::Chunk(AttachmentChunkFrame {
                attachment_id: attachment.id(),
                offset: state.offset,
                bytes,
            });
            self.start_frame(
                attachment.id(),
                &contact,
                &credential,
                stable_frame_id(attachment.id(), 2, state.offset),
                frame,
                OutgoingFramePhase::Chunk { next_offset: next, digest },
                now,
            )?;
            return Ok(AdvanceOutcome::Waiting);
        }

        self.start_frame(
            attachment.id(),
            &contact,
            &credential,
            stable_frame_id(attachment.id(), 4, attachment.size()),
            AttachmentFrame::Complete(AttachmentCompleteFrame {
                attachment_id: attachment.id(),
                digest,
            }),
            OutgoingFramePhase::Complete,
            now,
        )?;
        Ok(AdvanceOutcome::Waiting)
    }

    fn fail_outgoing<T>(
        &mut self,
        _attachment: Attachment,
        _now: Timestamp,
        error: AttachmentTransferError,
    ) -> Result<T, AttachmentTransferError> {
        // `maintain_outgoing` is the sole durable failure owner.  Recording
        // here as well caused one transport error to increment attempts
        // twice, shortened the retry window and made a healthy peer look
        // unstable in the UI.
        Err(error)
    }

    fn record_outgoing_failure(
        &mut self,
        attachment_id: AttachmentId,
        now: Timestamp,
        error: &AttachmentTransferError,
    ) -> Result<(), AttachmentTransferError> {
        let Some(mut attachment) = self.metadata.get(attachment_id).map_err(map_attachment)? else {
            return Ok(());
        };
        if attachment.status() == AttachmentStatus::Transferring {
            attachment
                .mark_failed(now, ErrorCode::new(transfer_error_code(&error)))
                .map_err(map_attachment)?;
            self.metadata.update(attachment).map_err(map_attachment)?;
        }
        Ok(())
    }

    fn start_frame(
        &mut self,
        attachment_id: AttachmentId,
        contact: &Contact,
        credential: &PeerCredential,
        envelope_id: OpaqueId,
        frame: AttachmentFrame,
        phase: OutgoingFramePhase,
        now: Timestamp,
    ) -> Result<(), AttachmentTransferError> {
        let plaintext =
            AttachmentCodec::encode(&frame).map_err(|_| AttachmentTransferError::Protocol)?;
        let encrypted = self.seal_wire(
            credential.secret_handle(),
            envelope_id,
            contact.remote_identity().identity_id().to_opaque(),
            &plaintext,
        )?;
        self.link
            .send_envelope(contact.id(), envelope_id, ATTACHMENT_MESSAGE_KIND, encrypted)
            .map_err(map_peer)?;
        self.pending_outgoing.insert(
            attachment_id,
            PendingOutgoingFrame { contact_id: contact.id(), envelope_id, sent_at: now, phase },
        );
        Ok(())
    }

    fn accept_metadata(
        &mut self,
        contact_id: ContactId,
        metadata: AttachmentMetadataFrame,
        now: Timestamp,
    ) -> Result<InboundAttachmentResult, AttachmentTransferError> {
        let message_id = MessageId::from_opaque(metadata.message_id);
        let message = self
            .messages
            .get(message_id)
            .map_err(|_| AttachmentTransferError::Message)?
            .ok_or(AttachmentTransferError::InboundMessagePending)?;
        let conversation =
            ConversationRepository::get(&self.relationships, message.conversation_id())
                .map_err(|_| AttachmentTransferError::Relationship)?
                .ok_or(AttachmentTransferError::Relationship)?;
        if conversation.contact_id() != contact_id {
            return Err(AttachmentTransferError::Relationship);
        }
        if let Some(existing) = self.metadata.get(metadata.attachment_id).map_err(map_attachment)? {
            if existing.message_id() == message_id
                && existing.name() == &metadata.name
                && existing.media_type() == &metadata.media_type
                && existing.size() == metadata.size
            {
                return Ok(InboundAttachmentResult::Duplicate);
            }
            return Err(AttachmentTransferError::InvalidState);
        }

        if let Some(preview) = metadata.preview.as_ref() {
            let credential = self.credential(contact_id)?;
            self.store_preview(credential.secret_handle(), metadata.attachment_id, preview)?;
        }
        let mut attachment = Attachment::prepare(
            metadata.attachment_id,
            message_id,
            metadata.name,
            metadata.media_type,
            metadata.size,
            now,
        )
        .map_err(map_attachment)?;
        attachment.begin_encryption(now).map_err(map_attachment)?;
        attachment.mark_queued(now).map_err(map_attachment)?;
        let _ = attachment.begin_transfer(now).map_err(map_attachment)?;
        self.metadata.insert(attachment).map_err(map_attachment)?;
        self.metadata
            .update_transfer_progress(metadata.attachment_id, 0, Some(metadata.digest), now)
            .map_err(map_attachment)?;
        Ok(InboundAttachmentResult::Accepted)
    }

    fn accept_chunk(
        &mut self,
        _contact_id: ContactId,
        credential: PeerCredential,
        chunk: AttachmentChunkFrame,
        now: Timestamp,
    ) -> Result<InboundAttachmentResult, AttachmentTransferError> {
        let attachment = self
            .metadata
            .get(chunk.attachment_id)
            .map_err(map_attachment)?
            .ok_or(AttachmentTransferError::Attachment)?;
        if attachment.status() != AttachmentStatus::Transferring {
            return Err(AttachmentTransferError::InvalidState);
        }
        let state = self
            .metadata
            .transfer_state(chunk.attachment_id)
            .map_err(map_attachment)?
            .ok_or(AttachmentTransferError::InvalidState)?;
        let chunk_len = u64::try_from(chunk.bytes.len())
            .map_err(|_| AttachmentTransferError::OffsetMismatch)?;
        let end =
            chunk.offset.checked_add(chunk_len).ok_or(AttachmentTransferError::OffsetMismatch)?;
        if end > attachment.size() {
            return Err(AttachmentTransferError::OffsetMismatch);
        }
        if end <= state.offset {
            return Ok(InboundAttachmentResult::Duplicate);
        }
        if chunk.offset != state.offset {
            return Err(AttachmentTransferError::OffsetMismatch);
        }
        self.store_staging_chunk(
            credential.secret_handle(),
            chunk.attachment_id,
            chunk.offset,
            &chunk.bytes,
        )?;
        self.metadata
            .update_transfer_progress(chunk.attachment_id, end, state.content_digest, now)
            .map_err(map_attachment)?;
        Ok(InboundAttachmentResult::Accepted)
    }

    fn accept_complete(
        &mut self,
        _contact_id: ContactId,
        credential: PeerCredential,
        complete: AttachmentCompleteFrame,
        now: Timestamp,
    ) -> Result<InboundAttachmentResult, AttachmentTransferError> {
        let mut attachment = self
            .metadata
            .get(complete.attachment_id)
            .map_err(map_attachment)?
            .ok_or(AttachmentTransferError::Attachment)?;
        if attachment.status() == AttachmentStatus::Available {
            return Ok(InboundAttachmentResult::Duplicate);
        }
        let state = self
            .metadata
            .transfer_state(complete.attachment_id)
            .map_err(map_attachment)?
            .ok_or(AttachmentTransferError::InvalidState)?;
        if state.offset != attachment.size() || state.content_digest != Some(complete.digest) {
            return Err(AttachmentTransferError::InvalidState);
        }
        let digest = self.promote_inbound_chunks(
            credential.secret_handle(),
            complete.attachment_id,
            attachment.size(),
        )?;
        if digest != complete.digest {
            self.remove_final_chunks(complete.attachment_id);
            return Err(AttachmentTransferError::DigestMismatch);
        }
        attachment.mark_available(now).map_err(map_attachment)?;
        self.metadata.update(attachment).map_err(map_attachment)?;
        self.remove_staging(complete.attachment_id);
        Ok(InboundAttachmentResult::Completed)
    }

    fn accept_cancel(
        &mut self,
        cancel: AttachmentCancelFrame,
        now: Timestamp,
    ) -> Result<InboundAttachmentResult, AttachmentTransferError> {
        let Some(mut attachment) =
            self.metadata.get(cancel.attachment_id).map_err(map_attachment)?
        else {
            // Cancellation is idempotent. A peer may retry after local orphan
            // cleanup or after this device already acknowledged the request.
            self.remove_staging(cancel.attachment_id);
            self.remove_final_chunks(cancel.attachment_id);
            let _ = self.cache.remove(cancel.attachment_id);
            let _ = self.cache.remove(preview_blob_id(cancel.attachment_id));
            return Ok(InboundAttachmentResult::Duplicate);
        };
        if attachment.status() == AttachmentStatus::Available {
            // Never allow a late cancel to remove verified content.
            return Ok(InboundAttachmentResult::Duplicate);
        }
        if attachment.status() != AttachmentStatus::Cancelled {
            attachment.cancel(now).map_err(map_attachment)?;
            self.metadata.update(attachment).map_err(map_attachment)?;
        }
        self.remove_staging(cancel.attachment_id);
        self.remove_final_chunks(cancel.attachment_id);
        let _ = self.cache.remove(cancel.attachment_id);
        let _ = self.cache.remove(preview_blob_id(cancel.attachment_id));
        Ok(InboundAttachmentResult::Accepted)
    }

    fn contact_for_message(
        &self,
        message_id: MessageId,
    ) -> Result<Contact, AttachmentTransferError> {
        let message = self
            .messages
            .get(message_id)
            .map_err(|_| AttachmentTransferError::Message)?
            .ok_or(AttachmentTransferError::Message)?;
        let conversation =
            ConversationRepository::get(&self.relationships, message.conversation_id())
                .map_err(|_| AttachmentTransferError::Relationship)?
                .ok_or(AttachmentTransferError::Relationship)?;
        ContactRepository::get(&self.relationships, conversation.contact_id())
            .map_err(|_| AttachmentTransferError::Relationship)?
            .ok_or(AttachmentTransferError::Relationship)
    }

    fn credential(&self, contact_id: ContactId) -> Result<PeerCredential, AttachmentTransferError> {
        self.relationships
            .credential_for_contact(contact_id)
            .map_err(|_| AttachmentTransferError::Relationship)?
            .ok_or(AttachmentTransferError::Relationship)
    }

    fn seal_wire(
        &mut self,
        handle: OpaqueId,
        envelope_id: OpaqueId,
        remote_identity: OpaqueId,
        plaintext: &[u8],
    ) -> Result<Vec<u8>, AttachmentTransferError> {
        let nonce = self.secrets.peer_nonce().map_err(|_| AttachmentTransferError::Crypto)?;
        let aad = peer_aad(envelope_id, self.local_identity_id, remote_identity);
        let ciphertext = self
            .secrets
            .seal_peer_payload(handle, nonce, &aad, plaintext)
            .map_err(|_| AttachmentTransferError::Crypto)?;
        Ok(pack_ciphertext(nonce, ciphertext))
    }

    fn open_wire(
        &self,
        handle: OpaqueId,
        envelope_id: OpaqueId,
        remote_identity: OpaqueId,
        stored: &[u8],
    ) -> Result<Vec<u8>, AttachmentTransferError> {
        let (nonce, ciphertext) = unpack_ciphertext(stored)?;
        let aad = peer_aad(envelope_id, self.local_identity_id, remote_identity);
        self.secrets
            .open_peer_payload(handle, nonce, &aad, &ciphertext)
            .map_err(|_| AttachmentTransferError::Crypto)
    }

    fn store_final_manifest(
        &mut self,
        attachment_id: AttachmentId,
        size: u64,
    ) -> Result<(), AttachmentTransferError> {
        let mut manifest = Vec::with_capacity(FINAL_MANIFEST.len() + 8);
        manifest.extend_from_slice(FINAL_MANIFEST);
        manifest.extend_from_slice(&size.to_be_bytes());
        self.cache
            .put_atomic(attachment_id, &manifest)
            .map_err(|_| AttachmentTransferError::Storage)
    }

    fn store_final_chunk(
        &self,
        handle: OpaqueId,
        attachment_id: AttachmentId,
        offset: u64,
        plaintext: &[u8],
    ) -> Result<(), AttachmentTransferError> {
        let nonce = self.secrets.peer_nonce().map_err(|_| AttachmentTransferError::Crypto)?;
        let ciphertext = self
            .secrets
            .seal_peer_payload(handle, nonce, &final_chunk_aad(attachment_id, offset), plaintext)
            .map_err(|_| AttachmentTransferError::Crypto)?;
        let directory = self.final_chunk_directory(attachment_id);
        fs::create_dir_all(&directory).map_err(|_| AttachmentTransferError::Io)?;
        let target = directory.join(format!("{offset:020}.chunk"));
        let temporary = directory.join(format!(".{offset:020}.tmp"));
        let bytes = pack_ciphertext(nonce, ciphertext);
        let mut file = File::create(&temporary).map_err(|_| AttachmentTransferError::Io)?;
        file.write_all(&bytes).map_err(|_| AttachmentTransferError::Io)?;
        file.sync_all().map_err(|_| AttachmentTransferError::Io)?;
        fs::rename(&temporary, &target).map_err(|_| AttachmentTransferError::Io)?;
        sync_directory(&directory)
    }

    fn promote_outgoing_chunks(
        &mut self,
        handle: OpaqueId,
        attachment_id: AttachmentId,
        size: u64,
    ) -> Result<(), AttachmentTransferError> {
        let mut offset = 0_u64;
        while offset < size {
            let chunk = self.load_outgoing_chunk(handle, attachment_id, offset, size)?;
            if chunk.is_empty() {
                return Err(AttachmentTransferError::OffsetMismatch);
            }
            let len = match u64::try_from(chunk.len()) {
                Ok(value) => value,
                Err(_) => {
                    let mut chunk = chunk;
                    chunk.fill(0);
                    return Err(AttachmentTransferError::OffsetMismatch);
                }
            };
            let result = self.store_final_chunk(handle, attachment_id, offset, &chunk);
            let mut chunk = chunk;
            chunk.fill(0);
            result?;
            offset = match offset.checked_add(len) {
                Some(value) => value,
                None => return Err(AttachmentTransferError::OffsetMismatch),
            };
        }
        if offset != size {
            return Err(AttachmentTransferError::OffsetMismatch);
        }
        self.store_final_manifest(attachment_id, size)
    }

    fn promote_inbound_chunks(
        &mut self,
        handle: OpaqueId,
        attachment_id: AttachmentId,
        size: u64,
    ) -> Result<[u8; 32], AttachmentTransferError> {
        let directory = self.staging_directory(attachment_id);
        let mut digest = Sha256::new();
        let mut offset = 0_u64;
        while offset < size {
            let path = directory.join(format!("{offset:020}.chunk"));
            let stored = fs::read(path).map_err(|_| AttachmentTransferError::Io)?;
            let (nonce, ciphertext) = unpack_ciphertext(&stored)?;
            let mut chunk = self
                .secrets
                .open_peer_payload(handle, nonce, &staging_aad(attachment_id, offset), &ciphertext)
                .map_err(|_| AttachmentTransferError::Crypto)?;
            if chunk.is_empty() || chunk.len() > MAX_ATTACHMENT_CHUNK {
                chunk.fill(0);
                return Err(AttachmentTransferError::InvalidState);
            }
            let len = match u64::try_from(chunk.len()) {
                Ok(value) => value,
                Err(_) => {
                    chunk.fill(0);
                    return Err(AttachmentTransferError::InvalidState);
                }
            };
            digest.update(&chunk);
            let result = self.store_final_chunk(handle, attachment_id, offset, &chunk);
            chunk.fill(0);
            result?;
            offset = match offset.checked_add(len) {
                Some(value) => value,
                None => return Err(AttachmentTransferError::InvalidState),
            };
        }
        if offset != size {
            return Err(AttachmentTransferError::OffsetMismatch);
        }
        self.store_final_manifest(attachment_id, size)?;
        Ok(digest.finalize().into())
    }

    fn load_final_chunk(
        &self,
        handle: OpaqueId,
        attachment_id: AttachmentId,
        offset: u64,
    ) -> Result<Vec<u8>, AttachmentTransferError> {
        let path = self.final_chunk_directory(attachment_id).join(format!("{offset:020}.chunk"));
        let stored = fs::read(path).map_err(|_| AttachmentTransferError::Io)?;
        let (nonce, ciphertext) = unpack_ciphertext(&stored)?;
        let chunk = self
            .secrets
            .open_peer_payload(handle, nonce, &final_chunk_aad(attachment_id, offset), &ciphertext)
            .map_err(|_| AttachmentTransferError::Crypto)?;
        if chunk.is_empty() || chunk.len() > MAX_ATTACHMENT_CHUNK {
            return Err(AttachmentTransferError::InvalidState);
        }
        Ok(chunk)
    }

    fn stage_outgoing_reader<T: Read>(
        &mut self,
        handle: OpaqueId,
        attachment_id: AttachmentId,
        expected_size: u64,
        source: &mut T,
    ) -> Result<[u8; 32], AttachmentTransferError> {
        let directory = self.outgoing_staging_directory(attachment_id);
        fs::create_dir_all(&directory).map_err(|_| AttachmentTransferError::Io)?;
        let mut digest = Sha256::new();
        let mut offset = 0_u64;
        let mut buffer = vec![0_u8; MAX_ATTACHMENT_CHUNK];
        let result = (|| -> Result<[u8; 32], AttachmentTransferError> {
            loop {
                let read = source.read(&mut buffer).map_err(|_| AttachmentTransferError::Io)?;
                if read == 0 {
                    break;
                }
                let read_u64 = u64::try_from(read).map_err(|_| AttachmentTransferError::OffsetMismatch)?;
                let end = offset
                    .checked_add(read_u64)
                    .ok_or(AttachmentTransferError::OffsetMismatch)?;
                if end > expected_size {
                    self.remove_outgoing_staging(attachment_id);
                    return Err(AttachmentTransferError::OffsetMismatch);
                }
                digest.update(&buffer[..read]);
                if let Err(error) = self.store_outgoing_chunk(handle, attachment_id, offset, &buffer[..read]) {
                    self.remove_outgoing_staging(attachment_id);
                    return Err(error);
                }
                offset = end;
            }
            if offset != expected_size {
                self.remove_outgoing_staging(attachment_id);
                return Err(AttachmentTransferError::OffsetMismatch);
            }
            if let Err(error) = sync_directory(&directory) {
                self.remove_outgoing_staging(attachment_id);
                return Err(error);
            }
            Ok(digest.finalize().into())
        })();
        buffer.fill(0);
        result
    }

    fn store_outgoing_chunk(
        &mut self,
        handle: OpaqueId,
        attachment_id: AttachmentId,
        offset: u64,
        plaintext: &[u8],
    ) -> Result<(), AttachmentTransferError> {
        let nonce = self.secrets.peer_nonce().map_err(|_| AttachmentTransferError::Crypto)?;
        let ciphertext = self
            .secrets
            .seal_peer_payload(handle, nonce, &outgoing_staging_aad(attachment_id, offset), plaintext)
            .map_err(|_| AttachmentTransferError::Crypto)?;
        let directory = self.outgoing_staging_directory(attachment_id);
        let target = directory.join(format!("{offset:020}.chunk"));
        let temporary = directory.join(format!(".{offset:020}.tmp"));
        let bytes = pack_ciphertext(nonce, ciphertext);
        let mut file = File::create(&temporary).map_err(|_| AttachmentTransferError::Io)?;
        file.write_all(&bytes).map_err(|_| AttachmentTransferError::Io)?;
        file.sync_all().map_err(|_| AttachmentTransferError::Io)?;
        fs::rename(&temporary, &target).map_err(|_| AttachmentTransferError::Io)?;
        Ok(())
    }

    fn load_outgoing_chunk(
        &self,
        handle: OpaqueId,
        attachment_id: AttachmentId,
        offset: u64,
        size: u64,
    ) -> Result<Vec<u8>, AttachmentTransferError> {
        let path = self.outgoing_staging_directory(attachment_id).join(format!("{offset:020}.chunk"));
        match fs::read(path) {
            Ok(stored) => {
                let (nonce, ciphertext) = unpack_ciphertext(&stored)?;
                self.secrets
                    .open_peer_payload(handle, nonce, &outgoing_staging_aad(attachment_id, offset), &ciphertext)
                    .map_err(|_| AttachmentTransferError::Crypto)
            }
            // Existing jobs created by a previous application version do not
            // have a chunk directory. Keep them transferable through the
            // bounded compatibility path; all new jobs use final chunks.
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let stored = self
                    .cache
                    .read(attachment_id)
                    .map_err(|_| AttachmentTransferError::Storage)?;
                if let Some(final_size) = parse_final_manifest(&stored) {
                    if final_size != size {
                        return Err(AttachmentTransferError::DigestMismatch);
                    }
                    return self.load_final_chunk(handle, attachment_id, offset);
                }
                let (nonce, ciphertext) = unpack_ciphertext(&stored)?;
                let mut plaintext = self
                    .secrets
                    .open_peer_payload(handle, nonce, &cache_aad(attachment_id), &ciphertext)
                    .map_err(|_| AttachmentTransferError::Crypto)?;
                if u64::try_from(plaintext.len()).ok() != Some(size) {
                    plaintext.fill(0);
                    return Err(AttachmentTransferError::DigestMismatch);
                }
                let start = match usize::try_from(offset) {
                    Ok(value) => value,
                    Err(_) => {
                        plaintext.fill(0);
                        return Err(AttachmentTransferError::OffsetMismatch);
                    }
                };
                let end = start.saturating_add(MAX_ATTACHMENT_CHUNK).min(plaintext.len());
                let result = plaintext
                    .get(start..end)
                    .map(ToOwned::to_owned)
                    .ok_or(AttachmentTransferError::OffsetMismatch);
                plaintext.fill(0);
                result
            }
            Err(_) => Err(AttachmentTransferError::Io),
        }
    }

    fn store_preview(
        &mut self,
        handle: OpaqueId,
        attachment_id: AttachmentId,
        preview: &AttachmentPreviewFrame,
    ) -> Result<(), AttachmentTransferError> {
        let media = preview.media_type.as_str().as_bytes();
        let media_length =
            u16::try_from(media.len()).map_err(|_| AttachmentTransferError::Storage)?;
        let mut plaintext = Vec::with_capacity(2 + media.len() + preview.bytes.len());
        plaintext.extend_from_slice(&media_length.to_be_bytes());
        plaintext.extend_from_slice(media);
        plaintext.extend_from_slice(&preview.bytes);
        let nonce = self.secrets.peer_nonce().map_err(|_| AttachmentTransferError::Crypto)?;
        let ciphertext = self
            .secrets
            .seal_peer_payload(handle, nonce, &preview_aad(attachment_id), &plaintext)
            .map_err(|_| AttachmentTransferError::Crypto)?;
        let stored = pack_ciphertext(nonce, ciphertext);
        self.cache
            .put_atomic(preview_blob_id(attachment_id), &stored)
            .map_err(|_| AttachmentTransferError::Storage)
    }

    fn load_preview(
        &self,
        handle: OpaqueId,
        attachment_id: AttachmentId,
    ) -> Result<Option<AttachmentPreviewFrame>, AttachmentTransferError> {
        let stored = match self.cache.read(preview_blob_id(attachment_id)) {
            Ok(value) => value,
            Err(torca_file_storage::BlobStoreError::NotFound) => return Ok(None),
            Err(_) => return Err(AttachmentTransferError::Storage),
        };
        let (nonce, ciphertext) = unpack_ciphertext(&stored)?;
        let plaintext = self
            .secrets
            .open_peer_payload(handle, nonce, &preview_aad(attachment_id), &ciphertext)
            .map_err(|_| AttachmentTransferError::Crypto)?;
        let media_length = plaintext
            .get(..2)
            .and_then(|bytes| bytes.try_into().ok())
            .map(u16::from_be_bytes)
            .map(usize::from)
            .ok_or(AttachmentTransferError::Storage)?;
        let media_end =
            2_usize.checked_add(media_length).ok_or(AttachmentTransferError::Storage)?;
        let media = plaintext.get(2..media_end).ok_or(AttachmentTransferError::Storage)?;
        let bytes = plaintext.get(media_end..).ok_or(AttachmentTransferError::Storage)?;
        let media_type = std::str::from_utf8(media)
            .ok()
            .and_then(|value| torca_attachments::MediaType::new(value).ok())
            .ok_or(AttachmentTransferError::Storage)?;
        if bytes.is_empty() || bytes.len() > torca_attachment_protocol::MAX_ATTACHMENT_PREVIEW {
            return Err(AttachmentTransferError::Storage);
        }
        Ok(Some(AttachmentPreviewFrame { media_type, bytes: bytes.to_vec() }))
    }

    fn store_staging_chunk(
        &mut self,
        handle: OpaqueId,
        attachment_id: AttachmentId,
        offset: u64,
        plaintext: &[u8],
    ) -> Result<(), AttachmentTransferError> {
        let nonce = self.secrets.peer_nonce().map_err(|_| AttachmentTransferError::Crypto)?;
        let ciphertext = self
            .secrets
            .seal_peer_payload(handle, nonce, &staging_aad(attachment_id, offset), plaintext)
            .map_err(|_| AttachmentTransferError::Crypto)?;
        let directory = self.staging_directory(attachment_id);
        fs::create_dir_all(&directory).map_err(|_| AttachmentTransferError::Io)?;
        let target = directory.join(format!("{offset:020}.chunk"));
        let temporary = directory.join(format!(".{offset:020}.tmp"));
        let bytes = pack_ciphertext(nonce, ciphertext);
        let mut file = File::create(&temporary).map_err(|_| AttachmentTransferError::Io)?;
        file.write_all(&bytes).map_err(|_| AttachmentTransferError::Io)?;
        file.sync_all().map_err(|_| AttachmentTransferError::Io)?;
        fs::rename(&temporary, &target).map_err(|_| AttachmentTransferError::Io)?;
        sync_directory(&directory)?;
        Ok(())
    }

    fn staging_directory(&self, id: AttachmentId) -> PathBuf {
        self.staging_root.join(id.to_string())
    }

    fn outgoing_staging_directory(&self, id: AttachmentId) -> PathBuf {
        self.staging_root.join("outgoing").join(id.to_string())
    }

    fn final_chunk_directory(&self, id: AttachmentId) -> PathBuf {
        self.staging_root.join("final").join(id.to_string())
    }

    fn remove_staging(&self, id: AttachmentId) {
        let _ = fs::remove_dir_all(self.staging_directory(id));
    }

    fn remove_outgoing_staging(&self, id: AttachmentId) {
        let _ = fs::remove_dir_all(self.outgoing_staging_directory(id));
    }

    fn remove_final_chunks(&self, id: AttachmentId) {
        let _ = fs::remove_dir_all(self.final_chunk_directory(id));
    }
}

enum AdvanceOutcome {
    Waiting,
    Chunk,
    Completed,
}

fn retry_due(attachment: &Attachment, now: Timestamp) -> bool {
    let attempts = u32::try_from(attachment.attempts().len()).unwrap_or(u32::MAX);
    let exponent = attempts.saturating_sub(1).min(6);
    let delay = Duration::from_secs(1_u64 << exponent).min(RETRY_MAX_DELAY);
    now.duration_since(attachment.updated_at()).is_some_and(|elapsed| elapsed >= delay)
}

fn stable_frame_id(id: AttachmentId, kind: u8, offset: u64) -> OpaqueId {
    let mut hash = Sha256::new();
    hash.update(b"TORCA-ATTACHMENT-FRAME-V1");
    hash.update(id.to_opaque().as_bytes());
    hash.update([kind]);
    hash.update(offset.to_be_bytes());
    let digest = hash.finalize();
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    let value = OpaqueId::from_bytes(bytes);
    if value.is_nil() { OpaqueId::from_u128(1) } else { value }
}

/// Uses a domain-separated deterministic key in the same encrypted blob
/// store as the payload.  It is not exposed to UI or peers; the encryption AAD
/// below binds it back to the authoritative attachment id.
fn preview_blob_id(id: AttachmentId) -> AttachmentId {
    let mut hash = Sha256::new();
    hash.update(b"TORCA-ATTACHMENT-PREVIEW-BLOB-V1");
    hash.update(id.to_opaque().as_bytes());
    let digest = hash.finalize();
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    AttachmentId::from_opaque(OpaqueId::from_bytes(bytes))
}

fn parse_final_manifest(stored: &[u8]) -> Option<u64> {
    let size = stored.strip_prefix(FINAL_MANIFEST)?.get(..8)?;
    Some(u64::from_be_bytes(size.try_into().ok()?))
}

fn final_chunk_aad(id: AttachmentId, offset: u64) -> Vec<u8> {
    let mut aad = Vec::with_capacity(FINAL_CHUNK_AAD_LABEL.len() + 24);
    aad.extend_from_slice(FINAL_CHUNK_AAD_LABEL);
    aad.extend_from_slice(id.to_opaque().as_bytes());
    aad.extend_from_slice(&offset.to_be_bytes());
    aad
}

fn outgoing_staging_aad(id: AttachmentId, offset: u64) -> Vec<u8> {
    let mut aad = Vec::with_capacity(OUTGOING_STAGING_AAD_LABEL.len() + 16 + 8);
    aad.extend_from_slice(OUTGOING_STAGING_AAD_LABEL);
    aad.extend_from_slice(id.to_opaque().as_bytes());
    aad.extend_from_slice(&offset.to_be_bytes());
    aad
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn peer_aad(envelope_id: OpaqueId, local_identity: OpaqueId, remote_identity: OpaqueId) -> Vec<u8> {
    let (first, second) = if local_identity <= remote_identity {
        (local_identity, remote_identity)
    } else {
        (remote_identity, local_identity)
    };
    let mut aad = Vec::with_capacity(PEER_AAD_LABEL.len() + 50);
    aad.extend_from_slice(PEER_AAD_LABEL);
    aad.extend_from_slice(envelope_id.as_bytes());
    aad.extend_from_slice(&ATTACHMENT_MESSAGE_KIND.to_be_bytes());
    aad.extend_from_slice(first.as_bytes());
    aad.extend_from_slice(second.as_bytes());
    aad
}

fn cache_aad(id: AttachmentId) -> Vec<u8> {
    let mut aad = Vec::with_capacity(CACHE_AAD_LABEL.len() + 16);
    aad.extend_from_slice(CACHE_AAD_LABEL);
    aad.extend_from_slice(id.to_opaque().as_bytes());
    aad
}

fn preview_aad(id: AttachmentId) -> Vec<u8> {
    let mut aad = Vec::with_capacity(PREVIEW_AAD_LABEL.len() + 16);
    aad.extend_from_slice(PREVIEW_AAD_LABEL);
    aad.extend_from_slice(id.to_opaque().as_bytes());
    aad
}

fn staging_aad(id: AttachmentId, offset: u64) -> Vec<u8> {
    let mut aad = Vec::with_capacity(STAGING_AAD_LABEL.len() + 24);
    aad.extend_from_slice(STAGING_AAD_LABEL);
    aad.extend_from_slice(id.to_opaque().as_bytes());
    aad.extend_from_slice(&offset.to_be_bytes());
    aad
}

fn pack_ciphertext(nonce: Nonce, ciphertext: Ciphertext) -> Vec<u8> {
    let mut output = Vec::with_capacity(NONCE_BYTES + ciphertext.0.len());
    output.extend_from_slice(&nonce.0);
    output.extend_from_slice(&ciphertext.0);
    output
}

fn unpack_ciphertext(stored: &[u8]) -> Result<(Nonce, Ciphertext), AttachmentTransferError> {
    if stored.len() <= NONCE_BYTES {
        return Err(AttachmentTransferError::Crypto);
    }
    let nonce =
        Nonce(stored[..NONCE_BYTES].try_into().map_err(|_| AttachmentTransferError::Crypto)?);
    Ok((nonce, Ciphertext(stored[NONCE_BYTES..].to_vec())))
}

fn map_attachment(_: AttachmentError) -> AttachmentTransferError {
    AttachmentTransferError::Attachment
}

fn transfer_error_code(error: &AttachmentTransferError) -> &'static str {
    match error {
        AttachmentTransferError::PeerAckTimeout => "ATTACHMENT_ACK_TIMEOUT",
        AttachmentTransferError::Peer => "ATTACHMENT_PEER_UNAVAILABLE",
        AttachmentTransferError::DigestMismatch => "ATTACHMENT_INTEGRITY_FAILED",
        AttachmentTransferError::Storage | AttachmentTransferError::Io => {
            "ATTACHMENT_STORAGE_FAILED"
        }
        AttachmentTransferError::Relationship | AttachmentTransferError::Message => {
            "ATTACHMENT_DEPENDENCY_MISSING"
        }
        AttachmentTransferError::InboundMessagePending => "ATTACHMENT_MESSAGE_PENDING",
        AttachmentTransferError::Crypto => "ATTACHMENT_CRYPTO_FAILED",
        AttachmentTransferError::Protocol | AttachmentTransferError::OffsetMismatch => {
            "ATTACHMENT_PROTOCOL_FAILED"
        }
        _ => "ATTACHMENT_SEND",
    }
}
fn map_peer(error: PeerLinkError) -> AttachmentTransferError {
    match error {
        PeerLinkError::AckTimeout => AttachmentTransferError::PeerAckTimeout,
        _ => AttachmentTransferError::Peer,
    }
}

fn sync_directory(path: &Path) -> Result<(), AttachmentTransferError> {
    if std::env::consts::OS == "windows" {
        return Ok(());
    }
    File::open(path).and_then(|file| file.sync_all()).map_err(|_| AttachmentTransferError::Io)
}
