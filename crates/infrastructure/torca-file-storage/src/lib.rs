//! Atomic encrypted attachment blob storage.

use core::fmt;
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use torca_attachments::{AttachmentId, MAX_ATTACHMENT_BYTES};
use torca_crypto::{CryptoError, CryptoProvider, Nonce, SecretKey};
use torca_foundation::OpaqueId;

/// Maximum stored encrypted blob length including authentication overhead.
pub const MAX_STORED_BLOB_BYTES: usize = MAX_ATTACHMENT_BYTES as usize + 1024;

/// Blob storage failure without exposing paths or secret data in normal display.
#[derive(Debug)]
pub enum BlobStoreError { Io(std::io::Error), NotFound, TooLarge { actual: usize }, Crypto(CryptoError), InvalidStoredBlob }
impl fmt::Display for BlobStoreError { fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result { match self { Self::Io(_) => formatter.write_str("blob storage I/O failure"), Self::NotFound => formatter.write_str("blob not found"), Self::TooLarge { actual } => write!(formatter, "blob exceeds size limit: {actual}"), Self::Crypto(error) => write!(formatter, "cryptographic failure: {error}"), Self::InvalidStoredBlob => formatter.write_str("stored blob is malformed") } } }
impl std::error::Error for BlobStoreError {}
impl From<std::io::Error> for BlobStoreError { fn from(value: std::io::Error) -> Self { Self::Io(value) } }
impl From<CryptoError> for BlobStoreError { fn from(value: CryptoError) -> Self { Self::Crypto(value) } }

/// Atomic opaque blob store.
pub trait BlobStore {
    /// Writes complete bytes atomically under an attachment ID.
    fn put_atomic(&mut self, id: AttachmentId, bytes: &[u8]) -> Result<(), BlobStoreError>;
    /// Reads complete bytes.
    fn read(&self, id: AttachmentId) -> Result<Vec<u8>, BlobStoreError>;
    /// Removes content when present.
    fn remove(&mut self, id: AttachmentId) -> Result<bool, BlobStoreError>;
    /// Returns whether content exists.
    fn exists(&self, id: AttachmentId) -> Result<bool, BlobStoreError>;
}

/// Filesystem store using write-sync-rename semantics.
pub struct FileBlobStore { root: PathBuf }
impl FileBlobStore {
    /// Creates a store and its root directory.
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, BlobStoreError> { let root = root.into(); fs::create_dir_all(&root)?; Ok(Self { root }) }
    fn path(&self, id: AttachmentId) -> PathBuf { self.root.join(format!("{}.blob", id)) }
    fn temporary_path(&self, id: AttachmentId) -> PathBuf { self.root.join(format!("{}.tmp", id)) }
}
impl BlobStore for FileBlobStore {
    fn put_atomic(&mut self, id: AttachmentId, bytes: &[u8]) -> Result<(), BlobStoreError> {
        if bytes.len() > MAX_STORED_BLOB_BYTES { return Err(BlobStoreError::TooLarge { actual: bytes.len() }); }
        let temporary = self.temporary_path(id); let final_path = self.path(id);
        let result = (|| -> Result<(), BlobStoreError> { let mut file = File::create(&temporary)?; file.write_all(bytes)?; file.sync_all()?; fs::rename(&temporary, &final_path)?; sync_directory(&self.root)?; Ok(()) })();
        if result.is_err() { let _ = fs::remove_file(&temporary); }
        result
    }
    fn read(&self, id: AttachmentId) -> Result<Vec<u8>, BlobStoreError> { let path = self.path(id); let metadata = fs::metadata(&path).map_err(|error| if error.kind() == std::io::ErrorKind::NotFound { BlobStoreError::NotFound } else { error.into() })?; let length = usize::try_from(metadata.len()).map_err(|_| BlobStoreError::TooLarge { actual: usize::MAX })?; if length > MAX_STORED_BLOB_BYTES { return Err(BlobStoreError::TooLarge { actual: length }); } let mut bytes = Vec::with_capacity(length); File::open(path)?.take(MAX_STORED_BLOB_BYTES as u64 + 1).read_to_end(&mut bytes)?; if bytes.len() > MAX_STORED_BLOB_BYTES { return Err(BlobStoreError::TooLarge { actual: bytes.len() }); } Ok(bytes) }
    fn remove(&mut self, id: AttachmentId) -> Result<bool, BlobStoreError> { match fs::remove_file(self.path(id)) { Ok(()) => { sync_directory(&self.root)?; Ok(true) }, Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false), Err(error) => Err(error.into()) } }
    fn exists(&self, id: AttachmentId) -> Result<bool, BlobStoreError> { Ok(self.path(id).try_exists()?) }
}
fn sync_directory(path: &Path) -> Result<(), std::io::Error> { File::open(path)?.sync_all() }

/// In-memory opaque blob store.
#[derive(Clone, Debug, Default)]
pub struct MemoryBlobStore { values: BTreeMap<OpaqueId, Vec<u8>> }
impl BlobStore for MemoryBlobStore {
    fn put_atomic(&mut self, id: AttachmentId, bytes: &[u8]) -> Result<(), BlobStoreError> { if bytes.len() > MAX_STORED_BLOB_BYTES { return Err(BlobStoreError::TooLarge { actual: bytes.len() }); } self.values.insert(id.to_opaque(), bytes.to_vec()); Ok(()) }
    fn read(&self, id: AttachmentId) -> Result<Vec<u8>, BlobStoreError> { self.values.get(&id.to_opaque()).cloned().ok_or(BlobStoreError::NotFound) }
    fn remove(&mut self, id: AttachmentId) -> Result<bool, BlobStoreError> { Ok(self.values.remove(&id.to_opaque()).is_some()) }
    fn exists(&self, id: AttachmentId) -> Result<bool, BlobStoreError> { Ok(self.values.contains_key(&id.to_opaque())) }
}

/// Encrypts/decrypts attachment content before it reaches a blob store.
pub struct EncryptedAttachmentStore<C, B> { crypto: C, blobs: B }
impl<C: CryptoProvider, B: BlobStore> EncryptedAttachmentStore<C, B> {
    /// Creates encrypted storage composition.
    pub const fn new(crypto: C, blobs: B) -> Self { Self { crypto, blobs } }
    /// Stores plaintext using associated metadata and a caller-owned secret key.
    pub fn store(&mut self, id: AttachmentId, key: &SecretKey, associated_data: &[u8], plaintext: &[u8]) -> Result<Nonce, BlobStoreError> {
        if plaintext.len() > MAX_ATTACHMENT_BYTES as usize { return Err(BlobStoreError::TooLarge { actual: plaintext.len() }); }
        let mut nonce_bytes = [0_u8; 24]; self.crypto.fill_random(&mut nonce_bytes)?; let nonce = Nonce(nonce_bytes); let ciphertext = self.crypto.seal(key, nonce, associated_data, plaintext)?; let mut stored = Vec::with_capacity(24 + ciphertext.0.len()); stored.extend_from_slice(&nonce.0); stored.extend_from_slice(&ciphertext.0); self.blobs.put_atomic(id, &stored)?; Ok(nonce)
    }
    /// Reads and authenticates plaintext.
    pub fn load(&self, id: AttachmentId, key: &SecretKey, associated_data: &[u8]) -> Result<Vec<u8>, BlobStoreError> { let stored = self.blobs.read(id)?; let (nonce, ciphertext) = stored.split_at_checked(24).ok_or(BlobStoreError::InvalidStoredBlob)?; let nonce = Nonce(nonce.try_into().map_err(|_| BlobStoreError::InvalidStoredBlob)?); self.crypto.open(key, nonce, associated_data, &torca_crypto::Ciphertext(ciphertext.to_vec())).map_err(Into::into) }
    /// Returns the underlying components.
    pub fn into_parts(self) -> (C, B) { (self.crypto, self.blobs) }
}
