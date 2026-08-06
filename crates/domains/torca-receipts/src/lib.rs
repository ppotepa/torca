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
