use std::fs;
use std::io::Write;
use std::path::PathBuf;

use sha2::{Digest, Sha256};
use torca_attachment_sqlite::SqlCipherAttachmentStore;
use torca_attachments::{AttachmentId, AttachmentRepository, AttachmentStatus};
use torca_communication_driver::{AttachmentExportRuntime, CommunicationError};
use torca_conversations::ConversationRepository;
use torca_crypto::{Ciphertext, ManagedPeerSecrets, Nonce, ProtectedSecretStore, RustCryptoProvider};
use torca_file_storage::{BlobStore, FileBlobStore};
use torca_messaging::MessageRepository;
use torca_storage_sqlite::{SqlCipherMessageStore, SqlCipherStore};
use torca_contacts::PeerCredentialRepository;

const CACHE_AAD_LABEL: &[u8] = b"TORCA-ATTACHMENT-CACHE-V1";
const NONCE_BYTES: usize = 24;

pub struct AttachmentExportAdapter<P> {
    relationships: SqlCipherStore,
    messages: SqlCipherMessageStore,
    metadata: SqlCipherAttachmentStore,
    cache: FileBlobStore,
    secrets: ManagedPeerSecrets<RustCryptoProvider, P>,
}

impl<P> AttachmentExportAdapter<P> {
    pub const fn new(
        relationships: SqlCipherStore,
        messages: SqlCipherMessageStore,
        metadata: SqlCipherAttachmentStore,
        cache: FileBlobStore,
        secrets: ManagedPeerSecrets<RustCryptoProvider, P>,
    ) -> Self {
        Self { relationships, messages, metadata, cache, secrets }
    }
}

impl<P> AttachmentExportRuntime for AttachmentExportAdapter<P>
where
    P: ProtectedSecretStore + Send + 'static,
{
    fn export_attachment(&mut self, id: AttachmentId, destination: PathBuf) -> Result<(), CommunicationError> {
        let attachment = self.metadata.get(id).map_err(|_| CommunicationError::Attachment)?
            .ok_or(CommunicationError::Attachment)?;
        if attachment.status() != AttachmentStatus::Available { return Err(CommunicationError::Attachment); }
        let state = self.metadata.transfer_state(id).map_err(|_| CommunicationError::Attachment)?
            .ok_or(CommunicationError::Attachment)?;
        let expected = state.content_digest.ok_or(CommunicationError::Attachment)?;
        let message = self.messages.get(attachment.message_id()).map_err(|_| CommunicationError::Attachment)?
            .ok_or(CommunicationError::Attachment)?;
        let conversation = ConversationRepository::get(&self.relationships, message.conversation_id())
            .map_err(|_| CommunicationError::Relationship)?.ok_or(CommunicationError::Relationship)?;
        let credential = self.relationships.credential_for_contact(conversation.contact_id())
            .map_err(|_| CommunicationError::Relationship)?.ok_or(CommunicationError::Relationship)?;
        let stored = self.cache.read(id).map_err(|_| CommunicationError::Attachment)?;
        if stored.len() <= NONCE_BYTES { return Err(CommunicationError::Attachment); }
        let nonce = Nonce(stored[..NONCE_BYTES].try_into().map_err(|_| CommunicationError::Attachment)?);
        let mut aad = Vec::with_capacity(CACHE_AAD_LABEL.len() + 16);
        aad.extend_from_slice(CACHE_AAD_LABEL);
        aad.extend_from_slice(id.to_opaque().as_bytes());
        let plaintext = self.secrets.open_peer_payload(
            credential.secret_handle(), nonce, &aad, &Ciphertext(stored[NONCE_BYTES..].to_vec()),
        ).map_err(|_| CommunicationError::Attachment)?;
        if u64::try_from(plaintext.len()).ok() != Some(attachment.size())
            || <[u8; 32]>::from(Sha256::digest(&plaintext)) != expected
        { return Err(CommunicationError::Attachment); }
        let parent = destination.parent().ok_or(CommunicationError::Attachment)?;
        if !parent.is_dir() { return Err(CommunicationError::Attachment); }
        let temporary = parent.join(format!(".torca-export-{id}.tmp"));
        let mut file = fs::File::create(&temporary).map_err(|_| CommunicationError::Attachment)?;
        file.write_all(&plaintext).map_err(|_| CommunicationError::Attachment)?;
        file.sync_all().map_err(|_| CommunicationError::Attachment)?;
        if destination.exists() { fs::remove_file(&destination).map_err(|_| CommunicationError::Attachment)?; }
        fs::rename(temporary, destination).map_err(|_| CommunicationError::Attachment)
    }
}
