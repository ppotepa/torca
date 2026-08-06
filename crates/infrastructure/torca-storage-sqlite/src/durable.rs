use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use torca_foundation::{CommandId, OpaqueId, Timestamp};
use torca_messaging::{Message, MessageId};

/// Outbox work state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutboxState { Pending, Claimed, Completed, DeadLetter }
/// Durable outbound work record.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutboxRecord { pub message: Message, pub command_id: CommandId, pub attempts: u32, pub next_attempt_at: Timestamp, pub state: OutboxState }
/// Durable delivery error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DurableDeliveryError { DuplicateMessage, NotFound, InvalidState }
impl fmt::Display for DurableDeliveryError { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{self:?}") } }
impl std::error::Error for DurableDeliveryError {}
/// Transactional outbox and inbound deduplication port.
pub trait DurableDeliveryStore {
    /// Atomically stores a message and its outbox work.
    fn queue_outbound(&mut self, message: Message, command_id: CommandId, next_attempt_at: Timestamp) -> Result<(), DurableDeliveryError>;
    /// Claims due pending work.
    fn claim_due(&mut self, now: Timestamp, limit: usize) -> Result<Vec<OutboxRecord>, DurableDeliveryError>;
    /// Reschedules claimed work.
    fn reschedule(&mut self, message_id: MessageId, attempts: u32, next_attempt_at: Timestamp) -> Result<(), DurableDeliveryError>;
    /// Completes claimed work.
    fn complete(&mut self, message_id: MessageId) -> Result<(), DurableDeliveryError>;
    /// Records an inbound envelope exactly once.
    fn record_inbound(&mut self, envelope_id: OpaqueId) -> Result<bool, DurableDeliveryError>;
}
/// In-memory model of the durable transaction contract.
#[derive(Clone, Debug, Default)]
pub struct InMemoryDurableDeliveryStore { outbox: BTreeMap<MessageId, OutboxRecord>, inbound: BTreeSet<OpaqueId> }
impl DurableDeliveryStore for InMemoryDurableDeliveryStore {
    fn queue_outbound(&mut self, message: Message, command_id: CommandId, next_attempt_at: Timestamp) -> Result<(), DurableDeliveryError> {
        if self.outbox.contains_key(&message.id()) { return Err(DurableDeliveryError::DuplicateMessage); }
        self.outbox.insert(message.id(), OutboxRecord { message, command_id, attempts: 0, next_attempt_at, state: OutboxState::Pending }); Ok(())
    }
    fn claim_due(&mut self, now: Timestamp, limit: usize) -> Result<Vec<OutboxRecord>, DurableDeliveryError> {
        let ids: Vec<_> = self.outbox.iter().filter(|(_, record)| record.state == OutboxState::Pending && record.next_attempt_at <= now).take(limit).map(|(id, _)| *id).collect();
        let mut claimed = Vec::with_capacity(ids.len());
        for id in ids { if let Some(record) = self.outbox.get_mut(&id) { record.state = OutboxState::Claimed; claimed.push(record.clone()); } }
        Ok(claimed)
    }
    fn reschedule(&mut self, message_id: MessageId, attempts: u32, next_attempt_at: Timestamp) -> Result<(), DurableDeliveryError> {
        let record = self.outbox.get_mut(&message_id).ok_or(DurableDeliveryError::NotFound)?;
        if record.state != OutboxState::Claimed { return Err(DurableDeliveryError::InvalidState); }
        record.attempts = attempts; record.next_attempt_at = next_attempt_at; record.state = OutboxState::Pending; Ok(())
    }
    fn complete(&mut self, message_id: MessageId) -> Result<(), DurableDeliveryError> {
        let record = self.outbox.get_mut(&message_id).ok_or(DurableDeliveryError::NotFound)?;
        if record.state != OutboxState::Claimed { return Err(DurableDeliveryError::InvalidState); }
        record.state = OutboxState::Completed; Ok(())
    }
    fn record_inbound(&mut self, envelope_id: OpaqueId) -> Result<bool, DurableDeliveryError> { Ok(self.inbound.insert(envelope_id)) }
}
