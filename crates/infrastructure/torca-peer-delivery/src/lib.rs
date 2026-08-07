//! Durable text/receipt delivery adapters over one shared authenticated PeerLink.

use core::fmt;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use torca_client_engine::{EngineCommand, EngineHandle};
use torca_contacts::{
    Contact, ContactError, ContactId, ContactRepository, PeerCredentialRepository,
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
use torca_peer_link::{InboundPeerEnvelope, LinkAck, PeerLink, PeerLinkError, PeerLinkReport};
use torca_peer_protocol::{AckStatus, HandshakeSigner};
use torca_receipts::{Receipt, ReceiptId, ReceiptKind};

pub const TEXT_MESSAGE_KIND: u16 = 1;
pub const RECEIPT_MESSAGE_KIND: u16 = 2;
const NONCE_BYTES: usize = 24;
const AAD_LABEL: &[u8] = b"TORCA-PEER-DATA-V1";

/// Cloneable handle to the one process-owned peer link registry.
pub struct SharedPeerLink<S, K> {
    inner: Arc<Mutex<PeerLink<S, K>>>,
}
impl<S, K> Clone for SharedPeerLink<S, K> {
    fn clone(&self) -> Self {
        Self { inner: Arc::clone(&self.inner) }
    }
}
impl<S, K> SharedPeerLink<S, K> {
    pub fn new(link: PeerLink<S, K>) -> Self {
        Self { inner: Arc::new(Mutex::new(link)) }
    }
}
impl<S, K> SharedPeerLink<S, K>
where
    S: ContactRepository + PeerCredentialRepository,
    K: HandshakeSigner,
{
    pub fn maintenance(
        &self,
        contacts: &[ContactId],
        now: Timestamp,
    ) -> Result<PeerLinkReport, PeerDeliveryError> {
        self.inner
            .lock()
            .map_err(|_| PeerDeliveryError::Peer)?
            .maintenance(contacts, now)
            .map_err(map_peer)
    }

    pub fn take_inbound(&self) -> Result<Option<InboundPeerEnvelope>, PeerDeliveryError> {
        Ok(self
            .inner
            .lock()
            .map_err(|_| PeerDeliveryError::Peer)?
            .take_inbound())
    }

    pub fn shutdown(&self) {
        if let Ok(mut link) = self.inner.lock() {
            link.shutdown();
        }
    }
}

/// Shared protected peer-secret crypto owner. Multiple delivery adapters never duplicate the
/// underlying key material; all access is serialized through the protected manager.
pub struct SharedPeerCipher<C, P> {
    inner: Arc<Mutex<ManagedPeerSecrets<C, P>>>,
}
impl<C, P> Clone for SharedPeerCipher<C, P> {
    fn clone(&self) -> Self {
        Self { inner: Arc::clone(&self.inner) }
    }
}
impl<C, P> SharedPeerCipher<C, P> {
    pub fn new(peer_secrets: ManagedPeerSecrets<C, P>) -> Self {
        Self { inner: Arc::new(Mutex::new(peer_secrets)) }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PeerDeliveryError {
    Relationship,
    Peer,
    Crypto,
    Protocol,
    InvalidCiphertext,
    ContactMismatch,
    ConversationMissing,
    Engine,
    Store,
}
impl fmt::Display for PeerDeliveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for PeerDeliveryError {}

/// Concrete DeliveryWorker transport for text messages.
pub struct PeerDeliveryTransport<R, S, K, C, P> {
    relationships: R,
    link: SharedPeerLink<S, K>,
    cipher: SharedPeerCipher<C, P>,
    local_identity_id: OpaqueId,
    ack_timeout: Duration,
}
impl<R, S, K, C, P> PeerDeliveryTransport<R, S, K, C, P> {
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

impl<R, S, K, C, P> DeliveryTransport for PeerDeliveryTransport<R, S, K, C, P>
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

impl<R, S, K, C, P> PeerDeliveryTransport<R, S, K, C, P>
where
    R: ContactRepository + ConversationRepository + PeerCredentialRepository,
    S: ContactRepository + PeerCredentialRepository,
    K: HandshakeSigner,
    C: CryptoProvider,
    P: ProtectedSecretStore,
{
    fn send_text(&mut self, message: &Message) -> Result<DeliveryAck, PeerDeliveryError> {
        let conversation = self
            .relationships
            .get(message.conversation_id())
            .map_err(|_| PeerDeliveryError::Relationship)?
            .ok_or(PeerDeliveryError::ConversationMissing)?;
        let contact = self.contact(conversation.contact_id())?;
        let credential = self.credential(contact.id())?;
        let payload = ApplicationPayload::Text(TextPayload {
            message_id: message.id().to_opaque(),
            conversation_id: message.conversation_id().to_opaque(),
            contact_id: contact.id().to_opaque(),
            body: message.body().as_str().to_owned(),
            reply_to: message.reply_to().map(|reply| reply.message_id.to_opaque()),
            sent_at: message.created_at(),
        });
        let plaintext = ApplicationPayloadCodec::encode(&payload)
            .map_err(|_| PeerDeliveryError::Protocol)?;
        let ciphertext = self.seal(
            credential.secret_handle(),
            message.id().to_opaque(),
            TEXT_MESSAGE_KIND,
            contact.remote_identity().identity_id().to_opaque(),
            &plaintext,
        )?;
        let ack = self
            .link
            .inner
            .lock()
            .map_err(|_| PeerDeliveryError::Peer)?
            .send_and_wait_ack(
                contact.id(),
                message.id().to_opaque(),
                TEXT_MESSAGE_KIND,
                ciphertext,
                self.ack_timeout,
            )
            .map_err(map_peer)?;
        Ok(match ack {
            LinkAck::Accepted => DeliveryAck::Accepted,
            LinkAck::Duplicate => DeliveryAck::Duplicate,
        })
    }

    fn contact(&self, contact_id: ContactId) -> Result<Contact, PeerDeliveryError> {
        self.relationships
            .get(contact_id)
            .map_err(|_| PeerDeliveryError::Relationship)?
            .ok_or(PeerDeliveryError::Relationship)
    }

    fn credential(
        &self,
        contact_id: ContactId,
    ) -> Result<torca_contacts::PeerCredential, PeerDeliveryError> {
        self.relationships
            .credential_for_contact(contact_id)
            .map_err(|_| PeerDeliveryError::Relationship)?
            .ok_or(PeerDeliveryError::Relationship)
    }

    fn seal(
        &self,
        handle: OpaqueId,
        envelope_id: OpaqueId,
        message_kind: u16,
        remote_identity_id: OpaqueId,
        plaintext: &[u8],
    ) -> Result<Vec<u8>, PeerDeliveryError> {
        let mut cipher = self.cipher.inner.lock().map_err(|_| PeerDeliveryError::Crypto)?;
        let nonce = cipher.peer_nonce().map_err(|_| PeerDeliveryError::Crypto)?;
        let aad = peer_aad(
            envelope_id,
            message_kind,
            self.local_identity_id,
            remote_identity_id,
        );
        let encrypted = cipher
            .seal_peer_payload(handle, nonce, &aad, plaintext)
            .map_err(|_| PeerDeliveryError::Crypto)?;
        let mut output = Vec::with_capacity(NONCE_BYTES + encrypted.0.len());
        output.extend_from_slice(&nonce.0);
        output.extend_from_slice(&encrypted.0);
        Ok(output)
    }
}

#[must_use]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct InboundProcessReport {
    pub processed: usize,
    pub texts_inserted: usize,
    pub duplicates: usize,
    pub receipts_applied: usize,
    /// Delivered receipts that must be durably enqueued by the runtime before considering the
    /// inbound text workflow fully settled.
    pub delivered_receipts: Vec<ReceiptPayload>,
}

pub struct PeerInboundProcessor<R, S, K, C, P, I> {
    relationships: R,
    link: SharedPeerLink<S, K>,
    cipher: SharedPeerCipher<C, P>,
    inbound: I,
    engine: EngineHandle,
    local_identity_id: OpaqueId,
}
impl<R, S, K, C, P, I> PeerInboundProcessor<R, S, K, C, P, I> {
    pub const fn new(
        relationships: R,
        link: SharedPeerLink<S, K>,
        cipher: SharedPeerCipher<C, P>,
        inbound: I,
        engine: EngineHandle,
        local_identity_id: OpaqueId,
    ) -> Self {
        Self { relationships, link, cipher, inbound, engine, local_identity_id }
    }
}

impl<R, S, K, C, P, I> PeerInboundProcessor<R, S, K, C, P, I>
where
    R: ContactRepository + ConversationRepository + PeerCredentialRepository,
    S: ContactRepository + PeerCredentialRepository,
    K: HandshakeSigner,
    C: CryptoProvider,
    P: ProtectedSecretStore,
    I: InboundMessageStore,
{
    pub fn process_available(
        &mut self,
        limit: usize,
    ) -> Result<InboundProcessReport, PeerDeliveryError> {
        let mut report = InboundProcessReport::default();
        while report.processed < limit {
            let Some(envelope) = self.link.take_inbound()? else {
                break;
            };
            self.process_one(envelope, &mut report)?;
            report.processed += 1;
        }
        Ok(report)
    }

    pub fn into_parts(self) -> (R, SharedPeerLink<S, K>, SharedPeerCipher<C, P>, I, EngineHandle) {
        (self.relationships, self.link, self.cipher, self.inbound, self.engine)
    }

    fn process_one(
        &mut self,
        envelope: InboundPeerEnvelope,
        report: &mut InboundProcessReport,
    ) -> Result<(), PeerDeliveryError> {
        let contact = self
            .relationships
            .get(envelope.contact_id)
            .map_err(|_| PeerDeliveryError::Relationship)?
            .ok_or(PeerDeliveryError::Relationship)?;
        let credential = self
            .relationships
            .credential_for_contact(contact.id())
            .map_err(|_| PeerDeliveryError::Relationship)?
            .ok_or(PeerDeliveryError::Relationship)?;
        let plaintext = self.open(
            credential.secret_handle(),
            envelope.envelope_id,
            envelope.message_kind,
            contact.remote_identity().identity_id().to_opaque(),
            &envelope.ciphertext,
        )?;
        let payload = ApplicationPayloadCodec::decode(&plaintext)
            .map_err(|_| PeerDeliveryError::Protocol)?;
        match (envelope.message_kind, payload) {
            (TEXT_MESSAGE_KIND, ApplicationPayload::Text(text)) => {
                let conversation = self
                    .relationships
                    .for_contact(contact.id())
                    .map_err(|_| PeerDeliveryError::Relationship)?
                    .ok_or(PeerDeliveryError::ConversationMissing)?;
                let message = inbound_message(&conversation, text)?;
                let inserted = self
                    .inbound
                    .persist_inbound(envelope.envelope_id, message)
                    .map_err(map_store)?;
                let ack = if inserted { AckStatus::Accepted } else { AckStatus::Duplicate };
                self.send_ack(contact.id(), envelope.envelope_id, ack)?;
                if inserted {
                    report.texts_inserted += 1;
                    report.delivered_receipts.push(ReceiptPayload {
                        receipt_id: derived_receipt_id(envelope.envelope_id, 0xD1),
                        message_id: envelope.envelope_id,
                        contact_id: contact.id().to_opaque(),
                        kind: DeliveryReceiptKind::Delivered,
                        at: system_timestamp()?,
                    });
                } else {
                    report.duplicates += 1;
                }
            }
            (RECEIPT_MESSAGE_KIND, ApplicationPayload::Receipt(receipt)) => {
                let domain = Receipt {
                    id: ReceiptId::from_opaque(receipt.receipt_id),
                    message_id: MessageId::from_opaque(receipt.message_id),
                    kind: match receipt.kind {
                        DeliveryReceiptKind::Delivered => ReceiptKind::Delivered,
                        DeliveryReceiptKind::Read => ReceiptKind::Read,
                    },
                    at: receipt.at,
                };
                self.engine
                    .dispatch(EngineCommand::ApplyReceipt(domain))
                    .map_err(|_| PeerDeliveryError::Engine)?;
                self.send_ack(contact.id(), envelope.envelope_id, AckStatus::Accepted)?;
                report.receipts_applied += 1;
            }
            _ => {
                let _ = self.send_ack(contact.id(), envelope.envelope_id, AckStatus::Rejected);
                return Err(PeerDeliveryError::Protocol);
            }
        }
        Ok(())
    }

    fn open(
        &self,
        handle: OpaqueId,
        envelope_id: OpaqueId,
        message_kind: u16,
        remote_identity_id: OpaqueId,
        stored: &[u8],
    ) -> Result<Vec<u8>, PeerDeliveryError> {
        if stored.len() <= NONCE_BYTES {
            return Err(PeerDeliveryError::InvalidCiphertext);
        }
        let nonce = Nonce(
            stored[..NONCE_BYTES]
                .try_into()
                .map_err(|_| PeerDeliveryError::InvalidCiphertext)?,
        );
        let ciphertext = Ciphertext(stored[NONCE_BYTES..].to_vec());
        let aad = peer_aad(
            envelope_id,
            message_kind,
            self.local_identity_id,
            remote_identity_id,
        );
        self.cipher
            .inner
            .lock()
            .map_err(|_| PeerDeliveryError::Crypto)?
            .open_peer_payload(handle, nonce, &aad, &ciphertext)
            .map_err(|_| PeerDeliveryError::Crypto)
    }

    fn send_ack(
        &self,
        contact_id: ContactId,
        envelope_id: OpaqueId,
        status: AckStatus,
    ) -> Result<(), PeerDeliveryError> {
        self.link
            .inner
            .lock()
            .map_err(|_| PeerDeliveryError::Peer)?
            .send_ack(contact_id, envelope_id, status)
            .map_err(map_peer)
    }
}

fn inbound_message(
    conversation: &DirectConversation,
    payload: TextPayload,
) -> Result<Message, PeerDeliveryError> {
    let body = MessageBody::new(payload.body).map_err(|_| PeerDeliveryError::Protocol)?;
    let reply_to = payload.reply_to.map(|message_id| ReplyReference {
        message_id: MessageId::from_opaque(message_id),
    });
    Ok(Message::inbound(
        MessageId::from_opaque(payload.message_id),
        conversation.id(),
        body,
        reply_to,
        payload.sent_at,
    ))
}

fn peer_aad(
    envelope_id: OpaqueId,
    message_kind: u16,
    local_identity_id: OpaqueId,
    remote_identity_id: OpaqueId,
) -> Vec<u8> {
    let (first, second) = if local_identity_id <= remote_identity_id {
        (local_identity_id, remote_identity_id)
    } else {
        (remote_identity_id, local_identity_id)
    };
    let mut aad = Vec::with_capacity(AAD_LABEL.len() + 16 + 2 + 32);
    aad.extend_from_slice(AAD_LABEL);
    aad.extend_from_slice(envelope_id.as_bytes());
    aad.extend_from_slice(&message_kind.to_be_bytes());
    aad.extend_from_slice(first.as_bytes());
    aad.extend_from_slice(second.as_bytes());
    aad
}

fn derived_receipt_id(message_id: OpaqueId, tag: u8) -> OpaqueId {
    let mut bytes = message_id.into_bytes();
    bytes[15] ^= tag;
    let result = OpaqueId::from_bytes(bytes);
    if result.is_nil() { OpaqueId::from_u128(u128::from(tag) + 1) } else { result }
}

fn system_timestamp() -> Result<Timestamp, PeerDeliveryError> {
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| PeerDeliveryError::Protocol)?;
    let millis = i64::try_from(duration.as_millis()).map_err(|_| PeerDeliveryError::Protocol)?;
    Timestamp::from_unix_millis(millis).map_err(|_| PeerDeliveryError::Protocol)
}

fn map_peer(_: PeerLinkError) -> PeerDeliveryError {
    PeerDeliveryError::Peer
}
fn map_store(_: DurableDeliveryError) -> PeerDeliveryError {
    PeerDeliveryError::Store
}
#[allow(dead_code)]
fn map_contact(_: ContactError) -> PeerDeliveryError {
    PeerDeliveryError::Relationship
}
