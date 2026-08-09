//! Durable delivery orchestration and strict encrypted application payloads.

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use torca_foundation::{CommandId, OpaqueId, Timestamp};
use torca_messaging::{Message, MessageDirection, MessageId, MessageStatus, RetryPolicy};

const PAYLOAD_MAGIC: &[u8; 4] = b"TCAP";
const PAYLOAD_VERSION: u16 = 1;
pub const MAX_TEXT_PAYLOAD_BODY: usize = 16 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutboxState {
    Pending,
    Claimed,
    Completed,
    DeadLetter,
}

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

pub trait InboundMessageStore {
    fn persist_inbound(
        &mut self,
        envelope_id: OpaqueId,
        message: Message,
    ) -> Result<bool, DurableDeliveryError>;
}

#[derive(Clone, Debug, Default)]
pub struct InMemoryDurableDeliveryStore {
    outbox: BTreeMap<MessageId, OutboxRecord>,
    inbound: BTreeSet<OpaqueId>,
    inbound_messages: BTreeMap<MessageId, Message>,
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
impl InboundMessageStore for InMemoryDurableDeliveryStore {
    fn persist_inbound(
        &mut self,
        envelope_id: OpaqueId,
        message: Message,
    ) -> Result<bool, DurableDeliveryError> {
        if message.direction() != MessageDirection::Inbound
            || message.status() != MessageStatus::Delivered
        {
            return Err(DurableDeliveryError::InvalidState);
        }
        if self.inbound.contains(&envelope_id) {
            return Ok(false);
        }
        if self.inbound_messages.contains_key(&message.id()) {
            return Err(DurableDeliveryError::DuplicateMessage);
        }
        self.inbound.insert(envelope_id);
        self.inbound_messages.insert(message.id(), message);
        Ok(true)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeliveryAck {
    Accepted,
    Duplicate,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeliveryTransportError(pub String);
impl fmt::Display for DeliveryTransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}
impl std::error::Error for DeliveryTransportError {}

pub trait DeliveryTransport {
    fn send(&mut self, message: &Message) -> Result<DeliveryAck, DeliveryTransportError>;
}

pub trait InboundAcknowledger {
    fn acknowledge(
        &mut self,
        envelope_id: OpaqueId,
        ack: DeliveryAck,
    ) -> Result<(), DeliveryTransportError>;
}

#[must_use]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DeliveryBatchReport {
    pub claimed: usize,
    pub completed: usize,
    pub rescheduled: usize,
    pub dead_lettered: usize,
}

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
    pub const fn new(store: S, transport: T, retry_policy: RetryPolicy) -> Self {
        Self { store, transport, retry_policy }
    }

    pub fn recover_stale_claims(
        &mut self,
        claimed_before: Timestamp,
    ) -> Result<usize, DeliveryWorkerError> {
        self.store.recover_stale_claims(claimed_before).map_err(Into::into)
    }

    #[allow(clippy::single_match_else)]
    pub fn run_once(
        &mut self,
        now: Timestamp,
        limit: usize,
    ) -> Result<DeliveryBatchReport, DeliveryWorkerError> {
        let records = self.store.claim_due(now, limit)?;
        let mut report = DeliveryBatchReport { claimed: records.len(), ..Default::default() };
        for record in records {
            let message_id = record.message.id();
            let attempts =
                record.attempts.checked_add(1).ok_or(DeliveryWorkerError::AttemptOverflow)?;
            match self.transport.send(&record.message) {
                Ok(DeliveryAck::Accepted | DeliveryAck::Duplicate) => {
                    self.store.complete(message_id)?;
                    report.completed += 1;
                }
                Err(_) => match self.retry_policy.delay_after(attempts) {
                    Some(delay) => {
                        let next =
                            now.checked_add(delay).ok_or(DeliveryWorkerError::TimestampOverflow)?;
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

    pub fn into_parts(self) -> (S, T) {
        (self.store, self.transport)
    }
}

pub struct InboundDeliveryHandler<S, A> {
    store: S,
    acknowledger: A,
}

impl<S, A> InboundDeliveryHandler<S, A>
where
    S: InboundMessageStore,
    A: InboundAcknowledger,
{
    pub const fn new(store: S, acknowledger: A) -> Self {
        Self { store, acknowledger }
    }

    pub fn handle(
        &mut self,
        envelope_id: OpaqueId,
        message: Message,
    ) -> Result<DeliveryAck, InboundDeliveryError> {
        let inserted = self.store.persist_inbound(envelope_id, message)?;
        let ack = if inserted { DeliveryAck::Accepted } else { DeliveryAck::Duplicate };
        self.acknowledger.acknowledge(envelope_id, ack).map_err(InboundDeliveryError::Ack)?;
        Ok(ack)
    }

    pub fn into_parts(self) -> (S, A) {
        (self.store, self.acknowledger)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InboundDeliveryError {
    Store(DurableDeliveryError),
    Ack(DeliveryTransportError),
}
impl fmt::Display for InboundDeliveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for InboundDeliveryError {}
impl From<DurableDeliveryError> for InboundDeliveryError {
    fn from(value: DurableDeliveryError) -> Self {
        Self::Store(value)
    }
}

/// Payload kind encrypted inside `PeerMessage::Data`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApplicationPayloadKind {
    Text,
    Receipt,
}

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextPayload {
    pub message_id: OpaqueId,
    pub conversation_id: OpaqueId,
    pub contact_id: OpaqueId,
    pub body: String,
    pub reply_to: Option<OpaqueId>,
    pub sent_at: Timestamp,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeliveryReceiptKind {
    Delivered,
    Read,
}

#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReceiptPayload {
    pub receipt_id: OpaqueId,
    pub message_id: OpaqueId,
    pub contact_id: OpaqueId,
    pub kind: DeliveryReceiptKind,
    pub at: Timestamp,
}

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ApplicationPayload {
    Text(TextPayload),
    Receipt(ReceiptPayload),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ApplicationPayloadError {
    InvalidMagic,
    UnsupportedVersion(u16),
    UnknownKind(u8),
    InvalidUtf8,
    EmptyBody,
    BodyTooLarge,
    InvalidTimestamp,
    InvalidReplyFlag,
    Truncated,
    TrailingBytes,
}
impl fmt::Display for ApplicationPayloadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for ApplicationPayloadError {}

pub struct ApplicationPayloadCodec;
impl ApplicationPayloadCodec {
    #[allow(clippy::cast_possible_truncation)]
    pub fn encode(payload: &ApplicationPayload) -> Result<Vec<u8>, ApplicationPayloadError> {
        let mut output = Vec::new();
        output.extend_from_slice(PAYLOAD_MAGIC);
        output.extend_from_slice(&PAYLOAD_VERSION.to_be_bytes());
        match payload {
            ApplicationPayload::Text(text) => {
                validate_text(text)?;
                output.push(1);
                output.extend_from_slice(text.message_id.as_bytes());
                output.extend_from_slice(text.conversation_id.as_bytes());
                output.extend_from_slice(text.contact_id.as_bytes());
                output.extend_from_slice(&text.sent_at.to_unix_millis().to_be_bytes());
                match text.reply_to {
                    Some(reply) => {
                        output.push(1);
                        output.extend_from_slice(reply.as_bytes());
                    }
                    None => output.push(0),
                }
                let body = text.body.as_bytes();
                output.extend_from_slice(&(body.len() as u32).to_be_bytes());
                output.extend_from_slice(body);
            }
            ApplicationPayload::Receipt(receipt) => {
                output.push(2);
                output.extend_from_slice(receipt.receipt_id.as_bytes());
                output.extend_from_slice(receipt.message_id.as_bytes());
                output.extend_from_slice(receipt.contact_id.as_bytes());
                output.push(match receipt.kind {
                    DeliveryReceiptKind::Delivered => 1,
                    DeliveryReceiptKind::Read => 2,
                });
                output.extend_from_slice(&receipt.at.to_unix_millis().to_be_bytes());
            }
        }
        Ok(output)
    }

    pub fn decode(input: &[u8]) -> Result<ApplicationPayload, ApplicationPayloadError> {
        let mut cursor = PayloadCursor::new(input);
        if cursor.take(4)? != PAYLOAD_MAGIC {
            return Err(ApplicationPayloadError::InvalidMagic);
        }
        let version = cursor.u16()?;
        if version != PAYLOAD_VERSION {
            return Err(ApplicationPayloadError::UnsupportedVersion(version));
        }
        let payload = match cursor.u8()? {
            1 => {
                let message_id = cursor.id()?;
                let conversation_id = cursor.id()?;
                let contact_id = cursor.id()?;
                let sent_at = timestamp(cursor.i64()?)?;
                let reply_to = match cursor.u8()? {
                    0 => None,
                    1 => Some(cursor.id()?),
                    _ => return Err(ApplicationPayloadError::InvalidReplyFlag),
                };
                let body_len = usize::try_from(cursor.u32()?)
                    .map_err(|_| ApplicationPayloadError::BodyTooLarge)?;
                if body_len > MAX_TEXT_PAYLOAD_BODY {
                    return Err(ApplicationPayloadError::BodyTooLarge);
                }
                let body = String::from_utf8(cursor.take(body_len)?.to_vec())
                    .map_err(|_| ApplicationPayloadError::InvalidUtf8)?;
                let text = TextPayload {
                    message_id,
                    conversation_id,
                    contact_id,
                    body,
                    reply_to,
                    sent_at,
                };
                validate_text(&text)?;
                ApplicationPayload::Text(text)
            }
            2 => {
                let receipt_id = cursor.id()?;
                let message_id = cursor.id()?;
                let contact_id = cursor.id()?;
                let kind = match cursor.u8()? {
                    1 => DeliveryReceiptKind::Delivered,
                    2 => DeliveryReceiptKind::Read,
                    value => return Err(ApplicationPayloadError::UnknownKind(value)),
                };
                let at = timestamp(cursor.i64()?)?;
                ApplicationPayload::Receipt(ReceiptPayload {
                    receipt_id,
                    message_id,
                    contact_id,
                    kind,
                    at,
                })
            }
            value => return Err(ApplicationPayloadError::UnknownKind(value)),
        };
        if !cursor.is_empty() {
            return Err(ApplicationPayloadError::TrailingBytes);
        }
        Ok(payload)
    }
}

fn validate_text(text: &TextPayload) -> Result<(), ApplicationPayloadError> {
    if text.body.is_empty() {
        return Err(ApplicationPayloadError::EmptyBody);
    }
    if text.body.len() > MAX_TEXT_PAYLOAD_BODY || text.body.contains('\0') {
        return Err(ApplicationPayloadError::BodyTooLarge);
    }
    Ok(())
}

fn timestamp(value: i64) -> Result<Timestamp, ApplicationPayloadError> {
    Timestamp::from_unix_millis(value).map_err(|_| ApplicationPayloadError::InvalidTimestamp)
}

struct PayloadCursor<'a> {
    input: &'a [u8],
    offset: usize,
}
impl<'a> PayloadCursor<'a> {
    const fn new(input: &'a [u8]) -> Self {
        Self { input, offset: 0 }
    }
    fn take(&mut self, len: usize) -> Result<&'a [u8], ApplicationPayloadError> {
        let end = self.offset.checked_add(len).ok_or(ApplicationPayloadError::Truncated)?;
        let value = self.input.get(self.offset..end).ok_or(ApplicationPayloadError::Truncated)?;
        self.offset = end;
        Ok(value)
    }
    fn u8(&mut self) -> Result<u8, ApplicationPayloadError> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, ApplicationPayloadError> {
        Ok(u16::from_be_bytes(
            self.take(2)?.try_into().map_err(|_| ApplicationPayloadError::Truncated)?,
        ))
    }
    fn u32(&mut self) -> Result<u32, ApplicationPayloadError> {
        Ok(u32::from_be_bytes(
            self.take(4)?.try_into().map_err(|_| ApplicationPayloadError::Truncated)?,
        ))
    }
    fn i64(&mut self) -> Result<i64, ApplicationPayloadError> {
        Ok(i64::from_be_bytes(
            self.take(8)?.try_into().map_err(|_| ApplicationPayloadError::Truncated)?,
        ))
    }
    fn id(&mut self) -> Result<OpaqueId, ApplicationPayloadError> {
        Ok(OpaqueId::from_bytes(
            self.take(16)?.try_into().map_err(|_| ApplicationPayloadError::Truncated)?,
        ))
    }
    const fn is_empty(&self) -> bool {
        self.offset == self.input.len()
    }
}
