//! Ephemeral in-memory rendezvous broker. It owns no user database or offline mailbox.

mod server;

use std::collections::{BTreeMap, VecDeque};

use torca_foundation::{OpaqueId, Timestamp};
use torca_relay_protocol::{
    RelayCode, RelayDelivery, RelayInfo, RelayJoinTicket, RelayMessageId, RelayOperationId,
    RelayProtocolError, RelayProtocolVersion, RelayRequest, RelayResponse, RelaySequence,
    RelaySide, RelaySideToken, RelaySlotCapability, RelaySlotId, validate_blob,
};

pub use server::{DEFAULT_MAX_CONNECTIONS, RelayServer, RelayServerConfig, RelayServerError};

/// Default maximum number of simultaneously active ephemeral pairing slots.
pub const DEFAULT_MAX_ACTIVE_SLOTS: usize = 4096;
/// The relay clock is authoritative for the short lifetime of a pairing slot.
pub const PAIRING_SLOT_TTL: std::time::Duration = std::time::Duration::from_secs(5 * 60);
/// Global ceiling for unsuccessful code lookups in a relay-clock minute.
pub const MAX_FAILED_JOINS_PER_MINUTE: u32 = 60;
const MAX_QUEUED_BLOBS_PER_SIDE: usize = 32;

#[derive(Clone, Debug)]
#[allow(clippy::struct_field_names)]
struct Slot {
    id: RelaySlotId,
    expires_at: Timestamp,
    creator_blob: Vec<u8>,
    slot_capability: RelaySlotCapability,
    creator_token: RelaySideToken,
    ticket: RelayJoinTicket,
    joiner_token: Option<RelaySideToken>,
    to_creator: VecDeque<RelayDelivery>,
    to_joiner: VecDeque<RelayDelivery>,
    next_to_creator_sequence: u64,
    next_to_joiner_sequence: u64,
}

impl Slot {
    fn authenticate(&self, token: RelaySideToken) -> Result<RelaySide, RelayProtocolError> {
        if token == self.creator_token {
            return Ok(RelaySide::Creator);
        }
        if self.joiner_token == Some(token) {
            return Ok(RelaySide::Joiner);
        }
        Err(RelayProtocolError::Unauthorized)
    }

    fn enqueue(
        &mut self,
        receiving_side: RelaySide,
        message_id: RelayMessageId,
        blob: Vec<u8>,
    ) -> Result<(), RelayProtocolError> {
        let (queue, next_sequence) = match receiving_side {
            RelaySide::Creator => (&mut self.to_creator, &mut self.next_to_creator_sequence),
            RelaySide::Joiner => (&mut self.to_joiner, &mut self.next_to_joiner_sequence),
        };
        if queue.len() >= MAX_QUEUED_BLOBS_PER_SIDE {
            return Err(RelayProtocolError::QueueFull);
        }
        let sequence = *next_sequence;
        *next_sequence =
            next_sequence.checked_add(1).ok_or(RelayProtocolError::InvalidOperation)?;
        queue.push_back(RelayDelivery { sequence: RelaySequence(sequence), message_id, blob });
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct OperationRecord {
    expires_at: Timestamp,
    response: RelayResponse,
}

/// Deterministic ephemeral relay broker.
#[derive(Clone, Debug)]
pub struct RelayBroker {
    info: RelayInfo,
    slots: BTreeMap<RelayCode, Slot>,
    operation_results: BTreeMap<RelayOperationId, OperationRecord>,
    next_id: u128,
    max_slots: usize,
    failed_join_window_started_at: Option<Timestamp>,
    failed_join_attempts: u32,
}
impl Default for RelayBroker {
    fn default() -> Self {
        Self::with_max_slots(DEFAULT_MAX_ACTIVE_SLOTS)
    }
}

impl RelayBroker {
    /// Creates a bounded broker. A zero capacity is normalized to one slot.
    pub fn with_max_slots(max_slots: usize) -> Self {
        let info = RelayInfo::new(
            env!("CARGO_PKG_VERSION"),
            option_env!("TORCA_RELAY_BUILD_ID").unwrap_or("development"),
            option_env!("TORCA_RELAY_SOURCE_COMMIT").unwrap_or("working-tree"),
        )
        .unwrap_or_else(|_| RelayInfo {
            product_version: env!("CARGO_PKG_VERSION").to_owned(),
            build_id: "invalid-build-metadata".to_owned(),
            source_commit: "unknown".to_owned(),
            protocol_version: RelayProtocolVersion::V4.0,
        });
        Self {
            info,
            slots: BTreeMap::new(),
            operation_results: BTreeMap::new(),
            next_id: 1,
            max_slots: max_slots.max(1),
            failed_join_window_started_at: None,
            failed_join_attempts: 0,
        }
    }

    /// Applies one request at a trusted clock value.
    pub fn handle(
        &mut self,
        request: RelayRequest,
        now: Timestamp,
    ) -> Result<RelayResponse, RelayProtocolError> {
        self.expire(now);
        match request {
            RelayRequest::Health => Ok(RelayResponse::Healthy),
            RelayRequest::Info => Ok(RelayResponse::Info(self.info.clone())),
            RelayRequest::Open {
                operation_id,
                code,
                expires_at,
                creator_blob,
                slot_capability,
                creator_token,
                ticket,
            } => {
                if let Some(response) = self.replayed(operation_id) {
                    return Ok(response);
                }
                validate_blob(&creator_blob)?;
                if expires_at <= now {
                    return Err(RelayProtocolError::SlotExpired);
                }
                if self.slots.contains_key(&code) {
                    return Err(RelayProtocolError::InvalidOperation);
                }
                if self.slots.len() >= self.max_slots {
                    return Err(RelayProtocolError::QueueFull);
                }
                let id = RelaySlotId(OpaqueId::from_u128(self.next_id));
                self.next_id =
                    self.next_id.checked_add(1).ok_or(RelayProtocolError::InvalidOperation)?;
                let relay_expires_at = now
                    .checked_add(PAIRING_SLOT_TTL)
                    .ok_or(RelayProtocolError::InvalidOperation)?;
                self.slots.insert(
                    code,
                    Slot {
                        id,
                        // The server owns expiry. The client-provided value
                        // only rejects already-expired requests above.
                        expires_at: relay_expires_at,
                        creator_blob,
                        slot_capability,
                        creator_token,
                        ticket,
                        joiner_token: None,
                        to_creator: VecDeque::new(),
                        to_joiner: VecDeque::new(),
                        next_to_creator_sequence: 1,
                        next_to_joiner_sequence: 1,
                    },
                );
                let response = RelayResponse::Opened { slot_id: id, expires_at: relay_expires_at };
                self.remember(operation_id, relay_expires_at, response.clone());
                Ok(response)
            }
            RelayRequest::Join { operation_id, code, joiner_blob, joiner_token, ticket } => {
                if let Some(response) = self.replayed(operation_id) {
                    return Ok(response);
                }
                validate_blob(&joiner_blob)?;
                if !self.slots.contains_key(&code) {
                    self.record_failed_join(now)?;
                    // Never distinguish invalid, expired and already-cleaned
                    // invitation codes to unauthenticated joiners.
                    return Err(RelayProtocolError::SlotNotFound);
                }
                let slot = self.slots.get_mut(&code).ok_or(RelayProtocolError::SlotNotFound)?;
                if slot.joiner_token.is_some() {
                    return Err(RelayProtocolError::SlotAlreadyJoined);
                }
                if joiner_token == slot.creator_token {
                    return Err(RelayProtocolError::Unauthorized);
                }
                if let Some(ticket) = ticket {
                    if ticket != slot.ticket {
                        return Err(RelayProtocolError::Unauthorized);
                    }
                }
                slot.joiner_token = Some(joiner_token);
                slot.enqueue(RelaySide::Creator, RelayMessageId(operation_id.0), joiner_blob)?;
                let expires_at = slot.expires_at;
                let response = RelayResponse::Joined {
                    slot_id: slot.id,
                    expires_at,
                    creator_blob: slot.creator_blob.clone(),
                };
                self.remember(operation_id, expires_at, response.clone());
                Ok(response)
            }
            RelayRequest::Push { operation_id, message_id, slot_id, token, blob } => {
                if let Some(response) = self.replayed(operation_id) {
                    return Ok(response);
                }
                validate_blob(&blob)?;
                let slot = self.find_mut(slot_id)?;
                let side = slot.authenticate(token)?;
                let receiving_side = match side {
                    RelaySide::Creator => RelaySide::Joiner,
                    RelaySide::Joiner => RelaySide::Creator,
                };
                slot.enqueue(receiving_side, message_id, blob)?;
                let expires_at = slot.expires_at;
                let response = RelayResponse::Accepted;
                self.remember(operation_id, expires_at, response.clone());
                Ok(response)
            }
            RelayRequest::Poll { slot_id, token, after } => {
                let slot = self.find_mut(slot_id)?;
                let side = slot.authenticate(token)?;
                let queue = match side {
                    RelaySide::Creator => &slot.to_creator,
                    RelaySide::Joiner => &slot.to_joiner,
                };
                let deliveries = queue
                    .iter()
                    .filter(|delivery| delivery.sequence > after)
                    .take(torca_relay_protocol::MAX_RELAY_BATCH_BLOBS)
                    .cloned()
                    .collect();
                Ok(RelayResponse::Deliveries(deliveries))
            }
            RelayRequest::Ack { slot_id, token, up_to } => {
                let slot = self.find_mut(slot_id)?;
                let side = slot.authenticate(token)?;
                let queue = match side {
                    RelaySide::Creator => &mut slot.to_creator,
                    RelaySide::Joiner => &mut slot.to_joiner,
                };
                while queue.front().is_some_and(|delivery| delivery.sequence <= up_to) {
                    queue.pop_front();
                }
                Ok(RelayResponse::Acked(up_to))
            }
            RelayRequest::Close { slot_id, capability } => {
                let code = self
                    .slots
                    .iter()
                    .find_map(|(code, slot)| {
                        (slot.id == slot_id && slot.slot_capability == capability)
                            .then(|| code.clone())
                    })
                    .ok_or(RelayProtocolError::Unauthorized)?;
                self.slots.remove(&code);
                Ok(RelayResponse::Closed)
            }
        }
    }

    /// Expires slots and returns the number removed.
    pub fn expire(&mut self, now: Timestamp) -> usize {
        let before = self.slots.len();
        self.slots.retain(|_, slot| slot.expires_at > now);
        self.operation_results.retain(|_, record| record.expires_at > now);
        before - self.slots.len()
    }

    /// Returns active slot count for health reporting.
    pub fn active_slots(&self) -> usize {
        self.slots.len()
    }

    /// Returns configured active-slot capacity.
    pub const fn max_slots(&self) -> usize {
        self.max_slots
    }

    fn find_mut(&mut self, id: RelaySlotId) -> Result<&mut Slot, RelayProtocolError> {
        self.slots.values_mut().find(|slot| slot.id == id).ok_or(RelayProtocolError::SlotNotFound)
    }

    fn replayed(&self, operation_id: RelayOperationId) -> Option<RelayResponse> {
        self.operation_results.get(&operation_id).map(|record| record.response.clone())
    }

    fn remember(
        &mut self,
        operation_id: RelayOperationId,
        expires_at: Timestamp,
        response: RelayResponse,
    ) {
        self.operation_results.insert(operation_id, OperationRecord { expires_at, response });
    }

    fn record_failed_join(&mut self, now: Timestamp) -> Result<(), RelayProtocolError> {
        let fresh_window = self.failed_join_window_started_at.is_none_or(|started_at| {
            now.duration_since(started_at)
                .is_none_or(|elapsed| elapsed >= std::time::Duration::from_secs(60))
        });
        if fresh_window {
            self.failed_join_window_started_at = Some(now);
            self.failed_join_attempts = 0;
        }
        if self.failed_join_attempts >= MAX_FAILED_JOINS_PER_MINUTE {
            return Err(RelayProtocolError::QueueFull);
        }
        self.failed_join_attempts = self.failed_join_attempts.saturating_add(1);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn broker_exposes_non_empty_build_metadata() {
        let response = RelayBroker::default()
            .handle(RelayRequest::Info, Timestamp::UNIX_EPOCH)
            .expect("relay info");
        let RelayResponse::Info(info) = response else {
            panic!("unexpected response: {response:?}");
        };
        assert!(!info.product_version.is_empty());
        assert!(!info.build_id.is_empty());
        assert!(!info.source_commit.is_empty());
        assert_eq!(info.protocol_version, RelayProtocolVersion::V4.0);
    }
}
