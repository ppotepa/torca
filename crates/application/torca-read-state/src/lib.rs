//! Transactional local Read state plus durable peer Read receipts.

use std::path::Path;

use rusqlite::params;
use torca_delivery::{
    ApplicationPayload, ApplicationPayloadCodec, DeliveryReceiptKind, ReceiptPayload,
};
use torca_foundation::{OpaqueId, Timestamp};
use torca_storage_sqlite::{
    DatabaseKey, SqlCipherBackend, StorageBackend, StorageKernel,
};

const READ_CANDIDATES_SQL: &str = include_str!("../sql/read_candidates.sql");
const MARK_READ_SQL: &str = include_str!("../sql/mark_message_read.sql");
const INSERT_RECEIPT_SQL: &str = include_str!("../sql/insert_read_receipt_job.sql");

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReadStateError {
    Storage,
    InvalidStoredId,
    Protocol,
}
impl core::fmt::Display for ReadStateError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for ReadStateError {}

pub struct SqlCipherReadState {
    backend: SqlCipherBackend,
}
impl SqlCipherReadState {
    pub fn open(path: impl AsRef<Path>, key: &DatabaseKey) -> Result<Self, ReadStateError> {
        let backend = SqlCipherBackend::open(path, key).map_err(|_| ReadStateError::Storage)?;
        let mut kernel = StorageKernel::new(backend);
        kernel.bootstrap().map_err(|_| ReadStateError::Storage)?;
        Ok(Self { backend: kernel.into_backend() })
    }

    pub fn mark_conversation_read(
        &mut self,
        conversation_id: OpaqueId,
        at: Timestamp,
    ) -> Result<usize, ReadStateError> {
        let conversation = conversation_id.into_bytes();
        let candidates = {
            let mut statement = self
                .backend
                .connection()
                .prepare(READ_CANDIDATES_SQL)
                .map_err(|_| ReadStateError::Storage)?;
            let rows = statement
                .query_map(params![conversation.as_slice()], |row| {
                    Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, Vec<u8>>(1)?))
                })
                .map_err(|_| ReadStateError::Storage)?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(|_| ReadStateError::Storage)?
        };
        if candidates.is_empty() {
            return Ok(0);
        }

        self.backend.begin().map_err(|_| ReadStateError::Storage)?;
        let result = (|| {
            let mut changed = 0_usize;
            for (contact, message) in candidates {
                let contact_id = OpaqueId::from_bytes(fixed16(contact)?);
                let message_id = OpaqueId::from_bytes(fixed16(message)?);
                let receipt_id = derived_receipt_id(message_id);
                let payload = ApplicationPayloadCodec::encode(&ApplicationPayload::Receipt(
                    ReceiptPayload {
                        receipt_id,
                        message_id,
                        contact_id,
                        kind: DeliveryReceiptKind::Read,
                        at,
                    },
                ))
                .map_err(|_| ReadStateError::Protocol)?;
                let message_bytes = message_id.into_bytes();
                let updated = self
                    .backend
                    .connection()
                    .execute(
                        MARK_READ_SQL,
                        params![message_bytes.as_slice(), at.to_unix_millis()],
                    )
                    .map_err(|_| ReadStateError::Storage)?;
                if updated == 0 {
                    continue;
                }
                let receipt_bytes = receipt_id.into_bytes();
                let contact_bytes = contact_id.into_bytes();
                self.backend
                    .connection()
                    .execute(
                        INSERT_RECEIPT_SQL,
                        params![
                            receipt_bytes.as_slice(),
                            contact_bytes.as_slice(),
                            payload,
                            at.to_unix_millis(),
                        ],
                    )
                    .map_err(|_| ReadStateError::Storage)?;
                changed += 1;
            }
            Ok::<usize, ReadStateError>(changed)
        })();
        match result {
            Ok(changed) => {
                self.backend.commit().map_err(|_| ReadStateError::Storage)?;
                Ok(changed)
            }
            Err(error) => {
                let _ = self.backend.rollback();
                Err(error)
            }
        }
    }
}

fn fixed16(value: Vec<u8>) -> Result<[u8; 16], ReadStateError> {
    value.try_into().map_err(|_| ReadStateError::InvalidStoredId)
}

fn derived_receipt_id(message_id: OpaqueId) -> OpaqueId {
    let mut bytes = message_id.into_bytes();
    bytes[15] ^= 0xA1;
    let value = OpaqueId::from_bytes(bytes);
    if value.is_nil() { OpaqueId::from_u128(0xA2) } else { value }
}
