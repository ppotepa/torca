//! SQLCipher attachment metadata and resumable-transfer progress.

mod projection;
pub use projection::{
    AttachmentProjectionError, AttachmentProjectionRow, SqlCipherAttachmentProjection,
};

use std::path::Path;

use rusqlite::{OptionalExtension, params};
use torca_attachments::{
    Attachment, AttachmentAttempt, AttachmentError, AttachmentId, AttachmentName,
    AttachmentRepository, AttachmentStatus, MediaType,
};
use torca_foundation::{OpaqueId, Timestamp};
use torca_messaging::MessageId;
use torca_storage_sqlite::{
    DatabaseKey, MigrationError, SqlCipherBackend, StorageBackendError, StorageKernel,
};

const INSERT_SQL: &str = include_str!("../sql/attachment_insert.sql");
const UPDATE_SQL: &str = include_str!("../sql/attachment_update.sql");
const SELECT_SQL: &str = include_str!("../sql/attachment_select.sql");
const FOR_MESSAGE_SQL: &str = include_str!("../sql/attachment_for_message.sql");
const UPDATE_PROGRESS_SQL: &str = include_str!("../sql/attachment_progress_update.sql");

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachmentStoreOpenError {
    Backend,
    Migration,
}
impl core::fmt::Display for AttachmentStoreOpenError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for AttachmentStoreOpenError {}

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachmentTransferState {
    pub offset: u64,
    pub content_digest: Option<[u8; 32]>,
}

pub struct SqlCipherAttachmentStore {
    backend: SqlCipherBackend,
}
impl SqlCipherAttachmentStore {
    pub fn open(
        path: impl AsRef<Path>,
        key: &DatabaseKey,
    ) -> Result<Self, AttachmentStoreOpenError> {
        let backend = SqlCipherBackend::open(path, key).map_err(map_backend)?;
        Self::bootstrap(backend)
    }

    pub fn open_in_memory(key: &DatabaseKey) -> Result<Self, AttachmentStoreOpenError> {
        let backend = SqlCipherBackend::open_in_memory(key).map_err(map_backend)?;
        Self::bootstrap(backend)
    }

    fn bootstrap(backend: SqlCipherBackend) -> Result<Self, AttachmentStoreOpenError> {
        let mut kernel = StorageKernel::new(backend);
        kernel.bootstrap().map_err(map_migration)?;
        Ok(Self { backend: kernel.into_backend() })
    }

    pub fn transfer_state(
        &self,
        id: AttachmentId,
    ) -> Result<Option<AttachmentTransferState>, AttachmentError> {
        let id = id.to_opaque().into_bytes();
        self.backend
            .connection()
            .query_row(SELECT_SQL, params![id.as_slice()], |row| {
                let offset: i64 = row.get(8)?;
                let digest: Option<Vec<u8>> = row.get(9)?;
                Ok((offset, digest))
            })
            .optional()
            .map_err(|_| AttachmentError::RepositoryFailure)?
            .map(|(offset, digest)| transfer_state(offset, digest))
            .transpose()
    }

    pub fn update_transfer_progress(
        &mut self,
        id: AttachmentId,
        offset: u64,
        content_digest: Option<[u8; 32]>,
        at: Timestamp,
    ) -> Result<(), AttachmentError> {
        let attachment = self.get(id)?.ok_or(AttachmentError::NotFound)?;
        if offset > attachment.size() {
            return Err(AttachmentError::InvalidPersistedState);
        }
        let id = id.to_opaque().into_bytes();
        let offset = i64::try_from(offset).map_err(|_| AttachmentError::RepositoryFailure)?;
        let digest = content_digest.as_ref().map(<[u8; 32]>::as_slice);
        let changed = self
            .backend
            .connection()
            .execute(
                UPDATE_PROGRESS_SQL,
                params![id.as_slice(), offset, digest, at.to_unix_millis()],
            )
            .map_err(|_| AttachmentError::RepositoryFailure)?;
        if changed == 1 { Ok(()) } else { Err(AttachmentError::NotFound) }
    }
}

impl AttachmentRepository for SqlCipherAttachmentStore {
    fn insert(&mut self, attachment: Attachment) -> Result<(), AttachmentError> {
        if self.get(attachment.id())?.is_some() {
            return Err(AttachmentError::AlreadyExists);
        }
        execute_insert(&self.backend, &attachment)
    }

    fn get(&self, id: AttachmentId) -> Result<Option<Attachment>, AttachmentError> {
        let id_bytes = id.to_opaque().into_bytes();
        let row = self
            .backend
            .connection()
            .query_row(SELECT_SQL, params![id_bytes.as_slice()], |row| {
                Ok(AttachmentRow {
                    attachment_id: id_bytes.to_vec(),
                    message_id: row.get(0)?,
                    name: row.get(1)?,
                    media_type: row.get(2)?,
                    size: row.get(3)?,
                    status: row.get(4)?,
                    created_at_ms: row.get(5)?,
                    updated_at_ms: row.get(6)?,
                    attempt_count: row.get(7)?,
                })
            })
            .optional()
            .map_err(|_| AttachmentError::RepositoryFailure)?;
        row.map(AttachmentRow::into_attachment).transpose()
    }

    fn update(&mut self, attachment: Attachment) -> Result<(), AttachmentError> {
        let state = self.transfer_state(attachment.id())?.ok_or(AttachmentError::NotFound)?;
        let id = attachment.id().to_opaque().into_bytes();
        let digest = state.content_digest.as_ref().map(<[u8; 32]>::as_slice);
        let attempts = i64::try_from(attachment.attempts().len())
            .map_err(|_| AttachmentError::RepositoryFailure)?;
        let offset = i64::try_from(state.offset).map_err(|_| AttachmentError::RepositoryFailure)?;
        let changed = self
            .backend
            .connection()
            .execute(
                UPDATE_SQL,
                params![
                    id.as_slice(),
                    encode_status(attachment.status()),
                    attachment.updated_at().to_unix_millis(),
                    attempts,
                    offset,
                    digest,
                ],
            )
            .map_err(|_| AttachmentError::RepositoryFailure)?;
        if changed == 1 { Ok(()) } else { Err(AttachmentError::NotFound) }
    }

    fn for_message(&self, message_id: MessageId) -> Result<Vec<Attachment>, AttachmentError> {
        let message = message_id.to_opaque().into_bytes();
        let mut statement = self
            .backend
            .connection()
            .prepare(FOR_MESSAGE_SQL)
            .map_err(|_| AttachmentError::RepositoryFailure)?;
        let rows = statement
            .query_map(params![message.as_slice()], |row| {
                Ok(AttachmentRow {
                    attachment_id: row.get(0)?,
                    message_id: message.to_vec(),
                    name: row.get(1)?,
                    media_type: row.get(2)?,
                    size: row.get(3)?,
                    status: row.get(4)?,
                    created_at_ms: row.get(5)?,
                    updated_at_ms: row.get(6)?,
                    attempt_count: row.get(7)?,
                })
            })
            .map_err(|_| AttachmentError::RepositoryFailure)?;
        rows.map(|row| row.map_err(|_| AttachmentError::RepositoryFailure)?.into_attachment())
            .collect()
    }
}

struct AttachmentRow {
    attachment_id: Vec<u8>,
    message_id: Vec<u8>,
    name: String,
    media_type: String,
    size: i64,
    status: i64,
    created_at_ms: i64,
    updated_at_ms: i64,
    attempt_count: i64,
}
impl AttachmentRow {
    fn into_attachment(self) -> Result<Attachment, AttachmentError> {
        let id = AttachmentId::from_opaque(OpaqueId::from_bytes(fixed16(self.attachment_id)?));
        let message_id = MessageId::from_opaque(OpaqueId::from_bytes(fixed16(self.message_id)?));
        let name = AttachmentName::new(self.name)?;
        let media_type = MediaType::new(self.media_type)?;
        let size = u64::try_from(self.size).map_err(|_| AttachmentError::RepositoryFailure)?;
        let status = decode_status(self.status)?;
        let created_at = Timestamp::from_unix_millis(self.created_at_ms)
            .map_err(|_| AttachmentError::RepositoryFailure)?;
        let updated_at = Timestamp::from_unix_millis(self.updated_at_ms)
            .map_err(|_| AttachmentError::RepositoryFailure)?;
        let attempt_count =
            u32::try_from(self.attempt_count).map_err(|_| AttachmentError::RepositoryFailure)?;
        let attempts = (1..=attempt_count)
            .map(|number| AttachmentAttempt { number, at: updated_at, error_code: None })
            .collect();
        Attachment::from_persisted(
            id, message_id, name, media_type, size, status, created_at, updated_at, attempts,
        )
    }
}

fn execute_insert(
    backend: &SqlCipherBackend,
    attachment: &Attachment,
) -> Result<(), AttachmentError> {
    let id = attachment.id().to_opaque().into_bytes();
    let message = attachment.message_id().to_opaque().into_bytes();
    let size = i64::try_from(attachment.size()).map_err(|_| AttachmentError::RepositoryFailure)?;
    let attempts = i64::try_from(attachment.attempts().len())
        .map_err(|_| AttachmentError::RepositoryFailure)?;
    backend
        .connection()
        .execute(
            INSERT_SQL,
            params![
                id.as_slice(),
                message.as_slice(),
                attachment.name().as_str(),
                attachment.media_type().as_str(),
                size,
                encode_status(attachment.status()),
                attachment.created_at().to_unix_millis(),
                attachment.updated_at().to_unix_millis(),
                attempts,
                0_i64,
                Option::<&[u8]>::None,
            ],
        )
        .map_err(|_| AttachmentError::RepositoryFailure)?;
    Ok(())
}

fn transfer_state(
    offset: i64,
    digest: Option<Vec<u8>>,
) -> Result<AttachmentTransferState, AttachmentError> {
    let offset = u64::try_from(offset).map_err(|_| AttachmentError::RepositoryFailure)?;
    let content_digest = digest
        .map(|value| value.try_into().map_err(|_| AttachmentError::RepositoryFailure))
        .transpose()?;
    Ok(AttachmentTransferState { offset, content_digest })
}

const fn encode_status(status: AttachmentStatus) -> i64 {
    match status {
        AttachmentStatus::Prepared => 0,
        AttachmentStatus::Encrypting => 1,
        AttachmentStatus::Queued => 2,
        AttachmentStatus::Transferring => 3,
        AttachmentStatus::Available => 4,
        AttachmentStatus::Failed => 5,
        AttachmentStatus::Cancelled => 6,
    }
}
fn decode_status(value: i64) -> Result<AttachmentStatus, AttachmentError> {
    match value {
        0 => Ok(AttachmentStatus::Prepared),
        1 => Ok(AttachmentStatus::Encrypting),
        2 => Ok(AttachmentStatus::Queued),
        3 => Ok(AttachmentStatus::Transferring),
        4 => Ok(AttachmentStatus::Available),
        5 => Ok(AttachmentStatus::Failed),
        6 => Ok(AttachmentStatus::Cancelled),
        _ => Err(AttachmentError::RepositoryFailure),
    }
}
fn fixed16(value: Vec<u8>) -> Result<[u8; 16], AttachmentError> {
    value.try_into().map_err(|_| AttachmentError::RepositoryFailure)
}
fn map_backend(_: StorageBackendError) -> AttachmentStoreOpenError {
    AttachmentStoreOpenError::Backend
}
fn map_migration(_: MigrationError) -> AttachmentStoreOpenError {
    AttachmentStoreOpenError::Migration
}
