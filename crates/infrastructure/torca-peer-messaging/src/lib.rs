//! Encrypted text/receipt adapters over the single shared PeerLink.

use core::fmt;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use torca_client_engine::{EngineCommand, EngineHandle};
use torca_contacts::{
    Contact, ContactId, ContactRepository, PeerCredential, PeerCredentialRepository,
};
use torca_control_delivery::{
    ControlAck, ControlBatchReport, ControlDeliveryError, ControlDeliveryWorker, ControlJob,
    ControlKind, ControlTransport, ControlTransportError,
};
use torca_conversations::{ConversationRepository, DirectConversation};
use torca_crypto::{
    Ciphertext, CryptoProvider, ManagedPeerSecrets, Nonce, ProtectedSecretStore,
};
use torca_delivery::{
    ApplicationPayload, ApplicationPayloadCodec, DeliveryAck, DeliveryReceiptKind,
    DeliveryTransport, DeliveryTransportError, DurableDeliveryError, InboundMessageStore,
    ReceiptPayload, TextPayload,
};
use torca_foundation::{OpaqueId, Timestamp};
use torca_messaging::{Message, MessageBody, MessageId, ReplyReference};
use torca_peer_link::{LinkAck, PeerLinkError};
use torca_peer_protocol::{AckStatus, HandshakeSigner};
use torca_peer_shared::SharedPeerLink;
use torca_receipts::{Receipt, ReceiptId, ReceiptKind};

pub const TEXT_MESSAGE_KIND: u16 = 1;
pub const RECEIPT_MESSAGE_KIND: u16 = 2;
const NONCE_BYTES: usize = 24;
const AAD_LABEL: &[u8] = b"TORCA-PEER-DATA-V1";

pub struct SharedPeerCipher<C, P> {
    inner: Arc<Mutex<ManagedPeerSecrets<C, P>>>,
}
impl<C, P> Clone for SharedPeerCipher<C, P> {
    fn clone(&self) -> Self {
        Self { inner: Arc::clone(&self.inner) }
    }
}
impl<C, P> SharedPeerCipher<C, P> {
    pub fn new(secrets: ManagedPeerSecrets<C, P>) -> Self {
        Self { inner: Arc::new(Mutex::new(secrets)) }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PeerMessagingError {
    Relationship,
    Peer,
    Crypto,
    Protocol,
    Store,
    Engine,
    Control,
    ConversationMissing,
    InvalidCiphertext,
}
impl fmt::Display for PeerMessagingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for PeerMessagingError {}

pub struct TextPeerTransport<R, S, K, C, P> {
    relationships: R,
    link: SharedPeerLink<S, K>,
    cipher: SharedPeerCipher<C, P>,
    local_identity_id: OpaqueId,
    ack_timeout: Duration,
}
impl<R, S, K, C, P> TextPeerTransport<R, S, K, C, P> {
    pub const fn new(
        relationships: R,
        link: SharedPeerLink<S, K>,
        cipher: SharedPeerCipher<C, P>,
        local_identity_id: OpaqueId,
        ack_timeout: Duration,
    ) -> Self {
        Self { relationships, link, cipher, local_identity_id, ack_timeout }
    }
}
impl<R, S, K, C, P> DeliveryTransport for TextPeerTransport<R, S, K, C, P>
where
    R: ContactRepository + ConversationRepository + PeerCredentialRepository,
    S: ContactRepository + PeerCredentialRepository,
    K: HandshakeSigner,
    C: CryptoProvider,
    P: ProtectedSecretStore,
{
    fn send(&mut self, message: &Message) -> Result<DeliveryAck, DeliveryTransportError> {
        self.send_text(message)
            .map_err(|error| DeliveryTransportError(error.to_string()))
    }
}
impl<R, S, K, C, P> TextPeerTransport<R, S, K, C, P>
where
    R: ContactRepository + ConversationRepository + PeerCredentialRepository,
    S: ContactRepository + PeerCredentialRepository,
    K: HandshakeSigner,
    C: CryptoProvider,
    P: ProtectedSecretStore,
{
    fn send_text(&mut self, message: &Message) -> Result<DeliveryAck, PeerMessagingError> {
        let conversation = self
            .relationships
            .get(message.conversation_id())
            .map_err(|_| PeerMessagingError::Relationship)?
            .ok_or(PeerMessagingError::ConversationMissing)?;
        let contact = load_contact(&self.relationships, conversation.contact_id())?;
        let credential = load_credential(&self.relationships, contact.id())?;
        let payload = ApplicationPayload::Text(TextPayload {
            message_id: message.id().to_opaque(),
            conversation_id: message.conversation_id().to_opaque(),
            contact_id: contact.id().to_opaque(),
            body: message.body().as_str().to_owned(),
            reply_to: message.reply_to().map(|reply| reply.message_id.to_opaque()),
            sent_at: message.created_at(),
        });
        let plaintext = ApplicationPayloadCodec::encode(&payload)
            .map_err(|_| PeerMessagingError::Protocol)?;
        let encrypted = seal_payload(
            &self.cipher,
            credential.secret_handle(),
            message.id().to_opaque(),
            TEXT_MESSAGE_KIND,
            self.local_identity_id,
            contact.remote_identity().identity_id().to_opaque(),
            &plaintext,
        )?;
        let ack = self
            .link
            .send_and_wait_ack(
                contact.id(),
                message.id().to_opaque(),
                TEXT_MESSAGE_KIND,
                encrypted,
                self.ack_timeout,
            )
            .map_err(map_peer)?;
        Ok(map_link_ack(ack))
    }
}

pub struct ControlPeerTransport<R, S, K, C, P> {
    relationships: R,
    link: SharedPeerLink<S, K>,
    cipher: SharedPeerCipher<C, P>,
    local_identity_id: OpaqueId,
    ack_timeout: Duration,
}
impl<R, S, K, C, P> ControlPeerTransport<R, S, K, C, P> {
    pub const fn new(
        relationships: R,
        link: SharedPeerLink<S, K>,
        cipher: SharedPeerCipher<C, P>,
        local_identity_id: OpaqueId,
        ack_timeout: Duration,
    ) -> Self {
        Self { relationships, link, cipher, local_identity_id, ack_timeout }
    }
}
impl<R, S, K, C, P> ControlTransport for ControlPeerTransport<R, S, K, C, P>
where
    R: ContactRepository + PeerCredentialRepository,
    S: ContactRepository + PeerCredentialRepository,
    K: HandshakeSigner,
    C: CryptoProvider,
    P: ProtectedSecretStore,
{
    fn send_control(&mut self, job: &ControlJob) -> Result<ControlAck, ControlTransportError> {
        self.send_job(job).map_err(|_| ControlTransportError)
    }
}
impl<R, S, K, C, P> ControlPeerTransport<R, S, K, C, P>
where
    R: ContactRepository + PeerCredentialRepository,
    S: ContactRepository + PeerCredentialRepository,
    K: HandshakeSigner,
    C: CryptoProvider,
    P: ProtectedSecretStore,
{
    fn send_job(&mut self, job: &ControlJob) -> Result<ControlAck, PeerMessagingError> {
        if job.kind != ControlKind::Receipt {
            return Err(PeerMessagingError::Protocol);
        }
        let contact_id = ContactId::from_opaque(job.contact_id);
        let contact = load_contact(&self.relationships, contact_id)?;
        let credential = load_credential(&self.relationships, contact_id)?;
        let encrypted = seal_payload(
            &self.cipher,
            credential.secret_handle(),
            job.job_id,
            RECEIPT_MESSAGE_KIND,
            self.local_identity_id,
            contact.remote_identity().identity_id().to_opaque(),
            &job.payload,
        )?;
        let ack = self
            .link
            .send_and_wait_ack(
                contact_id,
                job.job_id,
                RECEIPT_MESSAGE_KIND,
                encrypted,
                self.ack_timeout,
            )
            .map_err(map_peer)?;
        Ok(match ack {
            LinkAck::Accepted => ControlAck::Accepted,
            LinkAck::Duplicate => ControlAck::Duplicate,
        })
    }
}

pub struct SharedControlQueue<T> {
    inner: Arc<Mutex<ControlDeliveryWorker<T>>>,
}
impl<T> Clone for SharedControlQueue<T> {
    fn clone(&self) -> Self {
        Self { inner: Arc::clone(&self.inner) }
    }
}
impl<T> SharedControlQueue<T> {
    pub fn new(worker: ControlDeliveryWorker<T>) -> Self {
        Self { inner: Arc::new(Mutex::new(worker)) }
    }
}
impl<T: ControlTransport> SharedControlQueue<T> {
    pub fn recover_stale(&self, before: Timestamp) -> Result<usize, PeerMessagingError> {
        self.inner
            .lock()
            .map_err(|_| PeerMessagingError::Control)?
            .recover_stale(before)
            .map_err(map_control)
    }

    pub fn maintenance(
        &self,
        now: Timestamp,
        limit: usize,
    ) -> Result<ControlBatchReport, PeerMessagingError> {
        self.inner
            .lock()
            .map_err(|_| PeerMessagingError::Control)?
            .run_once(now, limit)
            .map_err(map_control)
    }

    pub fn ensure_receipt(
        &self,
        contact_id: ContactId,
        receipt: ReceiptPayload,
        now: Timestamp,
    ) -> Result<(), PeerMessagingError> {
        let payload = ApplicationPayloadCodec::encode(&ApplicationPayload::Receipt(receipt))
            .map_err(|_| PeerMessagingError::Protocol)?;
        let result = self
            .inner
            .lock()
            .map_err(|_| PeerMessagingError::Control)?
            .queue(
                receipt.receipt_id,
                contact_id.to_opaque(),
                ControlKind::Receipt,
                &payload,
                now,
            );
        match result {
            Ok(()) | Err(ControlDeliveryError::Duplicate) => Ok(()),
            Err(error) => Err(map_control(error)),
        }
    }
}

#[must_use]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct InboundProcessReport {
    pub processed: usize,
    pub texts_inserted: usize,
    pub duplicates: usize,
    pub receipts_applied: usize,
}

pub struct InboundPeerProcessor<R, S, K, C, P, I, T> {
    relationships: R,
    link: SharedPeerLink<S, K>,
    cipher: SharedPeerCipher<C, P>,
    inbound: I,
    receipts: SharedControlQueue<T>,
    engine: EngineHandle,
    local_identity_id: OpaqueId,
}
impl<R, S, K, C, P, I, T> InboundPeerProcessor<R, S, K, C, P, I, T> {
    pub const fn new(
        relationships: R,
        link: SharedPeerLink<S, K>,
        cipher: SharedPeerCipher<C, P>,
        inbound: I,
        receipts: SharedControlQueue<T>,
        engine: EngineHandle,
        local_identity_id: OpaqueId,
    ) -> Self {
        Self {
            relationships,
            link,
            cipher,
            inbound,
            receipts,
            engine,
            local_identity_id,
        }
    }
}
impl<R, S, K, C, P, I, T> InboundPeerProcessor<R, S, K, C, P, I, T>
where
    R: ContactRepository + ConversationRepository + PeerCredentialRepository,
    S: ContactRepository + PeerCredentialRepository,
    K: HandshakeSigner,
    C: CryptoProvider,
    P: ProtectedSecretStore,
    I: InboundMessageStore,
    T: ControlTransport,
{
    pub fn process_available(
        &mut self,
        limit: usize,
    ) -> Result<InboundProcessReport, PeerMessagingError> {
        let mut report = InboundProcessReport::default();
        while report.processed < limit {
            let Some(envelope) = self.link.take_inbound().map_err(map_peer)? else {
                break;
            };
            self.process_one(envelope, &mut report)?;
            report.processed += 1;
        }
        Ok(report)
    }

    fn process_one(
        &mut self,
        envelope: torca_peer_link::InboundPeerEnvelope,
        report: &mut InboundProcessReport,
    ) -> Result<(), PeerMessagingError> {
        let contact = load_contact(&self.relationships, envelope.contact_id)?;
        let credential = load_credential(&self.relationships, contact.id())?;
        let plaintext = open_payload(
            &self.cipher,
            credential.secret_handle(),
            envelope.envelope_id,
            envelope.message_kind,
            self.local_identity_id,
            contact.remote_identity().identity_id().to_opaque(),
            &envelope.ciphertext,
        )?;
        let payload = ApplicationPayloadCodec::decode(&plaintext)
            .map_err(|_| PeerMessagingError::Protocol)?;
        match (envelope.message_kind, payload) {
            (TEXT_MESSAGE_KIND, ApplicationPayload::Text(text)) => {
                let conversation = self
                    .relationships
                    .for_contact(contact.id())
                    .map_err(|_| PeerMessagingError::Relationship)?
                    .ok_or(PeerMessagingError::ConversationMissing)?;
                let message = inbound_message(&conversation, text)?;
                let inserted = self
                    .inbound
                    .persist_inbound(envelope.envelope_id, message)
                    .map_err(map_store)?;
                let receipt = ReceiptPayload {
                    receipt_id: derived_receipt_id(envelope.envelope_id, 0xD1),
                    message_id: envelope.envelope_id,
                    contact_id: contact.id().to_opaque(),
                    kind: DeliveryReceiptKind::Delivered,
                    at: system_timestamp()?,
                };
                // The durable Delivered receipt exists before protocol ACK. If the ACK is lost or
                // the process dies, redelivery is idempotent and ensure_receipt accepts Duplicate.
                self.receipts.ensure_receipt(contact.id(), receipt, receipt.at)?;
                self.link
                    .send_ack(
                        contact.id(),
                        envelope.envelope_id,
                        if inserted { AckStatus::Accepted } else { AckStatus::Duplicate },
                    )
                    .map_err(map_peer)?;
                if inserted {
                    report.texts_inserted += 1;
                } else {
                    report.duplicates += 1;
                }
            }
            (RECEIPT_MESSAGE_KIND, ApplicationPayload::Receipt(receipt)) => {
                self.engine
                    .dispatch(EngineCommand::ApplyReceipt(Receipt {
                        id: ReceiptId::from_opaque(receipt.receipt_id),
                        message_id: MessageId::from_opaque(receipt.message_id),
                        kind: match receipt.kind {
                            DeliveryReceiptKind::Delivered => ReceiptKind::Delivered,
                            DeliveryReceiptKind::Read => ReceiptKind::Read,
                        },
                        at: receipt.at,
                    }))
                    .map_err(|_| PeerMessagingError::Engine)?;
                self.link
                    .send_ack(contact.id(), envelope.envelope_id, AckStatus::Accepted)
                    .map_err(map_peer)?;
                report.receipts_applied += 1;
            }
            _ => {
                let _ = self
                    .link
                    .send_ack(contact.id(), envelope.envelope_id, AckStatus::Rejected);
                return Err(PeerMessagingError::Protocol);
            }
        }
        Ok(())
    }
}

fn inbound_message(
    conversation: &DirectConversation,
    payload: TextPayload,
) -> Result<Message, PeerMessagingError> {
    let body = MessageBody::new(payload.body).map_err(|_| PeerMessagingError::Protocol)?;
    let reply_to = payload.reply_to.map(|id| ReplyReference {
        message_id: MessageId::from_opaque(id),
    });
    Ok(Message::inbound(
        MessageId::from_opaque(payload.message_id),
        conversation.id(),
        body,
        reply_to,
        payload.sent_at,
    ))
}

fn load_contact<R: ContactRepository>(
    repository: &R,
    contact_id: ContactId,
) -> Result<Contact, PeerMessagingError> {
    repository
        .get(contact_id)
        .map_err(|_| PeerMessagingError::Relationship)?
        .ok_or(PeerMessagingError::Relationship)
}

fn load_credential<R: PeerCredentialRepository>(
    repository: &R,
    contact_id: ContactId,
) -> Result<PeerCredential, PeerMessagingError> {
    repository
        .credential_for_contact(contact_id)
        .map_err(|_| PeerMessagingError::Relationship)?
        .ok_or(PeerMessagingError::Relationship)
}

fn seal_payload<C, P>(
    cipher: &SharedPeerCipher<C, P>,
    handle: OpaqueId,
    envelope_id: OpaqueId,
    message_kind: u16,
    local_identity: OpaqueId,
    remote_identity: OpaqueId,
    plaintext: &[u8],
) -> Result<Vec<u8>, PeerMessagingError>
where
    C: CryptoProvider,
    P: ProtectedSecretStore,
{
    let mut secrets = cipher.inner.lock().map_err(|_| PeerMessagingError::Crypto)?;
    let nonce = secrets.peer_nonce().map_err(|_| PeerMessagingError::Crypto)?;
    let aad = peer_aad(envelope_id, message_kind, local_identity, remote_identity);
    let encrypted = secrets
        .seal_peer_payload(handle, nonce, &aad, plaintext)
        .map_err(|_| PeerMessagingError::Crypto)?;
    let mut output = Vec::with_capacity(NONCE_BYTES + encrypted.0.len());
    output.extend_from_slice(&nonce.0);
    output.extend_from_slice(&encrypted.0);
    Ok(output)
}

fn open_payload<C, P>(
    cipher: &SharedPeerCipher<C, P>,
    handle: OpaqueId,
    envelope_id: OpaqueId,
    message_kind: u16,
    local_identity: OpaqueId,
    remote_identity: OpaqueId,
    stored: &[u8],
) -> Result<Vec<u8>, PeerMessagingError>
where
    C: CryptoProvider,
    P: ProtectedSecretStore,
{
    if stored.len() <= NONCE_BYTES {
        return Err(PeerMessagingError::InvalidCiphertext);
    }
    let nonce = Nonce(
        stored[..NONCE_BYTES]
            .try_into()
            .map_err(|_| PeerMessagingError::InvalidCiphertext)?,
    );
    let ciphertext = Ciphertext(stored[NONCE_BYTES..].to_vec());
    let aad = peer_aad(envelope_id, message_kind, local_identity, remote_identity);
    cipher
        .inner
        .lock()
        .map_err(|_| PeerMessagingError::Crypto)?
        .open_peer_payload(handle, nonce, &aad, &ciphertext)
        .map_err(|_| PeerMessagingError::Crypto)
}

fn peer_aad(
    envelope_id: OpaqueId,
    message_kind: u16,
    local_identity: OpaqueId,
    remote_identity: OpaqueId,
) -> Vec<u8> {
    let (first, second) = if local_identity <= remote_identity {
        (local_identity, remote_identity)
    } else {
        (remote_identity, local_identity)
    };
    let mut aad = Vec::with_capacity(AAD_LABEL.len() + 50);
    aad.extend_from_slice(AAD_LABEL);
    aad.extend_from_slice(envelope_id.as_bytes());
    aad.extend_from_slice(&message_kind.to_be_bytes());
    aad.extend_from_slice(first.as_bytes());
    aad.extend_from_slice(second.as_bytes());
    aad
}

fn map_link_ack(ack: LinkAck) -> DeliveryAck {
    match ack {
        LinkAck::Accepted => DeliveryAck::Accepted,
        LinkAck::Duplicate => DeliveryAck::Duplicate,
    }
}

fn derived_receipt_id(message_id: OpaqueId, tag: u8) -> OpaqueId {
    let mut bytes = message_id.into_bytes();
    bytes[15] ^= tag;
    let value = OpaqueId::from_bytes(bytes);
    if value.is_nil() { OpaqueId::from_u128(u128::from(tag) + 1) } else { value }
}

fn system_timestamp() -> Result<Timestamp, PeerMessagingError> {
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| PeerMessagingError::Protocol)?;
    let millis = i64::try_from(duration.as_millis()).map_err(|_| PeerMessagingError::Protocol)?;
    Timestamp::from_unix_millis(millis).map_err(|_| PeerMessagingError::Protocol)
}

fn map_peer(_: PeerLinkError) -> PeerMessagingError {
    PeerMessagingError::Peer
}
fn map_control(_: ControlDeliveryError) -> PeerMessagingError {
    PeerMessagingError::Control
}
fn map_store(_: DurableDeliveryError) -> PeerMessagingError {
    PeerMessagingError::Store
}
