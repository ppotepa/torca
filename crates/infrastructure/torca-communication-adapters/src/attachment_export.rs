use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use sha2::{Digest, Sha256};
use torca_attachment_sqlite::SqlCipherAttachmentStore;
use torca_attachments::{AttachmentId, AttachmentRepository, AttachmentStatus};
use torca_communication_driver::{AttachmentExportRuntime, CommunicationError};
use torca_contacts::PeerCredentialRepository;
use torca_conversations::ConversationRepository;
use torca_crypto::{
    Ciphertext, ManagedPeerSecrets, Nonce, ProtectedSecretStore, RustCryptoProvider,
};
use torca_file_storage::{BlobStore, FileBlobStore};
use torca_messaging::MessageRepository;
use torca_storage_sqlite::{SqlCipherMessageStore, SqlCipherStore};

const CACHE_AAD_LABEL: &[u8] = b"TORCA-ATTACHMENT-CACHE-V1";
const PREVIEW_AAD_LABEL: &[u8] = b"TORCA-ATTACHMENT-PREVIEW-V1";
const FINAL_CHUNK_AAD_LABEL: &[u8] = b"TORCA-ATTACHMENT-FINAL-CHUNK-V1";
const FINAL_MANIFEST: &[u8] = b"TORCA-ATTACHMENT-CHUNKS-V1";
const NONCE_BYTES: usize = 24;
// "Open" creates a plaintext hand-off for the OS. Keep that exposure window short; explicit
// Save As destinations are user-owned and never participate in this cleanup namespace.
const TEMP_EXPORT_MAX_AGE: Duration = Duration::from_secs(30 * 60);

pub struct AttachmentExportAdapter<P> {
    relationships: SqlCipherStore,
    messages: SqlCipherMessageStore,
    metadata: SqlCipherAttachmentStore,
    cache: FileBlobStore,
    staging_root: PathBuf,
    secrets: ManagedPeerSecrets<RustCryptoProvider, P>,
}

impl<P> AttachmentExportAdapter<P> {
    pub const fn new(
        relationships: SqlCipherStore,
        messages: SqlCipherMessageStore,
        metadata: SqlCipherAttachmentStore,
        cache: FileBlobStore,
        staging_root: PathBuf,
        secrets: ManagedPeerSecrets<RustCryptoProvider, P>,
    ) -> Self {
        Self { relationships, messages, metadata, cache, staging_root, secrets }
    }
}

impl<P> AttachmentExportRuntime for AttachmentExportAdapter<P>
where
    P: ProtectedSecretStore + Send + 'static,
{
    fn export_attachment(
        &mut self,
        id: AttachmentId,
        destination: PathBuf,
    ) -> Result<(), CommunicationError> {
        let attachment = self
            .metadata
            .get(id)
            .map_err(|_| CommunicationError::Attachment)?
            .ok_or(CommunicationError::Attachment)?;
        if attachment.status() != AttachmentStatus::Available {
            return Err(CommunicationError::Attachment);
        }
        let state = self
            .metadata
            .transfer_state(id)
            .map_err(|_| CommunicationError::Attachment)?
            .ok_or(CommunicationError::Attachment)?;
        let expected = state.content_digest.ok_or(CommunicationError::Attachment)?;
        let message = self
            .messages
            .get(attachment.message_id())
            .map_err(|_| CommunicationError::Attachment)?
            .ok_or(CommunicationError::Attachment)?;
        let conversation =
            ConversationRepository::get(&self.relationships, message.conversation_id())
                .map_err(|_| CommunicationError::Relationship)?
                .ok_or(CommunicationError::Relationship)?;
        let credential = self
            .relationships
            .credential_for_contact(conversation.contact_id())
            .map_err(|_| CommunicationError::Relationship)?
            .ok_or(CommunicationError::Relationship)?;
        let stored = self.cache.read(id).map_err(|_| CommunicationError::Attachment)?;
        if let Some(size) = parse_final_manifest(&stored) {
            if size != attachment.size() {
                return Err(CommunicationError::Attachment);
            }
            return self.export_chunked_cache(
                id,
                size,
                credential.secret_handle(),
                expected,
                destination,
            );
        }
        if stored.len() <= NONCE_BYTES {
            return Err(CommunicationError::Attachment);
        }
        let nonce =
            Nonce(stored[..NONCE_BYTES].try_into().map_err(|_| CommunicationError::Attachment)?);
        let mut aad = Vec::with_capacity(CACHE_AAD_LABEL.len() + 16);
        aad.extend_from_slice(CACHE_AAD_LABEL);
        aad.extend_from_slice(id.to_opaque().as_bytes());
        let plaintext = self
            .secrets
            .open_peer_payload(
                credential.secret_handle(),
                nonce,
                &aad,
                &Ciphertext(stored[NONCE_BYTES..].to_vec()),
            )
            .map_err(|_| CommunicationError::Attachment)?;
        if u64::try_from(plaintext.len()).ok() != Some(attachment.size())
            || <[u8; 32]>::from(Sha256::digest(&plaintext)) != expected
        {
            return Err(CommunicationError::Attachment);
        }
        let parent = destination.parent().ok_or(CommunicationError::Attachment)?;
        if !parent.is_dir() {
            return Err(CommunicationError::Attachment);
        }

        if destination
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(is_controlled_open_export_name)
        {
            cleanup_stale_controlled_exports(parent, SystemTime::now(), TEMP_EXPORT_MAX_AGE);
        }

        let temporary = parent.join(format!(".torca-export-{id}.tmp"));
        let mut file = fs::File::create(&temporary).map_err(|_| CommunicationError::Attachment)?;
        file.write_all(&plaintext).map_err(|_| CommunicationError::Attachment)?;
        file.sync_all().map_err(|_| CommunicationError::Attachment)?;
        if destination.exists() {
            fs::remove_file(&destination).map_err(|_| CommunicationError::Attachment)?;
        }
        fs::rename(temporary, destination).map_err(|_| CommunicationError::Attachment)
    }

    fn export_chunked_cache(
        &self,
        id: AttachmentId,
        size: u64,
        handle: torca_foundation::OpaqueId,
        expected: [u8; 32],
        destination: PathBuf,
    ) -> Result<(), CommunicationError> {
        let parent = destination.parent().ok_or(CommunicationError::Attachment)?;
        if !parent.is_dir() {
            return Err(CommunicationError::Attachment);
        }
        if destination
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(is_controlled_open_export_name)
        {
            cleanup_stale_controlled_exports(parent, SystemTime::now(), TEMP_EXPORT_MAX_AGE);
        }
        let temporary = parent.join(format!(".torca-export-{id}.tmp"));
        let result = (|| -> Result<(), CommunicationError> {
            let mut file = fs::File::create(&temporary).map_err(|_| CommunicationError::Attachment)?;
            let mut digest = Sha256::new();
            let mut offset = 0_u64;
            while offset < size {
                let path = self
                    .staging_root
                    .join("final")
                    .join(id.to_string())
                    .join(format!("{offset:020}.chunk"));
                let stored = fs::read(path).map_err(|_| CommunicationError::Attachment)?;
                if stored.len() <= NONCE_BYTES {
                    return Err(CommunicationError::Attachment);
                }
                let nonce = Nonce(
                    stored[..NONCE_BYTES]
                        .try_into()
                        .map_err(|_| CommunicationError::Attachment)?,
                );
                let mut chunk = self
                    .secrets
                    .open_peer_payload(
                        handle,
                        nonce,
                        &final_chunk_aad(id, offset),
                        &Ciphertext(stored[NONCE_BYTES..].to_vec()),
                    )
                    .map_err(|_| CommunicationError::Attachment)?;
                if chunk.is_empty() || chunk.len() > torca_attachment_protocol::MAX_ATTACHMENT_CHUNK {
                    chunk.fill(0);
                    return Err(CommunicationError::Attachment);
                }
                let len = match u64::try_from(chunk.len()) {
                    Ok(value) => value,
                    Err(_) => {
                        chunk.fill(0);
                        return Err(CommunicationError::Attachment);
                    }
                };
                offset = match offset.checked_add(len) {
                    Some(value) => value,
                    None => {
                        chunk.fill(0);
                        return Err(CommunicationError::Attachment);
                    }
                };
                if offset > size {
                    chunk.fill(0);
                    return Err(CommunicationError::Attachment);
                }
                digest.update(&chunk);
                let write_result = file.write_all(&chunk).map_err(|_| CommunicationError::Attachment);
                chunk.fill(0);
                write_result?;
            }
            if offset != size || <[u8; 32]>::from(digest.finalize()) != expected {
                return Err(CommunicationError::Attachment);
            }
            file.sync_all().map_err(|_| CommunicationError::Attachment)?;
            if destination.exists() {
                fs::remove_file(&destination).map_err(|_| CommunicationError::Attachment)?;
            }
            fs::rename(&temporary, &destination).map_err(|_| CommunicationError::Attachment)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }

    fn export_attachment_preview(
        &mut self,
        id: AttachmentId,
        destination: PathBuf,
    ) -> Result<(), CommunicationError> {
        let attachment = self
            .metadata
            .get(id)
            .map_err(|_| CommunicationError::Attachment)?
            .ok_or(CommunicationError::Attachment)?;
        let message = self
            .messages
            .get(attachment.message_id())
            .map_err(|_| CommunicationError::Attachment)?
            .ok_or(CommunicationError::Attachment)?;
        let conversation =
            ConversationRepository::get(&self.relationships, message.conversation_id())
                .map_err(|_| CommunicationError::Relationship)?
                .ok_or(CommunicationError::Relationship)?;
        let credential = self
            .relationships
            .credential_for_contact(conversation.contact_id())
            .map_err(|_| CommunicationError::Relationship)?
            .ok_or(CommunicationError::Relationship)?;
        let stored =
            self.cache.read(preview_blob_id(id)).map_err(|_| CommunicationError::Attachment)?;
        if stored.len() <= NONCE_BYTES {
            return Err(CommunicationError::Attachment);
        }
        let nonce =
            Nonce(stored[..NONCE_BYTES].try_into().map_err(|_| CommunicationError::Attachment)?);
        let plaintext = self
            .secrets
            .open_peer_payload(
                credential.secret_handle(),
                nonce,
                &preview_aad(id),
                &Ciphertext(stored[NONCE_BYTES..].to_vec()),
            )
            .map_err(|_| CommunicationError::Attachment)?;
        let media_length = plaintext
            .get(..2)
            .and_then(|bytes| bytes.try_into().ok())
            .map(u16::from_be_bytes)
            .map(usize::from)
            .ok_or(CommunicationError::Attachment)?;
        let payload = plaintext
            .get(2_usize.checked_add(media_length).ok_or(CommunicationError::Attachment)?..)
            .filter(|bytes| !bytes.is_empty())
            .ok_or(CommunicationError::Attachment)?;
        let parent = destination.parent().ok_or(CommunicationError::Attachment)?;
        if !parent.is_dir() {
            return Err(CommunicationError::Attachment);
        }
        let temporary = parent.join(format!(".torca-preview-{id}.tmp"));
        let mut file = fs::File::create(&temporary).map_err(|_| CommunicationError::Attachment)?;
        file.write_all(payload).map_err(|_| CommunicationError::Attachment)?;
        file.sync_all().map_err(|_| CommunicationError::Attachment)?;
        if destination.exists() {
            fs::remove_file(&destination).map_err(|_| CommunicationError::Attachment)?;
        }
        fs::rename(temporary, destination).map_err(|_| CommunicationError::Attachment)
    }
}

fn preview_blob_id(id: AttachmentId) -> AttachmentId {
    let mut hash = Sha256::new();
    hash.update(b"TORCA-ATTACHMENT-PREVIEW-BLOB-V1");
    hash.update(id.to_opaque().as_bytes());
    let digest = hash.finalize();
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    AttachmentId::from_opaque(torca_foundation::OpaqueId::from_bytes(bytes))
}

fn preview_aad(id: AttachmentId) -> Vec<u8> {
    let mut aad = Vec::with_capacity(PREVIEW_AAD_LABEL.len() + 16);
    aad.extend_from_slice(PREVIEW_AAD_LABEL);
    aad.extend_from_slice(id.to_opaque().as_bytes());
    aad
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

fn cleanup_stale_controlled_exports(parent: &Path, now: SystemTime, max_age: Duration) {
    let Ok(entries) = fs::read_dir(parent) else { return };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else { continue };
        if !file_type.is_file() {
            continue;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if !is_controlled_open_export_name(name) {
            continue;
        }
        let Ok(metadata) = entry.metadata() else { continue };
        let Ok(modified) = metadata.modified() else { continue };
        let Ok(age) = now.duration_since(modified) else { continue };
        if age >= max_age {
            let _ = fs::remove_file(entry.path());
        }
    }
}

fn is_controlled_open_export_name(name: &str) -> bool {
    let Some(rest) = name.strip_prefix("torca-") else { return false };
    if rest.len() < 32 {
        return false;
    }
    let (id, suffix) = rest.split_at(32);
    if !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return false;
    }
    if suffix.is_empty() {
        return true;
    }
    let Some(extension) = suffix.strip_prefix('.') else { return false };
    (1..=10).contains(&extension.len())
        && extension.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::is_controlled_open_export_name;

    #[test]
    fn controlled_open_export_names_are_strictly_bounded() {
        assert!(is_controlled_open_export_name("torca-0123456789abcdef0123456789abcdef"));
        assert!(is_controlled_open_export_name("torca-0123456789abcdef0123456789abcdef.pdf"));
        assert!(!is_controlled_open_export_name("torca-short.pdf"));
        assert!(!is_controlled_open_export_name("torca-0123456789abcdef0123456789abcdef.tar.gz"));
        assert!(!is_controlled_open_export_name("report-0123456789abcdef0123456789abcdef.pdf"));
    }
}
