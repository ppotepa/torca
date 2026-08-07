//! Durable control delivery worker. Persistence is supplied by an infrastructure adapter.

use core::fmt;
use std::time::Duration;

use torca_foundation::{OpaqueId, Timestamp};

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
    pub const fn from_storage(value: i64) -> Option<Self> {
        match value {
            1 => Some(Self::Receipt),
            2 => Some(Self::Attachment),
            _ => None,
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

/// Durable control-outbox persistence port. SQLCipher belongs to infrastructure, not this crate.
pub trait ControlOutboxStore: Send {
    fn queue(
        &mut self,
        job_id: OpaqueId,
        contact_id: OpaqueId,
        kind: ControlKind,
        payload: &[u8],
        next_attempt_at: Timestamp,
    ) -> Result<(), ControlDeliveryError>;
    fn recover_stale(&mut self, claimed_before: Timestamp) -> Result<usize, ControlDeliveryError>;
    fn claim_due(&mut self, now: Timestamp, limit: usize) -> Result<Vec<ControlJob>, ControlDeliveryError>;
    fn reschedule(&mut self, job_id: OpaqueId, next_attempt_at: Timestamp) -> Result<(), ControlDeliveryError>;
    fn complete(&mut self, job_id: OpaqueId) -> Result<(), ControlDeliveryError>;
    fn dead_letter(&mut self, job_id: OpaqueId) -> Result<(), ControlDeliveryError>;
}

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
    outbox: Box<dyn ControlOutboxStore>,
    transport: T,
}
impl<T: ControlTransport> ControlDeliveryWorker<T> {
    pub fn new<S>(outbox: S, transport: T) -> Self
    where
        S: ControlOutboxStore + 'static,
    {
        Self { outbox: Box::new(outbox), transport }
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
        if payload.len() > MAX_CONTROL_PAYLOAD {
            return Err(ControlDeliveryError::PayloadTooLarge);
        }
        self.outbox.queue(job_id, contact_id, kind, payload, next_attempt_at)
    }

    pub fn into_transport(self) -> T { self.transport }
}

fn retry_delay(attempts: u32) -> Duration {
    let exponent = attempts.saturating_sub(1).min(16);
    BASE_DELAY
        .checked_mul(1_u32 << exponent)
        .unwrap_or(MAX_DELAY)
        .min(MAX_DELAY)
}
