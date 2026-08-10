//! Idempotent delivery and read receipt domain.

use core::fmt;
use std::collections::{BTreeMap, btree_map::Entry};
use torca_foundation::{OpaqueId, Timestamp};
use torca_messaging::{Message, MessageError, MessageId};
#[must_use]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ReceiptId(OpaqueId);
impl ReceiptId {
    pub const fn from_opaque(value: OpaqueId) -> Self {
        Self(value)
    }
    pub const fn from_u128(value: u128) -> Self {
        Self(OpaqueId::from_u128(value))
    }
    pub const fn to_opaque(self) -> OpaqueId {
        self.0
    }

    /// Derives the idempotency identifier for one receipt kind of a message.
    ///
    /// The derivation is deliberately owned by the receipt domain so storage and transport use
    /// the same stable identifier when they independently persist or send a receipt.
    pub fn deterministic_for(message_id: MessageId, kind: ReceiptKind) -> Self {
        let tag = match kind {
            ReceiptKind::Delivered => 0xD1,
            ReceiptKind::Read => 0xA1,
        };
        let mut bytes = message_id.to_opaque().into_bytes();
        bytes[15] ^= tag;
        let value = OpaqueId::from_bytes(bytes);
        Self(if value.is_nil() { OpaqueId::from_u128(u128::from(tag) + 1) } else { value })
    }
}
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ReceiptKind {
    Delivered,
    Read,
}
#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Receipt {
    pub id: ReceiptId,
    pub message_id: MessageId,
    pub kind: ReceiptKind,
    pub at: Timestamp,
}
impl Receipt {
    pub fn apply(self, message: &mut Message) -> Result<bool, ReceiptError> {
        if message.id() != self.message_id {
            return Err(ReceiptError::MessageMismatch);
        }
        match self.kind {
            ReceiptKind::Delivered => message.mark_delivered(self.at),
            ReceiptKind::Read => message.mark_read(self.at),
        }
        .map_err(ReceiptError::Message)
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReceiptError {
    MessageMismatch,
    Message(MessageError),
    /// Persistence dependency failed without exposing implementation details.
    RepositoryFailure,
}
impl fmt::Display for ReceiptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for ReceiptError {}
pub trait ReceiptRepository {
    fn record(&mut self, receipt: Receipt) -> Result<bool, ReceiptError>;
    fn for_message(&self, message_id: MessageId) -> Result<Vec<Receipt>, ReceiptError>;
}
#[derive(Clone, Debug, Default)]
pub struct InMemoryReceiptRepository {
    receipts: BTreeMap<(MessageId, ReceiptKind), Receipt>,
}
impl ReceiptRepository for InMemoryReceiptRepository {
    fn record(&mut self, receipt: Receipt) -> Result<bool, ReceiptError> {
        match self.receipts.entry((receipt.message_id, receipt.kind)) {
            Entry::Vacant(entry) => {
                entry.insert(receipt);
                Ok(true)
            }
            Entry::Occupied(_) => Ok(false),
        }
    }
    fn for_message(&self, message_id: MessageId) -> Result<Vec<Receipt>, ReceiptError> {
        Ok(self
            .receipts
            .iter()
            .filter_map(|((id, _), receipt)| (*id == message_id).then_some(*receipt))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::{ReceiptId, ReceiptKind};
    use torca_messaging::MessageId;

    #[test]
    fn deterministic_ids_are_stable_and_separate_kinds() {
        let message = MessageId::from_u128(42);
        assert_eq!(
            ReceiptId::deterministic_for(message, ReceiptKind::Delivered),
            ReceiptId::deterministic_for(message, ReceiptKind::Delivered)
        );
        assert_ne!(
            ReceiptId::deterministic_for(message, ReceiptKind::Delivered),
            ReceiptId::deterministic_for(message, ReceiptKind::Read)
        );
    }
}
