//! Concrete adapters that compose SQLCipher durable work with one authenticated shared PeerLink.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};
use std::time::Duration;

use crate::{application_envelope, application_peer_state, peer_envelope};
use torca_client_engine::{EngineCommand, EngineHandle};
use torca_communication_driver::{
    CommunicationError, ControlDeliveryRuntime, InboundEnvelope, InboundMessagingRuntime,
    PeerActivityEvidence, PeerConnectionStatus, PeerLinkRuntime, REACTION_MESSAGE_KIND,
    RECEIPT_MESSAGE_KIND, ReadStateRuntime, TEXT_MESSAGE_KIND, TextDeliveryRuntime,
    plan_read_receipts,
};
use torca_contacts::{
    Contact, ContactId, ContactRepository, PeerCredential, PeerCredentialRepository,
};
use torca_control_delivery::{
    ControlAck, ControlDeliveryError, ControlDeliveryWorker, ControlJob, ControlKind,
    ControlTransport, ControlTransportError,
};
use torca_conversations::{ConversationRepository, DirectConversation};
use torca_crypto::{Ciphertext, CryptoProvider, ManagedPeerSecrets, Nonce, ProtectedSecretStore};
use torca_delivery::{
    ApplicationPayload, ApplicationPayloadCodec, DeliveryAck, DeliveryReceiptKind,
    DeliveryTransport, DeliveryTransportError, DeliveryWorker, DurableDeliveryStore,
    InboundMessageStore, ReactionPayload, ReceiptPayload, TextPayload,
};
use torca_foundation::{OpaqueId, Timestamp};
use torca_messaging::{Message, MessageBody, MessageId, ReplyReference};
use torca_peer_link::{LinkAck, PeerLinkError};
use torca_peer_protocol::{AckStatus, HandshakeSigner};
use torca_peer_shared::SharedPeerLink;
use torca_receipts::{Receipt, ReceiptId, ReceiptKind};
use torca_storage_sqlite::SqlCipherReadState;

const NONCE_BYTES: usize = 24;
const PEER_AAD_LABEL: &[u8] = b"TORCA-PEER-DATA-V1";
const STALE_CLAIM_AGE: Duration = Duration::from_secs(120);

/// Process-shared protected peer-secret manager. It serializes cryptographic operations without
/// exposing pairwise key bytes to delivery or UI layers.
pub struct SharedPeerCrypto<C, P> {
    pub(crate) inner: Arc<Mutex<ManagedPeerSecrets<C, P>>>,
}
impl<C, P> Clone for SharedPeerCrypto<C, P> {
    fn clone(&self) -> Self {
        Self { inner: Arc::clone(&self.inner) }
    }
}
impl<C, P> SharedPeerCrypto<C, P> {
    pub fn new(secrets: ManagedPeerSecrets<C, P>) -> Self {
        Self { inner: Arc::new(Mutex::new(secrets)) }
    }
}

pub struct PeerLinkAdapter<S, K> {
    link: SharedPeerLink<S, K>,
}
impl<S, K> PeerLinkAdapter<S, K> {
    pub const fn new(link: SharedPeerLink<S, K>) -> Self {
        Self { link }
    }
}
impl<S, K> PeerLinkRuntime for PeerLinkAdapter<S, K>
where
    S: ContactRepository + PeerCredentialRepository + Send + 'static,
    K: HandshakeSigner + Send + 'static,
{
    fn maintenance(
        &mut self,
        contacts: &[ContactId],
        now: Timestamp,
    ) -> Result<(), CommunicationError> {
        self.link.maintenance(contacts, now).map(|_| ()).map_err(|_| CommunicationError::Peer)
    }

    fn next_maintenance_delay(&self, now: Timestamp) -> Option<Duration> {
        self.link.next_maintenance_delay(now)
    }

    fn network_changed(&mut self, now: Timestamp) {
        let _ = self.link.network_changed(now);
    }

    fn disconnect_contact(&mut self, contact_id: ContactId) {
        let _ = self.link.disconnect_contact(contact_id);
    }

    fn set_waker(&mut self, waker: Arc<dyn Fn() + Send + Sync>) {
        let _ = self.link.set_waker(waker);
    }

    fn connection_state(&self, contact_id: ContactId) -> PeerConnectionStatus {
        application_peer_state(self.link.connection_state(contact_id))
    }

    fn peer_activity(&self) -> Vec<PeerActivityEvidence> {
        self.link
            .activity()
            .into_iter()
            .map(|(contact_id, activity)| PeerActivityEvidence {
                contact_id,
                sequence: activity.sequence,
                tx_frames: activity.tx_frames,
                rx_frames: activity.rx_frames,
                tx_acks: activity.tx_acks,
                rx_acks: activity.rx_acks,
                handshakes: activity.handshakes,
                failures: activity.failures,
                last_activity_at: activity.last_activity_at,
            })
            .collect()
    }

    fn take_inbound(&mut self) -> Result<Option<InboundEnvelope>, CommunicationError> {
        self.link
            .take_inbound()
            .map_err(|_| CommunicationError::Peer)
            .map(|envelope| envelope.map(application_envelope))
    }

    fn reject(&mut self, envelope: &InboundEnvelope) -> Result<(), CommunicationError> {
        self.link
            .send_ack(envelope.contact_id, envelope.envelope_id, AckStatus::Rejected)
            .map_err(|_| CommunicationError::Peer)
    }

    fn shutdown(&mut self) {
        self.link.shutdown();
    }
}

pub struct TextWorkerAdapter<S, T> {
    worker: DeliveryWorker<S, T>,
    database_writes: u64,
}
impl<S, T> TextWorkerAdapter<S, T> {
    pub const fn new(worker: DeliveryWorker<S, T>) -> Self {
        Self { worker, database_writes: 0 }
    }
}
impl<S, T> TextDeliveryRuntime for TextWorkerAdapter<S, T>
where
    S: DurableDeliveryStore + Send + 'static,
    T: DeliveryTransport + Send + 'static,
{
    fn recover(&mut self, now: Timestamp) -> Result<(), CommunicationError> {
        let before = now.checked_sub(STALE_CLAIM_AGE).unwrap_or(Timestamp::UNIX_EPOCH);
        let recovered =
            self.worker.recover_stale_claims(before).map_err(|_| CommunicationError::Text)?;
        self.database_writes = self.database_writes.saturating_add(recovered as u64);
        Ok(())
    }

    fn maintenance(&mut self, now: Timestamp, limit: usize) -> Result<(), CommunicationError> {
        let report = self.worker.run_once(now, limit).map_err(|_| CommunicationError::Text)?;
        // Claiming a job and recording its outcome are separate durable
        // writes. The worker report exposes both sides without estimating
        // SQL statements from a maintenance tick.
        let writes = report.claimed.saturating_add(
            report
                .completed
                .saturating_add(report.rescheduled)
                .saturating_add(report.dead_lettered),
        );
        self.database_writes = self.database_writes.saturating_add(writes as u64);
        Ok(())
    }

    fn next_maintenance_delay(&self, now: Timestamp) -> Option<Duration> {
        self.worker.next_due().ok().flatten().map(|due| due.duration_since(now).unwrap_or_default())
    }

    fn database_write_count(&self) -> u64 {
        self.database_writes
    }
}

/// Shared durable control worker so inbound Delivered receipt creation and periodic sender
/// maintenance operate on the same outbox owner.
pub struct SharedControlWorker<T> {
    inner: Arc<Mutex<ControlDeliveryWorker<T>>>,
    database_writes: Arc<AtomicU64>,
}
impl<T> Clone for SharedControlWorker<T> {
    fn clone(&self) -> Self {
        Self { inner: Arc::clone(&self.inner), database_writes: Arc::clone(&self.database_writes) }
    }
}
impl<T> SharedControlWorker<T> {
    pub fn new(worker: ControlDeliveryWorker<T>) -> Self {
        Self { inner: Arc::new(Mutex::new(worker)), database_writes: Arc::new(AtomicU64::new(0)) }
    }
}
impl<T: ControlTransport + Send + 'static> SharedControlWorker<T> {
    pub fn ensure_receipt(
        &self,
        contact_id: ContactId,
        receipt: ReceiptPayload,
        at: Timestamp,
    ) -> Result<(), CommunicationError> {
        let payload = ApplicationPayloadCodec::encode(&ApplicationPayload::Receipt(receipt))
            .map_err(|_| CommunicationError::Control)?;
        let result = self.inner.lock().map_err(|_| CommunicationError::Control)?.queue(
            receipt.receipt_id,
            contact_id.to_opaque(),
            ControlKind::Receipt,
            &payload,
            at,
        );
        match result {
            Ok(()) => {
                self.database_writes.fetch_add(1, Ordering::Relaxed);
                Ok(())
            }
            Err(ControlDeliveryError::Duplicate) => Ok(()),
            Err(_) => Err(CommunicationError::Control),
        }
    }
    pub fn ensure_reaction(
        &self,
        contact_id: ContactId,
        reaction: ReactionPayload,
        at: Timestamp,
    ) -> Result<(), CommunicationError> {
        let job_id = reaction.reaction_id;
        let payload = ApplicationPayloadCodec::encode(&ApplicationPayload::Reaction(reaction))
            .map_err(|_| CommunicationError::Control)?;
        let result = self.inner.lock().map_err(|_| CommunicationError::Control)?.queue(
            job_id,
            contact_id.to_opaque(),
            ControlKind::Reaction,
            &payload,
            at,
        );
        match result {
            Ok(()) => {
                self.database_writes.fetch_add(1, Ordering::Relaxed);
                Ok(())
            }
            Err(ControlDeliveryError::Duplicate) => Ok(()),
            Err(_) => Err(CommunicationError::Control),
        }
    }
}
impl<T: ControlTransport + Send + 'static> ControlDeliveryRuntime for SharedControlWorker<T> {
    fn recover(&mut self, now: Timestamp) -> Result<(), CommunicationError> {
        let before = now.checked_sub(STALE_CLAIM_AGE).unwrap_or(Timestamp::UNIX_EPOCH);
        let recovered = self
            .inner
            .lock()
            .map_err(|_| CommunicationError::Control)?
            .recover_stale(before)
            .map_err(|_| CommunicationError::Control)?;
        self.database_writes.fetch_add(recovered as u64, Ordering::Relaxed);
        Ok(())
    }

    fn maintenance(&mut self, now: Timestamp, limit: usize) -> Result<(), CommunicationError> {
        let report = self
            .inner
            .lock()
            .map_err(|_| CommunicationError::Control)?
            .run_once(now, limit)
            .map_err(|_| CommunicationError::Control)?;
        let writes = report.claimed.saturating_add(
            report
                .completed
                .saturating_add(report.rescheduled)
                .saturating_add(report.dead_lettered),
        );
        self.database_writes.fetch_add(writes as u64, Ordering::Relaxed);
        Ok(())
    }
    fn next_maintenance_delay(&self, now: Timestamp) -> Option<Duration> {
        self.inner
            .lock()
            .ok()
            .and_then(|worker| worker.next_due().ok().flatten())
            .map(|due| due.duration_since(now).unwrap_or_default())
    }

    fn database_write_count(&self) -> u64 {
        self.database_writes.load(Ordering::Relaxed)
    }
    fn queue_reaction(
        &mut self,
        contact_id: ContactId,
        reaction: ReactionPayload,
        at: Timestamp,
    ) -> Result<(), CommunicationError> {
        self.ensure_reaction(contact_id, reaction, at)
    }
}

pub struct ReadStateAdapter {
    read_state: SqlCipherReadState,
}
impl ReadStateAdapter {
    pub const fn new(read_state: SqlCipherReadState) -> Self {
        Self { read_state }
    }
}
impl ReadStateRuntime for ReadStateAdapter {
    fn mark_conversation_read(
        &mut self,
        conversation_id: OpaqueId,
        now: Timestamp,
    ) -> Result<(), CommunicationError> {
        let candidates = self
            .read_state
            .read_candidates(conversation_id)
            .map_err(|_| CommunicationError::ReadState)?;
        let jobs = plan_read_receipts(&candidates, now)?;
        self.read_state
            .commit_mark_read(conversation_id, now, &jobs)
            .map(|_| ())
            .map_err(|_| CommunicationError::ReadState)
    }
}

pub struct TextPeerTransport<R, S, K, C, P> {
    relationships: R,
    link: SharedPeerLink<S, K>,
    crypto: SharedPeerCrypto<C, P>,
    local_identity_id: OpaqueId,
    ack_timeout: Duration,
}
impl<R, S, K, C, P> TextPeerTransport<R, S, K, C, P> {
    pub const fn new(
        relationships: R,
        link: SharedPeerLink<S, K>,
        crypto: SharedPeerCrypto<C, P>,
        local_identity_id: OpaqueId,
        ack_timeout: Duration,
    ) -> Self {
        Self { relationships, link, crypto, local_identity_id, ack_timeout }
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
        self.send_message(message).map_err(|error| DeliveryTransportError(format!("{error}")))
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
    fn send_message(&mut self, message: &Message) -> Result<DeliveryAck, CommunicationError> {
        let conversation =
            ConversationRepository::get(&self.relationships, message.conversation_id())
                .map_err(|_| CommunicationError::Text)?
                .ok_or(CommunicationError::Text)?;
        let contact = load_contact(&self.relationships, conversation.contact_id())?;
        let credential = load_credential(&self.relationships, contact.id())?;
        let plaintext = ApplicationPayloadCodec::encode(&ApplicationPayload::Text(TextPayload {
            message_id: message.id().to_opaque(),
            conversation_id: message.conversation_id().to_opaque(),
            contact_id: contact.id().to_opaque(),
            body: message.body().as_str().to_owned(),
            reply_to: message.reply_to().map(|reply| reply.message_id.to_opaque()),
            sent_at: message.created_at(),
        }))
        .map_err(|_| CommunicationError::Text)?;
        let encrypted = seal(
            &self.crypto,
            credential.secret_handle(),
            message.id().to_opaque(),
            TEXT_MESSAGE_KIND,
            self.local_identity_id,
            contact.remote_identity().identity_id().to_opaque(),
            &plaintext,
        )?;
        self.link
            .send_and_wait_ack(
                contact.id(),
                message.id().to_opaque(),
                TEXT_MESSAGE_KIND,
                encrypted,
                self.ack_timeout,
            )
            .map(map_ack)
            .map_err(|_| CommunicationError::Peer)
    }
}

pub struct ReceiptPeerTransport<R, S, K, C, P> {
    relationships: R,
    link: SharedPeerLink<S, K>,
    crypto: SharedPeerCrypto<C, P>,
    local_identity_id: OpaqueId,
    ack_timeout: Duration,
}
impl<R, S, K, C, P> ReceiptPeerTransport<R, S, K, C, P> {
    pub const fn new(
        relationships: R,
        link: SharedPeerLink<S, K>,
        crypto: SharedPeerCrypto<C, P>,
        local_identity_id: OpaqueId,
        ack_timeout: Duration,
    ) -> Self {
        Self { relationships, link, crypto, local_identity_id, ack_timeout }
    }
}
impl<R, S, K, C, P> ControlTransport for ReceiptPeerTransport<R, S, K, C, P>
where
    R: ContactRepository + PeerCredentialRepository,
    S: ContactRepository + PeerCredentialRepository,
    K: HandshakeSigner,
    C: CryptoProvider,
    P: ProtectedSecretStore,
{
    fn send_control(&mut self, job: &ControlJob) -> Result<ControlAck, ControlTransportError> {
        if !matches!(job.kind, ControlKind::Receipt | ControlKind::Reaction) {
            return Err(ControlTransportError);
        }
        let contact_id = ContactId::from_opaque(job.contact_id);
        let contact =
            load_contact(&self.relationships, contact_id).map_err(|_| ControlTransportError)?;
        let credential =
            load_credential(&self.relationships, contact_id).map_err(|_| ControlTransportError)?;
        let encrypted = seal(
            &self.crypto,
            credential.secret_handle(),
            job.job_id,
            if job.kind == ControlKind::Reaction {
                REACTION_MESSAGE_KIND
            } else {
                RECEIPT_MESSAGE_KIND
            },
            self.local_identity_id,
            contact.remote_identity().identity_id().to_opaque(),
            &job.payload,
        )
        .map_err(|_| ControlTransportError)?;
        self.link
            .send_and_wait_ack(
                contact_id,
                job.job_id,
                if job.kind == ControlKind::Reaction {
                    REACTION_MESSAGE_KIND
                } else {
                    RECEIPT_MESSAGE_KIND
                },
                encrypted,
                self.ack_timeout,
            )
            .map(|ack| match ack {
                LinkAck::Accepted => ControlAck::Accepted,
                LinkAck::Duplicate => ControlAck::Duplicate,
            })
            .map_err(|_| ControlTransportError)
    }
}

/// Explicit text/receipt inbound handler. It consumes only envelopes selected by the central
/// communication dispatcher; attachment frames can never be stolen by this handler.
pub struct InboundTextReceiptAdapter<R, S, K, C, P, I, T> {
    relationships: R,
    link: SharedPeerLink<S, K>,
    crypto: SharedPeerCrypto<C, P>,
    inbound: I,
    control: SharedControlWorker<T>,
    engine: EngineHandle,
    local_identity_id: OpaqueId,
    database_writes: u64,
}
impl<R, S, K, C, P, I, T> InboundTextReceiptAdapter<R, S, K, C, P, I, T> {
    pub const fn new(
        relationships: R,
        link: SharedPeerLink<S, K>,
        crypto: SharedPeerCrypto<C, P>,
        inbound: I,
        control: SharedControlWorker<T>,
        engine: EngineHandle,
        local_identity_id: OpaqueId,
    ) -> Self {
        Self {
            relationships,
            link,
            crypto,
            inbound,
            control,
            engine,
            local_identity_id,
            database_writes: 0,
        }
    }
}
impl<R, S, K, C, P, I, T> InboundMessagingRuntime for InboundTextReceiptAdapter<R, S, K, C, P, I, T>
where
    R: ContactRepository + ConversationRepository + PeerCredentialRepository + Send + 'static,
    S: ContactRepository + PeerCredentialRepository + Send + 'static,
    K: HandshakeSigner + Send + 'static,
    C: CryptoProvider + Send + 'static,
    P: ProtectedSecretStore + Send + 'static,
    I: InboundMessageStore + Send + 'static,
    T: ControlTransport + Send + 'static,
{
    fn process(
        &mut self,
        envelope: InboundEnvelope,
        now: Timestamp,
    ) -> Result<(), CommunicationError> {
        let contact = load_contact(&self.relationships, envelope.contact_id)?;
        let credential = load_credential(&self.relationships, contact.id())?;
        let plaintext = open(
            &self.crypto,
            credential.secret_handle(),
            envelope.envelope_id,
            envelope.message_kind,
            self.local_identity_id,
            contact.remote_identity().identity_id().to_opaque(),
            &envelope.ciphertext,
        )?;
        let payload =
            ApplicationPayloadCodec::decode(&plaintext).map_err(|_| CommunicationError::Inbound)?;
        match (envelope.message_kind, payload) {
            (TEXT_MESSAGE_KIND, ApplicationPayload::Text(text)) => {
                if text.message_id != envelope.envelope_id {
                    return self.reject(&envelope);
                }
                let conversation =
                    ConversationRepository::for_contact(&self.relationships, contact.id())
                        .map_err(|_| CommunicationError::Inbound)?
                        .ok_or(CommunicationError::Inbound)?;
                let message = inbound_message(&conversation, text)?;
                let inserted = self
                    .inbound
                    .persist_inbound(envelope.envelope_id, message)
                    .map_err(|_| CommunicationError::Inbound)?;
                if inserted {
                    self.database_writes = self.database_writes.saturating_add(1);
                }
                let receipt = ReceiptPayload {
                    receipt_id: ReceiptId::deterministic_for(
                        MessageId::from_opaque(envelope.envelope_id),
                        ReceiptKind::Delivered,
                    )
                    .to_opaque(),
                    message_id: envelope.envelope_id,
                    contact_id: contact.id().to_opaque(),
                    kind: DeliveryReceiptKind::Delivered,
                    at: now,
                };
                self.control.ensure_receipt(contact.id(), receipt, now)?;
                self.link
                    .send_ack(
                        contact.id(),
                        envelope.envelope_id,
                        if inserted { AckStatus::Accepted } else { AckStatus::Duplicate },
                    )
                    .map_err(|_| CommunicationError::Peer)?;
                Ok(())
            }
            (RECEIPT_MESSAGE_KIND, ApplicationPayload::Receipt(receipt)) => {
                if receipt.receipt_id != envelope.envelope_id {
                    return self.reject(&envelope);
                }
                let _ = self
                    .engine
                    .dispatch(EngineCommand::ApplyReceipt(Receipt {
                        id: ReceiptId::from_opaque(receipt.receipt_id),
                        message_id: MessageId::from_opaque(receipt.message_id),
                        kind: match receipt.kind {
                            DeliveryReceiptKind::Delivered => ReceiptKind::Delivered,
                            DeliveryReceiptKind::Read => ReceiptKind::Read,
                        },
                        at: receipt.at,
                    }))
                    .map_err(|_| CommunicationError::Engine)?;
                self.link
                    .send_ack(contact.id(), envelope.envelope_id, AckStatus::Accepted)
                    .map_err(|_| CommunicationError::Peer)
            }
            (REACTION_MESSAGE_KIND, ApplicationPayload::Reaction(reaction)) => {
                if reaction.reaction_id != envelope.envelope_id {
                    return self.reject(&envelope);
                }
                let domain = torca_messaging::MessageReaction::new(
                    MessageId::from_opaque(reaction.message_id),
                    torca_conversations::ConversationId::from_opaque(reaction.conversation_id),
                    reaction.actor_id,
                    reaction.emoji,
                    reaction.active,
                    reaction.at,
                )
                .map_err(|_| CommunicationError::Inbound)?;
                let _ = self
                    .engine
                    .dispatch(EngineCommand::SetMessageReaction { reaction: domain })
                    .map_err(|_| CommunicationError::Engine)?;
                self.link
                    .send_ack(contact.id(), envelope.envelope_id, AckStatus::Accepted)
                    .map_err(|_| CommunicationError::Peer)
            }
            _ => self.reject(&envelope),
        }
    }

    fn database_write_count(&self) -> u64 {
        self.database_writes
    }
}
impl<R, S, K, C, P, I, T> InboundTextReceiptAdapter<R, S, K, C, P, I, T>
where
    S: ContactRepository + PeerCredentialRepository,
    K: HandshakeSigner,
{
    fn reject(&self, envelope: &InboundEnvelope) -> Result<(), CommunicationError> {
        let peer = peer_envelope(envelope);
        let _ = self.link.send_ack(peer.contact_id, peer.envelope_id, AckStatus::Rejected);
        Err(CommunicationError::Inbound)
    }
}

fn inbound_message(
    conversation: &DirectConversation,
    payload: TextPayload,
) -> Result<Message, CommunicationError> {
    let body = MessageBody::new(payload.body).map_err(|_| CommunicationError::Inbound)?;
    let reply_to =
        payload.reply_to.map(|id| ReplyReference { message_id: MessageId::from_opaque(id) });
    Ok(Message::inbound(
        MessageId::from_opaque(payload.message_id),
        conversation.id(),
        body,
        reply_to,
        payload.sent_at,
    ))
}

pub(crate) fn load_contact<R: ContactRepository>(
    repository: &R,
    contact_id: ContactId,
) -> Result<Contact, CommunicationError> {
    repository
        .get(contact_id)
        .map_err(|_| CommunicationError::Peer)?
        .ok_or(CommunicationError::Peer)
}

pub(crate) fn load_credential<R: PeerCredentialRepository>(
    repository: &R,
    contact_id: ContactId,
) -> Result<PeerCredential, CommunicationError> {
    repository
        .credential_for_contact(contact_id)
        .map_err(|_| CommunicationError::Peer)?
        .ok_or(CommunicationError::Peer)
}

pub(crate) fn seal<C, P>(
    crypto: &SharedPeerCrypto<C, P>,
    handle: OpaqueId,
    envelope_id: OpaqueId,
    message_kind: u16,
    local_identity: OpaqueId,
    remote_identity: OpaqueId,
    plaintext: &[u8],
) -> Result<Vec<u8>, CommunicationError>
where
    C: CryptoProvider,
    P: ProtectedSecretStore,
{
    let mut secrets = crypto.inner.lock().map_err(|_| CommunicationError::Peer)?;
    let nonce = secrets.peer_nonce().map_err(|_| CommunicationError::Peer)?;
    let ciphertext = secrets
        .seal_peer_payload(
            handle,
            nonce,
            &peer_aad(envelope_id, message_kind, local_identity, remote_identity),
            plaintext,
        )
        .map_err(|_| CommunicationError::Peer)?;
    let mut output = Vec::with_capacity(NONCE_BYTES + ciphertext.0.len());
    output.extend_from_slice(&nonce.0);
    output.extend_from_slice(&ciphertext.0);
    Ok(output)
}

pub(crate) fn open<C, P>(
    crypto: &SharedPeerCrypto<C, P>,
    handle: OpaqueId,
    envelope_id: OpaqueId,
    message_kind: u16,
    local_identity: OpaqueId,
    remote_identity: OpaqueId,
    stored: &[u8],
) -> Result<Vec<u8>, CommunicationError>
where
    C: CryptoProvider,
    P: ProtectedSecretStore,
{
    if stored.len() <= NONCE_BYTES {
        return Err(CommunicationError::Inbound);
    }
    let nonce = Nonce(stored[..NONCE_BYTES].try_into().map_err(|_| CommunicationError::Inbound)?);
    let ciphertext = Ciphertext(stored[NONCE_BYTES..].to_vec());
    crypto
        .inner
        .lock()
        .map_err(|_| CommunicationError::Inbound)?
        .open_peer_payload(
            handle,
            nonce,
            &peer_aad(envelope_id, message_kind, local_identity, remote_identity),
            &ciphertext,
        )
        .map_err(|_| CommunicationError::Inbound)
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
    let mut aad = Vec::with_capacity(PEER_AAD_LABEL.len() + 50);
    aad.extend_from_slice(PEER_AAD_LABEL);
    aad.extend_from_slice(envelope_id.as_bytes());
    aad.extend_from_slice(&message_kind.to_be_bytes());
    aad.extend_from_slice(first.as_bytes());
    aad.extend_from_slice(second.as_bytes());
    aad
}

fn map_ack(ack: LinkAck) -> DeliveryAck {
    match ack {
        LinkAck::Accepted => DeliveryAck::Accepted,
        LinkAck::Duplicate => DeliveryAck::Duplicate,
    }
}

#[allow(dead_code)]
fn _map_peer(_: PeerLinkError) -> CommunicationError {
    CommunicationError::Peer
}
