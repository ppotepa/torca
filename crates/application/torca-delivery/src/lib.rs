//! Durable outbound-delivery orchestration independent from any concrete transport or database.

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use torca_foundation::{CommandId, OpaqueId, Timestamp};
use torca_messaging::{Message, MessageId, RetryPolicy};

/// Durable outbox lifecycle state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutboxState {
    Pending,
    Claimed,
    Completed,
    DeadLetter,
}

/// Message and delivery metadata returned to a worker.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutboxRecord {
    pub message: Message,
    pub command_id: CommandId,
    pub attempts: u32,
    pub next_attempt_at: Timestamp,
    pub claimed_at: Option<Timestamp>,
    pub state: OutboxState,
}

/// Durable delivery persistence failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DurableDeliveryError {
    DuplicateMessage,
    NotFound,
    InvalidState,
    Storage(String),
}
impl fmt::Display for DurableDeliveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for DurableDeliveryError {}

/// Transactional outbox and inbound-dedup persistence port.
pub trait DurableDeliveryStore {
    fn queue_outbound(
        &mut self,
        message: Message,
        command_id: CommandId,
        next_attempt_at: Timestamp,
    ) -> Result<(), DurableDeliveryError>;
    fn claim_due(
        &mut self,
        now: Timestamp,
        limit: usize,
    ) -> Result<Vec<OutboxRecord>, DurableDeliveryError>;
    fn reschedule(
        &mut self,
        message_id: MessageId,
        attempts: u32,
        next_attempt_at: Timestamp,
    ) -> Result<(), DurableDeliveryError>;
    fn complete(&mut self, message_id: MessageId) -> Result<(), DurableDeliveryError>;
    fn dead_letter(&mut self, message_id: MessageId) -> Result<(), DurableDeliveryError>;
    fn recover_stale_claims(
        &mut self,
        claimed_before: Timestamp,
    ) -> Result<usize, DurableDeliveryError>;
    fn record_inbound(&mut self, envelope_id: OpaqueId) -> Result<bool, DurableDeliveryError>;
}

/// In-memory reference implementation used only by tests and explicit previews.
#[derive(Clone, Debug, Default)]
pub struct InMemoryDurableDeliveryStore {
    outbox: BTreeMap<MessageId, OutboxRecord>,
    inbound: BTreeSet<OpaqueId>,
}
impl DurableDeliveryStore for InMemoryDurableDeliveryStore {
    fn queue_outbound(
        &mut self,
        message: Message,
        command_id: CommandId,
        next_attempt_at: Timestamp,
    ) -> Result<(), DurableDeliveryError> {
        if self.outbox.contains_key(&message.id()) {
            return Err(DurableDeliveryError::DuplicateMessage);
        }
        self.outbox.insert(
            message.id(),
            OutboxRecord {
                message,
                command_id,
                attempts: 0,
                next_attempt_at,
                claimed_at: None,
                state: OutboxState::Pending,
            },
        );
        Ok(())
    }

    fn claim_due(
        &mut self,
        now: Timestamp,
        limit: usize,
    ) -> Result<Vec<OutboxRecord>, DurableDeliveryError> {
        let ids: Vec<_> = self
            .outbox
            .iter()
            .filter(|(_, record)| {
                record.state == OutboxState::Pending && record.next_attempt_at <= now
            })
            .take(limit)
            .map(|(id, _)| *id)
            .collect();
        let mut claimed = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(record) = self.outbox.get_mut(&id) {
                record.state = OutboxState::Claimed;
                record.claimed_at = Some(now);
                claimed.push(record.clone());
            }
        }
        Ok(claimed)
    }

    fn reschedule(
        &mut self,
        message_id: MessageId,
        attempts: u32,
        next_attempt_at: Timestamp,
    ) -> Result<(), DurableDeliveryError> {
        let record = self.outbox.get_mut(&message_id).ok_or(DurableDeliveryError::NotFound)?;
        if record.state != OutboxState::Claimed {
            return Err(DurableDeliveryError::InvalidState);
        }
        record.attempts = attempts;
        record.next_attempt_at = next_attempt_at;
        record.claimed_at = None;
        record.state = OutboxState::Pending;
        Ok(())
    }

    fn complete(&mut self, message_id: MessageId) -> Result<(), DurableDeliveryError> {
        let record = self.outbox.get_mut(&message_id).ok_or(DurableDeliveryError::NotFound)?;
        if record.state != OutboxState::Claimed {
            return Err(DurableDeliveryError::InvalidState);
        }
        record.claimed_at = None;
        record.state = OutboxState::Completed;
        Ok(())
    }

    fn dead_letter(&mut self, message_id: MessageId) -> Result<(), DurableDeliveryError> {
        let record = self.outbox.get_mut(&message_id).ok_or(DurableDeliveryError::NotFound)?;
        if !matches!(record.state, OutboxState::Claimed | OutboxState::Pending) {
            return Err(DurableDeliveryError::InvalidState);
        }
        record.claimed_at = None;
        record.state = OutboxState::DeadLetter;
        Ok(())
    }

    fn recover_stale_claims(
        &mut self,
        claimed_before: Timestamp,
    ) -> Result<usize, DurableDeliveryError> {
        let mut recovered = 0;
        for record in self.outbox.values_mut() {
            if record.state == OutboxState::Claimed
                && record.claimed_at.is_some_and(|at| at <= claimed_before)
            {
                record.state = OutboxState::Pending;
                record.claimed_at = None;
                recovered += 1;
            }
        }
        Ok(recovered)
    }

    fn record_inbound(&mut self, envelope_id: OpaqueId) -> Result<bool, DurableDeliveryError> {
        Ok(self.inbound.insert(envelope_id))
    }
}

/// Protocol acknowledgement accepted by the durable worker.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeliveryAck {
    Accepted,
    Duplicate,
}

/// Redaction-safe transport failure. Transport-specific details stay behind the adapter boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeliveryTransportError(pub String);
impl fmt::Display for DeliveryTransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}
impl std::error::Error for DeliveryTransportError {}

/// One authenticated transport capable of waiting for the peer protocol ACK of a message.
pub trait DeliveryTransport {
    fn send(&mut self, message: &Message) -> Result<DeliveryAck, DeliveryTransportError>;
}

/// Summary of one bounded delivery pass.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DeliveryBatchReport {
    pub claimed: usize,
    pub completed: usize,
    pub rescheduled: usize,
    pub dead_lettered: usize,
}

/// Delivery-worker failure. Individual transport failures are converted into retry/dead-letter
/// decisions and therefore are not returned as worker failures.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeliveryWorkerError {
    Store(DurableDeliveryError),
    AttemptOverflow,
    TimestampOverflow,
}
impl fmt::Display for DeliveryWorkerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for DeliveryWorkerError {}
impl From<DurableDeliveryError> for DeliveryWorkerError {
    fn from(value: DurableDeliveryError) -> Self {
        Self::Store(value)
    }
}

/// Single owner of durable outbound delivery attempts.
pub struct DeliveryWorker<S, T> {
    store: S,
    transport: T,
    retry_policy: RetryPolicy,
}

impl<S, T> DeliveryWorker<S, T>
where
    S: DurableDeliveryStore,
    T: DeliveryTransport,
{
    /// Creates a worker. No background thread is hidden here; the runtime decides when to call it.
    pub const fn new(store: S, transport: T, retry_policy: RetryPolicy) -> Self {
        Self { store, transport, retry_policy }
    }

    /// Requeues claims left behind by a previous process instance.
    pub fn recover_stale_claims(
        &mut self,
        claimed_before: Timestamp,
    ) -> Result<usize, DeliveryWorkerError> {
        self.store.recover_stale_claims(claimed_before).map_err(Into::into)
    }

    /// Processes at most `limit` currently due records.
    pub fn run_once(
        &mut self,
        now: Timestamp,
        limit: usize,
    ) -> Result<DeliveryBatchReport, DeliveryWorkerError> {
        let records = self.store.claim_due(now, limit)?;
        let mut report = DeliveryBatchReport { claimed: records.len(), ..Default::default() };

        for record in records {
            let message_id = record.message.id();
            let attempts = record
                .attempts
                .checked_add(1)
                .ok_or(DeliveryWorkerError::AttemptOverflow)?;
            match self.transport.send(&record.message) {
                Ok(DeliveryAck::Accepted | DeliveryAck::Duplicate) => {
                    self.store.complete(message_id)?;
                    report.completed += 1;
                }
                Err(_) => match self.retry_policy.delay_after(attempts) {
                    Some(delay) => {
                        let next = now
                            .checked_add(delay)
                            .ok_or(DeliveryWorkerError::TimestampOverflow)?;
                        self.store.reschedule(message_id, attempts, next)?;
                        report.rescheduled += 1;
                    }
                    None => {
                        self.store.dead_letter(message_id)?;
                        report.dead_lettered += 1;
                    }
                },
            }
        }
        Ok(report)
    }

    /// Consumes the worker and returns its owned adapters.
    pub fn into_parts(self) -> (S, T) {
        (self.store, self.transport)
    }
}
