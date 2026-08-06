//! Idempotent delivery and read receipt domain.

use core::fmt;
use std::collections::BTreeMap;

use torca_foundation::{OpaqueId, Timestamp};
use torca_messaging::{Message, MessageError, MessageId};

/// Receipt ID.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ReceiptId(OpaqueId);
impl ReceiptId { /// Creates an ID.
    pub const fn from_u128(value: u128) -> Self { Self(OpaqueId::from_u128(value)) } }
/// Receipt semantic kind.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ReceiptKind { Delivered, Read }
/// Immutable receipt fact.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Receipt { pub id: ReceiptId, pub message_id: MessageId, pub kind: ReceiptKind, pub at: Timestamp }
impl Receipt {
    /// Applies the fact monotonically and reports whether state changed.
    pub fn apply(self, message: &mut Message) -> Result<bool, ReceiptError> {
        if message.id() != self.message_id { return Err(ReceiptError::MessageMismatch); }
        match self.kind { ReceiptKind::Delivered => message.mark_delivered(self.at), ReceiptKind::Read => message.mark_read(self.at) }.map_err(ReceiptError::Message)
    }
}
/// Receipt error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReceiptError { MessageMismatch, Message(MessageError), AlreadyExists }
impl fmt::Display for ReceiptError { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{self:?}") } }
impl std::error::Error for ReceiptError {}
/// Receipt repository port.
pub trait ReceiptRepository {
    /// Records a receipt exactly once by message and kind.
    fn record(&mut self, receipt: Receipt) -> Result<bool, ReceiptError>;
    /// Lists receipts for a message.
    fn for_message(&self, message_id: MessageId) -> Result<Vec<Receipt>, ReceiptError>;
}
/// In-memory idempotent receipt repository.
#[derive(Clone, Debug, Default)]
pub struct InMemoryReceiptRepository { receipts: BTreeMap<(MessageId, ReceiptKind), Receipt> }
impl ReceiptRepository for InMemoryReceiptRepository {
    fn record(&mut self, receipt: Receipt) -> Result<bool, ReceiptError> { Ok(self.receipts.insert((receipt.message_id, receipt.kind), receipt).is_none()) }
    fn for_message(&self, message_id: MessageId) -> Result<Vec<Receipt>, ReceiptError> { Ok(self.receipts.iter().filter_map(|((id, _), receipt)| (*id == message_id).then_some(*receipt)).collect()) }
}
