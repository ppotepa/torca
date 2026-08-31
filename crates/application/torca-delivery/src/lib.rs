//! Durable delivery orchestration and strict encrypted application payloads.

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use torca_foundation::{CommandId, OpaqueId, Timestamp};
use torca_messaging::{
    Message, MessageBody, MessageDirection, MessageId, MessageStatus, RetryPolicy,
};

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

    /// Returns the next persisted delivery deadline without claiming work.
    /// `None` means that this store has no pending retry deadline.
    fn next_due(&self) -> Result<Option<Timestamp>, DurableDeliveryError> {
        Ok(None)
    }
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

    fn next_due(&self) -> Result<Option<Timestamp>, DurableDeliveryError> {
        Ok(self
            .outbox
            .values()
            .filter(|record| record.state == OutboxState::Pending)
            .map(|record| record.next_attempt_at)
            .min())
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

    /// Sends a group of claimed messages while preserving one result per
    /// message. The default is deliberately conservative; optimized
    /// transports can override this to coalesce provider writes.
    fn send_batch(
        &mut self,
        messages: &[&Message],
    ) -> Vec<Result<DeliveryAck, DeliveryTransportError>> {
        messages.iter().map(|message| self.send(message)).collect()
    }
}

pub trait InboundAcknowledger {
    fn acknowledge(
        &mut self,
        envelope_id: OpaqueId,
        ack: DeliveryAck,
    ) -> Result<(), DeliveryTransportError>;
}

#[must_use]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DeliveryBatchReport {
    pub claimed: usize,
    pub completed: usize,
    pub rescheduled: usize,
    pub dead_lettered: usize,
    /// Message ids whose durable outbox state reached a terminal result in
    /// this batch. Adapters use these ids to reconcile the domain message
    /// status without coupling the generic worker to the client engine.
    pub completed_message_ids: Vec<MessageId>,
    pub dead_lettered_message_ids: Vec<MessageId>,
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

    /// Adds a newly-created outbound message to the durable delivery outbox.
    pub fn queue_outbound(
        &mut self,
        message: Message,
        command_id: CommandId,
        next_attempt_at: Timestamp,
    ) -> Result<(), DeliveryWorkerError> {
        self.store.queue_outbound(message, command_id, next_attempt_at).map_err(Into::into)
    }

    pub fn next_due(&self) -> Result<Option<Timestamp>, DeliveryWorkerError> {
        self.store.next_due().map_err(Into::into)
    }

    #[allow(clippy::single_match_else)]
    pub fn run_once(
        &mut self,
        now: Timestamp,
        limit: usize,
    ) -> Result<DeliveryBatchReport, DeliveryWorkerError> {
        self.run_once_with_observer(now, limit, |_| {})
    }

    /// Runs one batch and invokes `before_send` immediately after claiming a
    /// message but before bytes leave the process. This lets an adapter
    /// reconcile the domain `Queued -> Sending` transition before an inbound
    /// Delivered receipt can race the transport result.
    pub fn run_once_with_observer<F>(
        &mut self,
        now: Timestamp,
        limit: usize,
        mut before_send: F,
    ) -> Result<DeliveryBatchReport, DeliveryWorkerError>
    where
        F: FnMut(&Message),
    {
        let records = self.store.claim_due(now, limit)?;
        let mut report = DeliveryBatchReport { claimed: records.len(), ..Default::default() };
        for record in &records {
            before_send(&record.message);
        }
        let messages = records.iter().map(|record| &record.message).collect::<Vec<_>>();
        let results = self.transport.send_batch(&messages);
        let results = if results.len() == records.len() {
            results
        } else {
            // A custom transport must return one result per claimed message;
            // treat a malformed response as a retryable transport failure
            // rather than silently dropping durable jobs.
            vec![Err(DeliveryTransportError("batch result length mismatch".into())); records.len()]
        };
        for (record, result) in records.into_iter().zip(results) {
            let message_id = record.message.id();
            let attempts =
                record.attempts.checked_add(1).ok_or(DeliveryWorkerError::AttemptOverflow)?;
            match result {
                Ok(DeliveryAck::Accepted | DeliveryAck::Duplicate) => {
                    self.store.complete(message_id)?;
                    report.completed += 1;
                    report.completed_message_ids.push(message_id);
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
                        report.dead_lettered_message_ids.push(message_id);
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
    Reaction,
    MessageDeletion,
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
pub struct ReactionPayload {
    pub reaction_id: OpaqueId,
    pub message_id: OpaqueId,
    pub conversation_id: OpaqueId,
    pub actor_id: OpaqueId,
    pub emoji: String,
    pub active: bool,
    pub at: Timestamp,
}
#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MessageDeletionPayload {
    pub message_id: OpaqueId,
    pub conversation_id: OpaqueId,
    pub contact_id: OpaqueId,
    pub at: Timestamp,
}

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ApplicationPayload {
    Text(TextPayload),
    Receipt(ReceiptPayload),
    Reaction(ReactionPayload),
    MessageDeletion(MessageDeletionPayload),
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
            ApplicationPayload::Reaction(reaction) => {
                if reaction.emoji.is_empty()
                    || reaction.emoji.len() > 32
                    || reaction.emoji.contains('\0')
                {
                    return Err(ApplicationPayloadError::BodyTooLarge);
                }
                output.push(3);
                output.extend_from_slice(reaction.reaction_id.as_bytes());
                output.extend_from_slice(reaction.message_id.as_bytes());
                output.extend_from_slice(reaction.conversation_id.as_bytes());
                output.extend_from_slice(reaction.actor_id.as_bytes());
                output.push(u8::from(reaction.active));
                output.extend_from_slice(&reaction.at.to_unix_millis().to_be_bytes());
                let emoji = reaction.emoji.as_bytes();
                output.extend_from_slice(&(emoji.len() as u32).to_be_bytes());
                output.extend_from_slice(emoji);
            }
            ApplicationPayload::MessageDeletion(deletion) => {
                output.push(4);
                output.extend_from_slice(deletion.message_id.as_bytes());
                output.extend_from_slice(deletion.conversation_id.as_bytes());
                output.extend_from_slice(deletion.contact_id.as_bytes());
                output.extend_from_slice(&deletion.at.to_unix_millis().to_be_bytes());
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
            3 => {
                let reaction_id = cursor.id()?;
                let message_id = cursor.id()?;
                let conversation_id = cursor.id()?;
                let actor_id = cursor.id()?;
                let active = match cursor.u8()? {
                    0 => false,
                    1 => true,
                    _ => return Err(ApplicationPayloadError::InvalidReplyFlag),
                };
                let at = timestamp(cursor.i64()?)?;
                let emoji_len = usize::try_from(cursor.u32()?)
                    .map_err(|_| ApplicationPayloadError::BodyTooLarge)?;
                if emoji_len == 0 || emoji_len > 32 {
                    return Err(ApplicationPayloadError::BodyTooLarge);
                }
                let emoji = String::from_utf8(cursor.take(emoji_len)?.to_vec())
                    .map_err(|_| ApplicationPayloadError::InvalidUtf8)?;
                ApplicationPayload::Reaction(ReactionPayload {
                    reaction_id,
                    message_id,
                    conversation_id,
                    actor_id,
                    emoji,
                    active,
                    at,
                })
            }
            4 => ApplicationPayload::MessageDeletion(MessageDeletionPayload {
                message_id: cursor.id()?,
                conversation_id: cursor.id()?,
                contact_id: cursor.id()?,
                at: timestamp(cursor.i64()?)?,
            }),
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
    if text.body.len() > MAX_TEXT_PAYLOAD_BODY
        || text.body.chars().count() > MessageBody::MAX_CHARACTERS
        || text.body.contains('\0')
    {
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

#[cfg(test)]
mod tests {
    use super::{ApplicationPayload, ApplicationPayloadCodec, MessageDeletionPayload};
    use torca_foundation::{OpaqueId, Timestamp};

    #[test]
    fn message_deletion_tombstone_round_trips() {
        let payload = ApplicationPayload::MessageDeletion(MessageDeletionPayload {
            message_id: OpaqueId::from_u128(1),
            conversation_id: OpaqueId::from_u128(2),
            contact_id: OpaqueId::from_u128(3),
            at: Timestamp::from_unix_millis(42).expect("timestamp"),
        });
        let encoded = ApplicationPayloadCodec::encode(&payload).expect("encode");
        assert_eq!(ApplicationPayloadCodec::decode(&encoded).expect("decode"), payload);
    }
}
