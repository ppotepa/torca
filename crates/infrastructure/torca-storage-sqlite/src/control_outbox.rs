use std::path::Path;

use rusqlite::{OptionalExtension, params};
use torca_control_delivery::{
    ControlDeliveryError, ControlJob, ControlKind, ControlOutboxStore, MAX_CONTROL_PAYLOAD,
};
use torca_foundation::{OpaqueId, Timestamp};

use crate::{DatabaseKey, SqlCipherBackend, StorageBackendError, StorageKernel};

const INSERT_SQL: &str = include_str!("../sql/commands/control_insert.sql");
const CLAIM_SQL: &str = include_str!("../sql/queries/control_claim_due.sql");
const RESCHEDULE_SQL: &str = include_str!("../sql/commands/control_reschedule.sql");
const COMPLETE_SQL: &str = include_str!("../sql/commands/control_complete.sql");
const DEAD_LETTER_SQL: &str = include_str!("../sql/commands/control_dead_letter.sql");
const RECOVER_STALE_SQL: &str = include_str!("../sql/commands/control_recover_stale.sql");
const EXISTS_SQL: &str = include_str!("../sql/queries/control_exists.sql");

pub struct SqlCipherControlOutbox {
    backend: SqlCipherBackend,
}
impl SqlCipherControlOutbox {
    pub fn open(path: impl AsRef<Path>, key: &DatabaseKey) -> Result<Self, ControlDeliveryError> {
        let backend = SqlCipherBackend::open(path, key).map_err(map_backend)?;
        let mut kernel = StorageKernel::new(backend);
        kernel.bootstrap().map_err(|_| ControlDeliveryError::Migration)?;
        Ok(Self { backend: kernel.into_backend() })
    }
    fn transition(
        &mut self,
        sql: &str,
        job_id: OpaqueId,
        time: Option<i64>,
    ) -> Result<(), ControlDeliveryError> {
        let id = job_id.into_bytes();
        let changed = match time {
            Some(value) => self.backend.connection().execute(sql, params![id.as_slice(), value]),
            None => self.backend.connection().execute(sql, params![id.as_slice()]),
        }
        .map_err(|_| ControlDeliveryError::Backend)?;
        if changed == 1 {
            return Ok(());
        }
        let exists = self
            .backend
            .connection()
            .query_row(EXISTS_SQL, params![id.as_slice()], |_| Ok(()))
            .optional()
            .map_err(|_| ControlDeliveryError::Backend)?
            .is_some();
        if exists {
            Err(ControlDeliveryError::InvalidState)
        } else {
            Err(ControlDeliveryError::NotFound)
        }
    }
}
impl ControlOutboxStore for SqlCipherControlOutbox {
    fn queue(
        &mut self,
        job_id: OpaqueId,
        contact_id: OpaqueId,
        kind: ControlKind,
        payload: &[u8],
        next_attempt_at: Timestamp,
    ) -> Result<(), ControlDeliveryError> {
        if payload.len() > MAX_CONTROL_PAYLOAD {
            return Err(ControlDeliveryError::PayloadTooLarge);
        }
        let job = job_id.into_bytes();
        let contact = contact_id.into_bytes();
        self.backend
            .connection()
            .execute(
                INSERT_SQL,
                params![
                    job.as_slice(),
                    contact.as_slice(),
                    kind as i64,
                    payload,
                    next_attempt_at.to_unix_millis()
                ],
            )
            .map_err(|error| {
                if error.sqlite_error_code() == Some(rusqlite::ErrorCode::ConstraintViolation) {
                    ControlDeliveryError::Duplicate
                } else {
                    ControlDeliveryError::Backend
                }
            })?;
        Ok(())
    }
    fn recover_stale(&mut self, claimed_before: Timestamp) -> Result<usize, ControlDeliveryError> {
        self.backend
            .connection()
            .execute(RECOVER_STALE_SQL, params![claimed_before.to_unix_millis()])
            .map_err(|_| ControlDeliveryError::Backend)
    }
    fn claim_due(
        &mut self,
        now: Timestamp,
        limit: usize,
    ) -> Result<Vec<ControlJob>, ControlDeliveryError> {
        let limit = i64::try_from(limit).map_err(|_| ControlDeliveryError::InvalidState)?;
        let mut statement = self
            .backend
            .connection()
            .prepare(CLAIM_SQL)
            .map_err(|_| ControlDeliveryError::Backend)?;
        let rows = statement
            .query_map(params![now.to_unix_millis(), limit], |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, Vec<u8>>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            })
            .map_err(|_| ControlDeliveryError::Backend)?;
        let mut result = Vec::new();
        for row in rows {
            let (job, contact, kind, payload, attempts) =
                row.map_err(|_| ControlDeliveryError::Backend)?;
            if payload.len() > MAX_CONTROL_PAYLOAD {
                return Err(ControlDeliveryError::InvalidStoredJob);
            }
            result.push(ControlJob {
                job_id: OpaqueId::from_bytes(fixed16(job)?),
                contact_id: OpaqueId::from_bytes(fixed16(contact)?),
                kind: ControlKind::from_storage(kind)
                    .ok_or(ControlDeliveryError::InvalidStoredJob)?,
                payload,
                attempts: u32::try_from(attempts)
                    .map_err(|_| ControlDeliveryError::InvalidStoredJob)?,
            });
        }
        Ok(result)
    }
    fn reschedule(
        &mut self,
        job_id: OpaqueId,
        next_attempt_at: Timestamp,
    ) -> Result<(), ControlDeliveryError> {
        self.transition(RESCHEDULE_SQL, job_id, Some(next_attempt_at.to_unix_millis()))
    }
    fn complete(&mut self, job_id: OpaqueId) -> Result<(), ControlDeliveryError> {
        self.transition(COMPLETE_SQL, job_id, None)
    }
    fn dead_letter(&mut self, job_id: OpaqueId) -> Result<(), ControlDeliveryError> {
        self.transition(DEAD_LETTER_SQL, job_id, None)
    }
}
fn fixed16(value: Vec<u8>) -> Result<[u8; 16], ControlDeliveryError> {
    value.try_into().map_err(|_| ControlDeliveryError::InvalidStoredJob)
}
fn map_backend(_: StorageBackendError) -> ControlDeliveryError {
    ControlDeliveryError::Backend
}
