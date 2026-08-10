//! Ephemeral in-memory rendezvous broker. It owns no user database or offline mailbox.

mod server;

use std::collections::{BTreeMap, VecDeque};

use torca_foundation::{OpaqueId, Timestamp};
use torca_relay_protocol::{
    RelayCode, RelayProtocolError, RelayRequest, RelayResponse, RelaySide, RelaySideToken,
    RelaySlotCapability, RelaySlotId, validate_blob,
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
    joiner_token: Option<RelaySideToken>,
    to_creator: VecDeque<Vec<u8>>,
    to_joiner: VecDeque<Vec<u8>>,
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
}

/// Deterministic ephemeral relay broker.
#[derive(Clone, Debug)]
pub struct RelayBroker {
    slots: BTreeMap<RelayCode, Slot>,
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
        Self {
            slots: BTreeMap::new(),
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
            RelayRequest::Open {
                code,
                expires_at,
                creator_blob,
                slot_capability,
                creator_token,
            } => {
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
                        joiner_token: None,
                        to_creator: VecDeque::new(),
                        to_joiner: VecDeque::new(),
                    },
                );
                Ok(RelayResponse::Opened { slot_id: id, expires_at: relay_expires_at })
            }
            RelayRequest::Join { code, joiner_blob, joiner_token } => {
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
                slot.joiner_token = Some(joiner_token);
                slot.to_creator.push_back(joiner_blob);
                Ok(RelayResponse::Joined {
                    slot_id: slot.id,
                    expires_at: slot.expires_at,
                    creator_blob: slot.creator_blob.clone(),
                })
            }
            RelayRequest::Push { slot_id, token, blob } => {
                validate_blob(&blob)?;
                let slot = self.find_mut(slot_id)?;
                let side = slot.authenticate(token)?;
                let queue = match side {
                    RelaySide::Creator => &mut slot.to_joiner,
                    RelaySide::Joiner => &mut slot.to_creator,
                };
                if queue.len() >= MAX_QUEUED_BLOBS_PER_SIDE {
                    return Err(RelayProtocolError::QueueFull);
                }
                queue.push_back(blob);
                Ok(RelayResponse::Accepted)
            }
            RelayRequest::Poll { slot_id, token } => {
                let slot = self.find_mut(slot_id)?;
                let side = slot.authenticate(token)?;
                let queue = match side {
                    RelaySide::Creator => &mut slot.to_creator,
                    RelaySide::Joiner => &mut slot.to_joiner,
                };
                Ok(RelayResponse::Blobs(queue.drain(..).collect()))
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
