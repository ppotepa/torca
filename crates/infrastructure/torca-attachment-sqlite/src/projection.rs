use std::path::Path;

use rusqlite::params;
use torca_foundation::OpaqueId;
use torca_storage_sqlite::{DatabaseKey, SqlCipherBackend, StorageKernel};

const LIST_SQL: &str = include_str!("../sql/attachment_list.sql");

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachmentProjectionRow {
    pub id: OpaqueId,
    pub message_id: OpaqueId,
    pub name: String,
    pub media_type: String,
    pub size: u64,
    pub status: String,
    pub offset: u64,
    pub attempt_count: u32,
    pub updated_at_ms: i64,
    pub direction: String,
    pub last_error_code: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachmentProjectionError {
    Storage,
    InvalidStoredState,
}
impl core::fmt::Display for AttachmentProjectionError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for AttachmentProjectionError {}

pub struct SqlCipherAttachmentProjection {
    backend: SqlCipherBackend,
}
impl SqlCipherAttachmentProjection {
    pub fn open(
        path: impl AsRef<Path>,
        key: &DatabaseKey,
    ) -> Result<Self, AttachmentProjectionError> {
        let backend =
            SqlCipherBackend::open(path, key).map_err(|_| AttachmentProjectionError::Storage)?;
        let mut kernel = StorageKernel::new(backend);
        kernel.bootstrap().map_err(|_| AttachmentProjectionError::Storage)?;
        Ok(Self { backend: kernel.into_backend() })
    }

    pub fn list(&self) -> Result<Vec<AttachmentProjectionRow>, AttachmentProjectionError> {
        let mut statement = self
            .backend
            .connection()
            .prepare(LIST_SQL)
            .map_err(|_| AttachmentProjectionError::Storage)?;
        let rows = statement
            .query_map(params![], |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, i64>(8)?,
                    row.get::<_, i64>(9)?,
                    row.get::<_, Option<String>>(10)?,
                    row.get::<_, i64>(11)?,
                ))
            })
            .map_err(|_| AttachmentProjectionError::Storage)?;
        rows.map(|row| {
            let (
                id,
                message_id,
                name,
                media_type,
                size,
                status,
                updated_at_ms,
                attempt_count,
                offset,
                last_error_code,
                direction,
            ) = row.map_err(|_| AttachmentProjectionError::Storage)?;
            Ok(AttachmentProjectionRow {
                id: OpaqueId::from_bytes(fixed16(id)?),
                message_id: OpaqueId::from_bytes(fixed16(message_id)?),
                name,
                media_type,
                size: u64::try_from(size)
                    .map_err(|_| AttachmentProjectionError::InvalidStoredState)?,
                status: status_label(status)?.into(),
                offset: u64::try_from(offset)
                    .map_err(|_| AttachmentProjectionError::InvalidStoredState)?,
                attempt_count: u32::try_from(attempt_count)
                    .map_err(|_| AttachmentProjectionError::InvalidStoredState)?,
                updated_at_ms,
                direction: direction_label(direction)?.into(),
                last_error_code,
            })
        })
        .collect()
    }
}

fn direction_label(value: i64) -> Result<&'static str, AttachmentProjectionError> {
    match value {
        0 => Ok("outbound"),
        1 => Ok("inbound"),
        _ => Err(AttachmentProjectionError::InvalidStoredState),
    }
}

fn fixed16(value: Vec<u8>) -> Result<[u8; 16], AttachmentProjectionError> {
    value.try_into().map_err(|_| AttachmentProjectionError::InvalidStoredState)
}

fn status_label(value: i64) -> Result<&'static str, AttachmentProjectionError> {
    match value {
        0 => Ok("prepared"),
        1 => Ok("encrypting"),
        2 => Ok("queued"),
        3 => Ok("transferring"),
        4 => Ok("available"),
        5 => Ok("failed"),
        6 => Ok("cancelled"),
        _ => Err(AttachmentProjectionError::InvalidStoredState),
    }
}
