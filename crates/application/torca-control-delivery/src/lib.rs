//! Durable control delivery for receipts and attachment-control frames.

use core::fmt;
use std::path::Path;
use std::time::Duration;

use rusqlite::{OptionalExtension, params};
use torca_foundation::{OpaqueId, Timestamp};
use torca_storage_sqlite::{
    DatabaseKey, MigrationError, SqlCipherBackend, StorageBackendError, StorageKernel,
};

const INSERT_SQL: &str = include_str!("../sql/control_insert.sql");
const CLAIM_SQL: &str = include_str!("../sql/control_claim_due.sql");
const RESCHEDULE_SQL: &str = include_str!("../sql/control_reschedule.sql");
const COMPLETE_SQL: &str = include_str!("../sql/control_complete.sql");
const DEAD_LETTER_SQL: &str = include_str!("../sql/control_dead_letter.sql");
const RECOVER_STALE_SQL: &str = include_str!("../sql/control_recover_stale.sql");

pub const MAX_CONTROL_PAYLOAD: usize = 64 * 1024;
const MAX_ATTEMPTS: u32 = 8;
const BASE_DELAY: Duration = Duration::from_secs(1);
const MAX_DELAY: Duration = Duration::from_secs(60);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlKind {
    Receipt = 1,
    Attachment = 2,
}
impl ControlKind {
    fn from_i64(value: i64) -> Result<Self, ControlDeliveryError> {
        match value {
            1 => Ok(Self::Receipt),
            2 => Ok(Self::Attachment),
            _ => Err(ControlDeliveryError::InvalidStoredJob),
        }
    }
}

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlJob {
    pub job_id: OpaqueId,
    pub contact_id: OpaqueId,
    pub kind: ControlKind,
    pub payload: Vec<u8>,
    pub attempts: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ControlDeliveryError {
    Backend,
    Migration,
    Duplicate,
    NotFound,
    InvalidState,
    PayloadTooLarge,
    InvalidStoredJob,
    TimestampOverflow,
}
impl fmt::Display for ControlDeliveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for ControlDeliveryError {}

pub struct ControlOutbox {
    backend: SqlCipherBackend,
}
impl ControlOutbox {
    pub fn open(path: impl AsRef<Path>, key: &DatabaseKey) -> Result<Self, ControlDeliveryError> {
        let backend = SqlCipherBackend::open(path, key).map_err(map_backend)?;
        Self::bootstrap(backend)
    }

    pub fn open_in_memory(key: &DatabaseKey) -> Result<Self, ControlDeliveryError> {
        let backend = SqlCipherBackend::open_in_memory(key).map_err(map_backend)?;
        Self::bootstrap(backend)
    }

    fn bootstrap(backend: SqlCipherBackend) -> Result<Self, ControlDeliveryError> {
        let mut kernel = StorageKernel::new(backend);
        kernel.bootstrap().map_err(map_migration)?;
        Ok(Self { backend: kernel.into_backend() })
    }

    pub fn queue(
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
                    next_attempt_at.to_unix_millis(),
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

    pub fn recover_stale(&mut self, claimed_before: Timestamp) -> Result<usize, ControlDeliveryError> {
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
                kind: ControlKind::from_i64(kind)?,
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
        self.transition(
            RESCHEDULE_SQL,
            job_id,
            Some(next_attempt_at.to_unix_millis()),
        )
    }

    fn complete(&mut self, job_id: OpaqueId) -> Result<(), ControlDeliveryError> {
        self.transition(COMPLETE_SQL, job_id, None)
    }

    fn dead_letter(&mut self, job_id: OpaqueId) -> Result<(), ControlDeliveryError> {
        self.transition(DEAD_LETTER_SQL, job_id, None)
    }

    fn transition(
        &mut self,
        sql: &str,
        job_id: OpaqueId,
        time: Option<i64>,
    ) -> Result<(), ControlDeliveryError> {
        let id = job_id.into_bytes();
        let changed = match time {
            Some(value) => self
                .backend
                .connection()
                .execute(sql, params![id.as_slice(), value]),
            None => self.backend.connection().execute(sql, params![id.as_slice()]),
        }
        .map_err(|_| ControlDeliveryError::Backend)?;
        if changed == 1 {
            return Ok(());
        }
        let exists = self
            .backend
            .connection()
            .query_row(
                "SELECT 1 FROM control_outbox WHERE job_id = ?1",
                params![id.as_slice()],
                |_| Ok(()),
            )
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

/// ACK result for one control frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlAck {
    Accepted,
    Duplicate,
}

pub trait ControlTransport {
    fn send_control(&mut self, job: &ControlJob) -> Result<ControlAck, ControlTransportError>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlTransportError;
impl fmt::Display for ControlTransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("control transport failed")
    }
}
impl std::error::Error for ControlTransportError {}

#[must_use]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ControlBatchReport {
    pub claimed: usize,
    pub completed: usize,
    pub rescheduled: usize,
    pub dead_lettered: usize,
}

pub struct ControlDeliveryWorker<T> {
    outbox: ControlOutbox,
    transport: T,
}
impl<T: ControlTransport> ControlDeliveryWorker<T> {
    pub const fn new(outbox: ControlOutbox, transport: T) -> Self {
        Self { outbox, transport }
    }

    pub fn recover_stale(&mut self, before: Timestamp) -> Result<usize, ControlDeliveryError> {
        self.outbox.recover_stale(before)
    }

    pub fn run_once(
        &mut self,
        now: Timestamp,
        limit: usize,
    ) -> Result<ControlBatchReport, ControlDeliveryError> {
        let jobs = self.outbox.claim_due(now, limit)?;
        let mut report = ControlBatchReport { claimed: jobs.len(), ..Default::default() };
        for job in jobs {
            match self.transport.send_control(&job) {
                Ok(ControlAck::Accepted | ControlAck::Duplicate) => {
                    self.outbox.complete(job.job_id)?;
                    report.completed += 1;
                }
                Err(_) if job.attempts >= MAX_ATTEMPTS => {
                    self.outbox.dead_letter(job.job_id)?;
                    report.dead_lettered += 1;
                }
                Err(_) => {
                    let delay = retry_delay(job.attempts);
                    let next = now.checked_add(delay).ok_or(ControlDeliveryError::TimestampOverflow)?;
                    self.outbox.reschedule(job.job_id, next)?;
                    report.rescheduled += 1;
                }
            }
        }
        Ok(report)
    }

    pub fn queue(
        &mut self,
        job_id: OpaqueId,
        contact_id: OpaqueId,
        kind: ControlKind,
        payload: &[u8],
        next_attempt_at: Timestamp,
    ) -> Result<(), ControlDeliveryError> {
        self.outbox.queue(job_id, contact_id, kind, payload, next_attempt_at)
    }

    pub fn into_parts(self) -> (ControlOutbox, T) {
        (self.outbox, self.transport)
    }
}

fn retry_delay(attempts: u32) -> Duration {
    let exponent = attempts.saturating_sub(1).min(16);
    BASE_DELAY
        .checked_mul(1_u32 << exponent)
        .unwrap_or(MAX_DELAY)
        .min(MAX_DELAY)
}

fn fixed16(value: Vec<u8>) -> Result<[u8; 16], ControlDeliveryError> {
    value.try_into().map_err(|_| ControlDeliveryError::InvalidStoredJob)
}
fn map_backend(_: StorageBackendError) -> ControlDeliveryError {
    ControlDeliveryError::Backend
}
fn map_migration(_: MigrationError) -> ControlDeliveryError {
    ControlDeliveryError::Migration
}
