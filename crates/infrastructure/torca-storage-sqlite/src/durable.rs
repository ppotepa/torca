use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use torca_foundation::{CommandId, OpaqueId, Timestamp};
use torca_messaging::{Message, MessageId};

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

/// Durable delivery failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DurableDeliveryError {
    DuplicateMessage,
    NotFound,
    InvalidState,
    /// Redaction-safe infrastructure failure.
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

/// In-memory reference implementation.
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
