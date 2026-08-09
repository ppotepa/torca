//! Resumable attachment transfer over the single authenticated PeerLink.
//!
//! Source and received attachment bytes are encrypted at rest with the existing protected pairwise
//! peer secret. Incoming chunks are written atomically before SQL progress advances, making ACK
//! loss/process death safe without a second message transport.

use core::fmt;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use sha2::{Digest, Sha256};
use torca_attachment_protocol::{
    AttachmentChunkFrame, AttachmentCodec, AttachmentCompleteFrame, AttachmentFrame,
    AttachmentMetadataFrame, AttachmentResumeFrame, MAX_ATTACHMENT_CHUNK,
};
use torca_attachment_sqlite::SqlCipherAttachmentStore;
use torca_attachments::{
    Attachment, AttachmentError, AttachmentId, AttachmentRepository, AttachmentStatus,
};
use torca_contacts::{
    Contact, ContactId, ContactRepository, PeerCredential, PeerCredentialRepository,
};
use torca_conversations::ConversationRepository;
use torca_crypto::{Ciphertext, CryptoProvider, ManagedPeerSecrets, Nonce, ProtectedSecretStore};
use torca_file_storage::{BlobStore, FileBlobStore};
use torca_foundation::{ErrorCode, OpaqueId, Timestamp};
use torca_messaging::{Message, MessageDirection, MessageId, MessageRepository};
use torca_peer_link::{InboundPeerEnvelope, LinkAck, PeerLinkError};
use torca_peer_protocol::{AckStatus, HandshakeSigner};
use torca_peer_shared::SharedPeerLink;

pub const ATTACHMENT_MESSAGE_KIND: u16 = 3;
const NONCE_BYTES: usize = 24;
const PEER_AAD_LABEL: &[u8] = b"TORCA-PEER-DATA-V1";
const CACHE_AAD_LABEL: &[u8] = b"TORCA-ATTACHMENT-CACHE-V1";
const STAGING_AAD_LABEL: &[u8] = b"TORCA-ATTACHMENT-STAGING-V1";
const RETRY_MAX_DELAY: Duration = Duration::from_secs(60);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachmentTransferError {
    Relationship,
    Message,
    Attachment,
    Storage,
    Crypto,
    Peer,
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
        })
    }

    /// Copies a selected source into the app-private encrypted cache and makes transfer state
    /// durable before network work begins.
    pub fn prepare_outgoing(
        &mut self,
        mut attachment: Attachment,
        plaintext: &[u8],
        at: Timestamp,
    ) -> Result<[u8; 32], AttachmentTransferError> {
        if attachment.status() != AttachmentStatus::Prepared
            || usize::try_from(attachment.size()).ok() != Some(plaintext.len())
        {
            return Err(AttachmentTransferError::InvalidState);
        }
        let contact = self.contact_for_message(attachment.message_id())?;
        let credential = self.credential(contact.id())?;
        let digest = sha256(plaintext);
        attachment.begin_encryption(at).map_err(map_attachment)?;
        self.store_final_cache(credential.secret_handle(), attachment.id(), plaintext)?;
        attachment.mark_queued(at).map_err(map_attachment)?;
        self.metadata.insert(attachment.clone()).map_err(map_attachment)?;
        self.metadata
            .update_transfer_progress(attachment.id(), 0, Some(digest), at)
            .map_err(map_attachment)?;
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
                ) {
                    continue;
                }
                if attachment.status() == AttachmentStatus::Failed && !retry_due(&attachment, now) {
                    continue;
                }
                report.attempted += 1;
                match self.advance_outgoing(attachment, now) {
                    Ok(AdvanceOutcome::Chunk) => report.chunks_sent += 1,
                    Ok(AdvanceOutcome::Completed) => report.completed += 1,
                    Err(_) => report.failed += 1,
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

    fn advance_outgoing(
        &mut self,
        mut attachment: Attachment,
        now: Timestamp,
    ) -> Result<AdvanceOutcome, AttachmentTransferError> {
        let contact = self.contact_for_message(attachment.message_id())?;
        let credential = self.credential(contact.id())?;
        if matches!(attachment.status(), AttachmentStatus::Queued | AttachmentStatus::Failed) {
            attachment.begin_transfer(now).map_err(map_attachment)?;
            self.metadata.update(attachment.clone()).map_err(map_attachment)?;
        }
        let state = self
            .metadata
            .transfer_state(attachment.id())
            .map_err(map_attachment)?
            .ok_or(AttachmentTransferError::InvalidState)?;
        let digest = state.content_digest.ok_or(AttachmentTransferError::InvalidState)?;
        let plaintext = self.load_final_cache(credential.secret_handle(), attachment.id())?;
        if u64::try_from(plaintext.len()).ok() != Some(attachment.size())
            || sha256(&plaintext) != digest
        {
            return self.fail_outgoing(attachment, now, AttachmentTransferError::DigestMismatch);
        }

        if state.offset == 0 {
            let frame = AttachmentFrame::Metadata(AttachmentMetadataFrame {
                attachment_id: attachment.id(),
                message_id: attachment.message_id().to_opaque(),
                name: attachment.name().clone(),
                media_type: attachment.media_type().clone(),
                size: attachment.size(),
                digest,
            });
            self.send_frame(&contact, &credential, stable_frame_id(attachment.id(), 1, 0), frame)?;
        }

        if state.offset < attachment.size() {
            let start = usize::try_from(state.offset)
                .map_err(|_| AttachmentTransferError::OffsetMismatch)?;
            let end = start.saturating_add(MAX_ATTACHMENT_CHUNK).min(plaintext.len());
            let frame = AttachmentFrame::Chunk(AttachmentChunkFrame {
                attachment_id: attachment.id(),
                offset: state.offset,
                bytes: plaintext[start..end].to_vec(),
            });
            self.send_frame(
                &contact,
                &credential,
                stable_frame_id(attachment.id(), 2, state.offset),
                frame,
            )?;
            let next = u64::try_from(end).map_err(|_| AttachmentTransferError::OffsetMismatch)?;
            self.metadata
                .update_transfer_progress(attachment.id(), next, Some(digest), now)
                .map_err(map_attachment)?;
            if next < attachment.size() {
                return Ok(AdvanceOutcome::Chunk);
            }
        }

        self.send_frame(
            &contact,
            &credential,
            stable_frame_id(attachment.id(), 4, attachment.size()),
            AttachmentFrame::Complete(AttachmentCompleteFrame {
                attachment_id: attachment.id(),
                digest,
            }),
        )?;
        attachment.mark_available(now).map_err(map_attachment)?;
        self.metadata.update(attachment).map_err(map_attachment)?;
        Ok(AdvanceOutcome::Completed)
    }

    fn fail_outgoing<T>(
        &mut self,
        mut attachment: Attachment,
        now: Timestamp,
        error: AttachmentTransferError,
    ) -> Result<T, AttachmentTransferError> {
        let code = ErrorCode::new("ATTACHMENT_SEND");
        if attachment.status() == AttachmentStatus::Transferring {
            let _ = attachment.mark_failed(now, code);
            let _ = self.metadata.update(attachment);
        }
        Err(error)
    }

    fn send_frame(
        &mut self,
        contact: &Contact,
        credential: &PeerCredential,
        envelope_id: OpaqueId,
        frame: AttachmentFrame,
    ) -> Result<LinkAck, AttachmentTransferError> {
        let plaintext =
            AttachmentCodec::encode(&frame).map_err(|_| AttachmentTransferError::Protocol)?;
        let encrypted = self.seal_wire(
            credential.secret_handle(),
            envelope_id,
            contact.remote_identity().identity_id().to_opaque(),
            &plaintext,
        )?;
        self.link
            .send_and_wait_ack(
                contact.id(),
                envelope_id,
                ATTACHMENT_MESSAGE_KIND,
                encrypted,
                self.ack_timeout,
            )
            .map_err(map_peer)
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
            .ok_or(AttachmentTransferError::Message)?;
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
        let plaintext = self.load_staging_plaintext(
            credential.secret_handle(),
            complete.attachment_id,
            attachment.size(),
        )?;
        if sha256(&plaintext) != complete.digest {
            return Err(AttachmentTransferError::DigestMismatch);
        }
        self.store_final_cache(credential.secret_handle(), complete.attachment_id, &plaintext)?;
        attachment.mark_available(now).map_err(map_attachment)?;
        self.metadata.update(attachment).map_err(map_attachment)?;
        self.remove_staging(complete.attachment_id);
        Ok(InboundAttachmentResult::Completed)
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

    fn store_final_cache(
        &mut self,
        handle: OpaqueId,
        attachment_id: AttachmentId,
        plaintext: &[u8],
    ) -> Result<(), AttachmentTransferError> {
        let nonce = self.secrets.peer_nonce().map_err(|_| AttachmentTransferError::Crypto)?;
        let aad = cache_aad(attachment_id);
        let ciphertext = self
            .secrets
            .seal_peer_payload(handle, nonce, &aad, plaintext)
            .map_err(|_| AttachmentTransferError::Crypto)?;
        self.cache
            .put_atomic(attachment_id, &pack_ciphertext(nonce, ciphertext))
            .map_err(|_| AttachmentTransferError::Storage)
    }

    fn load_final_cache(
        &self,
        handle: OpaqueId,
        attachment_id: AttachmentId,
    ) -> Result<Vec<u8>, AttachmentTransferError> {
        let stored =
            self.cache.read(attachment_id).map_err(|_| AttachmentTransferError::Storage)?;
        let (nonce, ciphertext) = unpack_ciphertext(&stored)?;
        self.secrets
            .open_peer_payload(handle, nonce, &cache_aad(attachment_id), &ciphertext)
            .map_err(|_| AttachmentTransferError::Crypto)
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

    fn load_staging_plaintext(
        &self,
        handle: OpaqueId,
        attachment_id: AttachmentId,
        expected_size: u64,
    ) -> Result<Vec<u8>, AttachmentTransferError> {
        let directory = self.staging_directory(attachment_id);
        let mut offset = 0_u64;
        let mut plaintext = Vec::with_capacity(
            usize::try_from(expected_size).map_err(|_| AttachmentTransferError::InvalidState)?,
        );
        while offset < expected_size {
            let path = directory.join(format!("{offset:020}.chunk"));
            let stored = fs::read(path).map_err(|_| AttachmentTransferError::Io)?;
            let (nonce, ciphertext) = unpack_ciphertext(&stored)?;
            let chunk = self
                .secrets
                .open_peer_payload(handle, nonce, &staging_aad(attachment_id, offset), &ciphertext)
                .map_err(|_| AttachmentTransferError::Crypto)?;
            if chunk.is_empty() || chunk.len() > MAX_ATTACHMENT_CHUNK {
                return Err(AttachmentTransferError::InvalidState);
            }
            offset = offset
                .checked_add(
                    u64::try_from(chunk.len())
                        .map_err(|_| AttachmentTransferError::InvalidState)?,
                )
                .ok_or(AttachmentTransferError::InvalidState)?;
            plaintext.extend_from_slice(&chunk);
        }
        if offset != expected_size {
            return Err(AttachmentTransferError::OffsetMismatch);
        }
        Ok(plaintext)
    }

    fn staging_directory(&self, id: AttachmentId) -> PathBuf {
        self.staging_root.join(id.to_string())
    }

    fn remove_staging(&self, id: AttachmentId) {
        let _ = fs::remove_dir_all(self.staging_directory(id));
    }
}

enum AdvanceOutcome {
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
fn map_peer(_: PeerLinkError) -> AttachmentTransferError {
    AttachmentTransferError::Peer
}

#[cfg(not(windows))]
fn sync_directory(path: &Path) -> Result<(), AttachmentTransferError> {
    File::open(path).and_then(|file| file.sync_all()).map_err(|_| AttachmentTransferError::Io)
}
#[cfg(windows)]
fn sync_directory(_path: &Path) -> Result<(), AttachmentTransferError> {
    Ok(())
}
