use std::path::Path;

use rusqlite::params;
use torca_control_delivery::{ControlKind, PendingControlJob, ReadCandidate};
use torca_foundation::{OpaqueId, Timestamp};

use crate::{DatabaseKey, SqlCipherBackend, StorageBackend, StorageKernel};

const READ_CANDIDATES_SQL: &str = include_str!("../sql/queries/read_candidates.sql");
const MARK_READ_SQL: &str = include_str!("../sql/commands/mark_message_read.sql");
const INSERT_RECEIPT_SQL: &str = include_str!("../sql/commands/insert_read_receipt_job.sql");

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

    pub fn read_candidates(
        &mut self,
        conversation_id: OpaqueId,
    ) -> Result<Vec<ReadCandidate>, ReadStateError> {
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
            rows.collect::<Result<Vec<_>, _>>().map_err(|_| ReadStateError::Storage)?
        };
        candidates
            .into_iter()
            .map(|(contact, message)| {
                Ok(ReadCandidate {
                    contact_id: OpaqueId::from_bytes(fixed16(contact)?),
                    message_id: OpaqueId::from_bytes(fixed16(message)?),
                })
            })
            .collect()
    }

    pub fn commit_mark_read(
        &mut self,
        conversation_id: OpaqueId,
        at: Timestamp,
        jobs: &[PendingControlJob],
    ) -> Result<usize, ReadStateError> {
        let candidates = self.read_candidates(conversation_id)?;
        if candidates.is_empty() {
            return Ok(0);
        }
        self.backend.begin().map_err(|_| ReadStateError::Storage)?;
        let result = (|| {
            let mut changed = 0_usize;
            let mut changed_messages = Vec::new();
            for candidate in candidates {
                let message_id = candidate.message_id;
                let message_bytes = message_id.into_bytes();
                let updated = self
                    .backend
                    .connection()
                    .execute(MARK_READ_SQL, params![message_bytes.as_slice(), at.to_unix_millis()])
                    .map_err(|_| ReadStateError::Storage)?;
                if updated == 0 {
                    continue;
                }

                changed_messages.push(message_id);
                changed += 1;
            }
            for job in jobs.iter().filter(|job| {
                job.kind == ControlKind::Receipt
                    && job.message_id.is_some_and(|message| changed_messages.contains(&message))
            }) {
                let receipt_bytes = job.job_id.into_bytes();
                let contact_bytes = job.contact_id.into_bytes();
                self.backend
                    .connection()
                    .execute(
                        INSERT_RECEIPT_SQL,
                        params![
                            receipt_bytes.as_slice(),
                            contact_bytes.as_slice(),
                            &job.payload,
                            job.next_attempt_at.to_unix_millis(),
                        ],
                    )
                    .map_err(|_| ReadStateError::Storage)?;
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

    pub fn mark_conversation_read(
        &mut self,
        conversation_id: OpaqueId,
        at: Timestamp,
    ) -> Result<usize, ReadStateError> {
        self.commit_mark_read(conversation_id, at, &[])
    }
}

fn fixed16(value: Vec<u8>) -> Result<[u8; 16], ReadStateError> {
    value.try_into().map_err(|_| ReadStateError::InvalidStoredId)
}
