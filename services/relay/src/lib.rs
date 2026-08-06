//! Ephemeral in-memory rendezvous broker. It owns no user database or offline mailbox.

use std::collections::{BTreeMap, VecDeque};

use torca_foundation::{OpaqueId, Timestamp};
use torca_relay_protocol::{
    RelayCode, RelayProtocolError, RelayRequest, RelayResponse, RelaySide, RelaySlotId,
    validate_blob,
};

const MAX_QUEUED_BLOBS_PER_SIDE: usize = 32;

#[derive(Clone, Debug)]
struct Slot {
    id: RelaySlotId,
    expires_at: Timestamp,
    creator_blob: Vec<u8>,
    joined: bool,
    to_creator: VecDeque<Vec<u8>>,
    to_joiner: VecDeque<Vec<u8>>,
}

/// Deterministic ephemeral relay broker.
#[derive(Clone, Debug)]
pub struct RelayBroker {
    slots: BTreeMap<RelayCode, Slot>,
    next_id: u128,
}
impl Default for RelayBroker {
    fn default() -> Self {
        Self { slots: BTreeMap::new(), next_id: 1 }
    }
}

impl RelayBroker {
    /// Applies one request at a trusted clock value.
    pub fn handle(
        &mut self,
        request: RelayRequest,
        now: Timestamp,
    ) -> Result<RelayResponse, RelayProtocolError> {
        self.expire(now);
        match request {
            RelayRequest::Open { code, expires_at, creator_blob } => {
                validate_blob(&creator_blob)?;
                if expires_at <= now {
                    return Err(RelayProtocolError::SlotExpired);
                }
                if self.slots.contains_key(&code) {
                    return Err(RelayProtocolError::InvalidOperation);
                }
                let id = RelaySlotId(OpaqueId::from_u128(self.next_id));
                self.next_id =
                    self.next_id.checked_add(1).ok_or(RelayProtocolError::InvalidOperation)?;
                self.slots.insert(
                    code,
                    Slot {
                        id,
                        expires_at,
                        creator_blob,
                        joined: false,
                        to_creator: VecDeque::new(),
                        to_joiner: VecDeque::new(),
                    },
                );
                Ok(RelayResponse::Opened { slot_id: id })
            }
            RelayRequest::Join { code, joiner_blob } => {
                validate_blob(&joiner_blob)?;
                let slot = self.slots.get_mut(&code).ok_or(RelayProtocolError::SlotNotFound)?;
                if slot.joined {
                    return Err(RelayProtocolError::SlotAlreadyJoined);
                }
                slot.joined = true;
                slot.to_creator.push_back(joiner_blob);
                Ok(RelayResponse::Joined {
                    slot_id: slot.id,
                    creator_blob: slot.creator_blob.clone(),
                })
            }
            RelayRequest::Push { slot_id, side, blob } => {
                validate_blob(&blob)?;
                let slot = self.find_mut(slot_id)?;
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
            RelayRequest::Poll { slot_id, side } => {
                let slot = self.find_mut(slot_id)?;
                let queue = match side {
                    RelaySide::Creator => &mut slot.to_creator,
                    RelaySide::Joiner => &mut slot.to_joiner,
                };
                Ok(RelayResponse::Blobs(queue.drain(..).collect()))
            }
            RelayRequest::Close { slot_id } => {
                let code = self
                    .slots
                    .iter()
                    .find_map(|(code, slot)| (slot.id == slot_id).then(|| code.clone()))
                    .ok_or(RelayProtocolError::SlotNotFound)?;
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
    fn find_mut(&mut self, id: RelaySlotId) -> Result<&mut Slot, RelayProtocolError> {
        self.slots.values_mut().find(|slot| slot.id == id).ok_or(RelayProtocolError::SlotNotFound)
    }
}
