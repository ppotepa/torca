//! Single-writer client engine coordinating identity, pairing and messaging.

use core::fmt;
use std::sync::mpsc::{self, Receiver, Sender, SyncSender, TrySendError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use torca_contacts::{
    Contact, ContactError, ContactId, ContactRepository, InMemoryContactRepository,
    InMemoryPeerCredentialRepository, PeerCredential, PeerCredentialRepository,
};
use torca_conversations::{
    ConversationError, ConversationId, ConversationRepository, DirectConversation,
    InMemoryConversationRepository,
};
use torca_foundation::{ErrorCode, Timestamp};
use torca_identity::{
    CreateIdentity, DeterministicKeyProvider, Identity, IdentityId, IdentityKeyProvider,
    IdentityRepository, IdentityService, InMemoryIdentityRepository, Profile, ProfileName,
    UpdateProfile,
};
use torca_messaging::{
    InMemoryMessageRepository, Message, MessageBody, MessageId, MessageRepository, ReplyReference,
};
use torca_pairing::{
    InMemoryPairingRepository, PairingCode, PairingRepository, PairingSession, PairingSessionId,
    PeerProposal,
};
use torca_receipts::{InMemoryReceiptRepository, Receipt, ReceiptRepository};

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EngineCommand {
    CreateIdentity {
        identity_id: IdentityId,
        profile: Option<Profile>,
        at: Timestamp,
    },
    UpdateProfile {
        display_name: ProfileName,
        at: Timestamp,
    },
    StartPairing {
        session_id: PairingSessionId,
        code: PairingCode,
        expires_at: Timestamp,
    },
    JoinPairing {
        session_id: PairingSessionId,
        code: PairingCode,
        expires_at: Timestamp,
    },
    PeerJoined {
        session_id: PairingSessionId,
        proposal: PeerProposal,
        at: Timestamp,
    },
    ApprovePairing {
        session_id: PairingSessionId,
        at: Timestamp,
    },
    RejectPairing {
        session_id: PairingSessionId,
    },
    CancelPairing {
        session_id: PairingSessionId,
    },
    ExpirePairing {
        session_id: PairingSessionId,
        at: Timestamp,
    },
    RemoteApproved {
        session_id: PairingSessionId,
        at: Timestamp,
    },
    CompletePairing {
        session_id: PairingSessionId,
        contact_id: ContactId,
        conversation_id: ConversationId,
        credential: PeerCredential,
        at: Timestamp,
    },
    RemovePairing {
        session_id: PairingSessionId,
    },
    QueueMessage {
        message_id: MessageId,
        conversation_id: ConversationId,
        body: MessageBody,
        reply_to: Option<ReplyReference>,
        at: Timestamp,
    },
    BeginMessageSend {
        message_id: MessageId,
        at: Timestamp,
    },
    MarkMessageSent {
        message_id: MessageId,
        at: Timestamp,
    },
    MarkMessageFailed {
        message_id: MessageId,
        at: Timestamp,
        error_code: ErrorCode,
    },
    RetryMessage {
        message_id: MessageId,
        at: Timestamp,
    },
    ApplyReceipt(Receipt),
}

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EngineResult {
    IdentityCreated,
    ProfileUpdated,
    PairingStarted,
    PairingJoined,
    PairingUpdated,
    PairingRejected,
    PairingCancelled,
    PairingCompleted { contact_id: ContactId, conversation_id: ConversationId },
    PairingRemoved,
    MessageQueued { message_id: MessageId },
    MessageUpdated { message_id: MessageId },
    ReceiptApplied { message_id: MessageId, changed: bool },
}

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientSnapshot {
    pub identity: Option<Identity>,
    pub pairings: Vec<PairingSession>,
    pub contacts: Vec<Contact>,
    pub conversations: Vec<DirectConversation>,
    pub messages: Vec<Message>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EngineError(pub String);
impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for EngineError {}

pub trait RelationshipRepository:
    ContactRepository + ConversationRepository + PeerCredentialRepository
{
    fn insert_pairing_result(
        &mut self,
        contact: Contact,
        conversation: DirectConversation,
        credential: PeerCredential,
    ) -> Result<(), EngineError>;
}

#[derive(Clone, Debug, Default)]
pub struct InMemoryRelationshipRepository {
    contacts: InMemoryContactRepository,
    conversations: InMemoryConversationRepository,
    credentials: InMemoryPeerCredentialRepository,
}

impl ContactRepository for InMemoryRelationshipRepository {
    fn insert(&mut self, contact: Contact) -> Result<(), ContactError> {
        self.contacts.insert(contact)
    }
    fn get(&self, id: ContactId) -> Result<Option<Contact>, ContactError> {
        self.contacts.get(id)
    }
    fn update(&mut self, contact: Contact) -> Result<(), ContactError> {
        self.contacts.update(contact)
    }
    fn list(&self) -> Result<Vec<Contact>, ContactError> {
        self.contacts.list()
    }
}
impl ConversationRepository for InMemoryRelationshipRepository {
    fn insert(&mut self, conversation: DirectConversation) -> Result<(), ConversationError> {
        self.conversations.insert(conversation)
    }
    fn get(&self, id: ConversationId) -> Result<Option<DirectConversation>, ConversationError> {
        self.conversations.get(id)
    }
    fn for_contact(
        &self,
        contact_id: ContactId,
    ) -> Result<Option<DirectConversation>, ConversationError> {
        self.conversations.for_contact(contact_id)
    }
    fn list(&self) -> Result<Vec<DirectConversation>, ConversationError> {
        self.conversations.list()
    }
}
impl PeerCredentialRepository for InMemoryRelationshipRepository {
    fn insert_credential(&mut self, credential: PeerCredential) -> Result<(), ContactError> {
        self.credentials.insert_credential(credential)
    }
    fn credential_for_contact(
        &self,
        contact_id: ContactId,
    ) -> Result<Option<PeerCredential>, ContactError> {
        self.credentials.credential_for_contact(contact_id)
    }
}
impl RelationshipRepository for InMemoryRelationshipRepository {
    fn insert_pairing_result(
        &mut self,
        contact: Contact,
        conversation: DirectConversation,
        credential: PeerCredential,
    ) -> Result<(), EngineError> {
        if contact.id() != conversation.contact_id() || contact.id() != credential.contact_id() {
            return Err(EngineError("pairing relationship identifiers do not match".into()));
        }
        if ContactRepository::get(self, contact.id()).map_err(map_error)?.is_some()
            || ConversationRepository::get(self, conversation.id()).map_err(map_error)?.is_some()
            || ConversationRepository::for_contact(self, contact.id()).map_err(map_error)?.is_some()
            || PeerCredentialRepository::credential_for_contact(self, contact.id())
                .map_err(map_error)?
                .is_some()
        {
            return Err(EngineError("contact, conversation or credential already exists".into()));
        }
        let mut contacts = self.contacts.clone();
        let mut conversations = self.conversations.clone();
        let mut credentials = self.credentials.clone();
        contacts.insert(contact).map_err(map_error)?;
        conversations.insert(conversation).map_err(map_error)?;
        credentials.insert_credential(credential).map_err(map_error)?;
        self.contacts = contacts;
        self.conversations = conversations;
        self.credentials = credentials;
        Ok(())
    }
}

pub struct ClientEngine<
    I = InMemoryIdentityRepository,
    K = DeterministicKeyProvider,
    P = InMemoryPairingRepository,
    L = InMemoryRelationshipRepository,
    M = InMemoryMessageRepository,
    R = InMemoryReceiptRepository,
> {
    identity: IdentityService<I, K>,
    pairings: P,
    relationships: L,
    messages: M,
    receipts: R,
}

impl Default
    for ClientEngine<
        InMemoryIdentityRepository,
        DeterministicKeyProvider,
        InMemoryPairingRepository,
        InMemoryRelationshipRepository,
        InMemoryMessageRepository,
        InMemoryReceiptRepository,
    >
{
    fn default() -> Self {
        Self::new(
            InMemoryIdentityRepository::default(),
            DeterministicKeyProvider::default(),
            InMemoryPairingRepository::default(),
            InMemoryRelationshipRepository::default(),
            InMemoryMessageRepository::default(),
            InMemoryReceiptRepository::default(),
        )
    }
}

impl<I, K, P, L, M, R> ClientEngine<I, K, P, L, M, R>
where
    I: IdentityRepository,
    K: IdentityKeyProvider,
    P: PairingRepository,
    L: RelationshipRepository,
    M: MessageRepository,
    R: ReceiptRepository,
{
    pub const fn new(
        identity_repository: I,
        key_provider: K,
        pairings: P,
        relationships: L,
        messages: M,
        receipts: R,
    ) -> Self {
        Self {
            identity: IdentityService::new(identity_repository, key_provider),
            pairings,
            relationships,
            messages,
            receipts,
        }
    }

    #[allow(clippy::too_many_lines)]
    pub fn dispatch(&mut self, command: EngineCommand) -> Result<EngineResult, EngineError> {
        match command {
            EngineCommand::CreateIdentity { identity_id, profile, at } => {
                let (_identity, _event) = self
                    .identity
                    .create(CreateIdentity { identity_id, profile, at })
                    .map_err(map_error)?;
                Ok(EngineResult::IdentityCreated)
            }
            EngineCommand::UpdateProfile { display_name, at } => {
                let profile = Profile::new(display_name, None);
                let (_identity, _event) = self
                    .identity
                    .update_profile(UpdateProfile { profile, at })
                    .map_err(map_error)?;
                Ok(EngineResult::ProfileUpdated)
            }
            EngineCommand::StartPairing { session_id, code, expires_at } => {
                self.pairings
                    .insert(PairingSession::creator(session_id, code, expires_at))
                    .map_err(map_error)?;
                Ok(EngineResult::PairingStarted)
            }
            EngineCommand::JoinPairing { session_id, code, expires_at } => {
                self.pairings
                    .insert(PairingSession::joining(session_id, code, expires_at))
                    .map_err(map_error)?;
                Ok(EngineResult::PairingJoined)
            }
            EngineCommand::PeerJoined { session_id, proposal, at } => {
                let mut session = self.load_pairing(session_id)?;
                session.peer_joined(proposal, at).map_err(map_error)?;
                self.pairings.update(session).map_err(map_error)?;
                Ok(EngineResult::PairingUpdated)
            }
            EngineCommand::ApprovePairing { session_id, at } => {
                let mut session = self.load_pairing(session_id)?;
                session.approve_local(at).map_err(map_error)?;
                self.pairings.update(session).map_err(map_error)?;
                Ok(EngineResult::PairingUpdated)
            }
            EngineCommand::RejectPairing { session_id } => {
                let mut session = self.load_pairing(session_id)?;
                session.reject().map_err(map_error)?;
                self.pairings.update(session).map_err(map_error)?;
                Ok(EngineResult::PairingRejected)
            }
            EngineCommand::CancelPairing { session_id } => {
                let mut session = self.load_pairing(session_id)?;
                session.cancel().map_err(map_error)?;
                self.pairings.update(session).map_err(map_error)?;
                Ok(EngineResult::PairingCancelled)
            }
            EngineCommand::ExpirePairing { session_id, at } => {
                let mut session = self.load_pairing(session_id)?;
                if !session.expire(at) {
                    return Err(EngineError("pairing session is not due to expire".into()));
                }
                self.pairings.update(session).map_err(map_error)?;
                Ok(EngineResult::PairingUpdated)
            }
            EngineCommand::RemoteApproved { session_id, at } => {
                let mut session = self.load_pairing(session_id)?;
                session.approve_remote(at).map_err(map_error)?;
                self.pairings.update(session).map_err(map_error)?;
                Ok(EngineResult::PairingUpdated)
            }
            EngineCommand::CompletePairing {
                session_id,
                contact_id,
                conversation_id,
                credential,
                at,
            } => {
                if credential.contact_id() != contact_id {
                    return Err(EngineError(
                        "peer credential contact does not match pairing".into(),
                    ));
                }
                let existing_contact =
                    ContactRepository::get(&self.relationships, contact_id).map_err(map_error)?;
                let existing_conversation =
                    ConversationRepository::get(&self.relationships, conversation_id)
                        .map_err(map_error)?;
                let existing_for_contact =
                    ConversationRepository::for_contact(&self.relationships, contact_id)
                        .map_err(map_error)?;
                let existing_credential = PeerCredentialRepository::credential_for_contact(
                    &self.relationships,
                    contact_id,
                )
                .map_err(map_error)?;
                if existing_contact.is_some()
                    && existing_conversation
                        .as_ref()
                        .is_some_and(|conversation| conversation.contact_id() == contact_id)
                    && existing_for_contact.is_some()
                    && existing_credential.is_some()
                {
                    return Ok(EngineResult::PairingCompleted { contact_id, conversation_id });
                }
                if existing_contact.is_some()
                    || existing_conversation.is_some()
                    || existing_for_contact.is_some()
                    || existing_credential.is_some()
                {
                    return Err(EngineError(
                        "contact, conversation or peer credential already exists".into(),
                    ));
                }
                let mut session = self.load_pairing(session_id)?;
                let proposal = session.complete(at).map_err(map_error)?;
                let contact =
                    Contact::new(contact_id, proposal.public_identity, proposal.route, at);
                let conversation = DirectConversation::new(conversation_id, contact_id, at);
                self.relationships.insert_pairing_result(contact, conversation, credential)?;
                let _ = self.pairings.update(session);
                Ok(EngineResult::PairingCompleted { contact_id, conversation_id })
            }
            EngineCommand::RemovePairing { session_id } => {
                self.pairings.delete(session_id).map_err(map_error)?;
                Ok(EngineResult::PairingRemoved)
            }
            EngineCommand::QueueMessage { message_id, conversation_id, body, reply_to, at } => {
                if ConversationRepository::get(&self.relationships, conversation_id)
                    .map_err(map_error)?
                    .is_none()
                {
                    return Err(EngineError("conversation not found".into()));
                }
                self.messages
                    .insert(Message::outbound(message_id, conversation_id, body, reply_to, at))
                    .map_err(map_error)?;
                Ok(EngineResult::MessageQueued { message_id })
            }
            EngineCommand::BeginMessageSend { message_id, at } => {
                let mut message = self.load_message(message_id)?;
                let _ = message.begin_send(at).map_err(map_error)?;
                self.messages.update(message).map_err(map_error)?;
                Ok(EngineResult::MessageUpdated { message_id })
            }
            EngineCommand::MarkMessageSent { message_id, at } => {
                let mut message = self.load_message(message_id)?;
                message.mark_sent(at).map_err(map_error)?;
                self.messages.update(message).map_err(map_error)?;
                Ok(EngineResult::MessageUpdated { message_id })
            }
            EngineCommand::MarkMessageFailed { message_id, at, error_code } => {
                let mut message = self.load_message(message_id)?;
                message.mark_failed(at, error_code).map_err(map_error)?;
                self.messages.update(message).map_err(map_error)?;
                Ok(EngineResult::MessageUpdated { message_id })
            }
            EngineCommand::RetryMessage { message_id, at } => {
                let mut message = self.load_message(message_id)?;
                message.retry(at).map_err(map_error)?;
                self.messages.update(message).map_err(map_error)?;
                Ok(EngineResult::MessageUpdated { message_id })
            }
            EngineCommand::ApplyReceipt(receipt) => {
                let mut message = self.load_message(receipt.message_id)?;
                let changed = receipt.apply(&mut message).map_err(map_error)?;
                let _ = self.receipts.record(receipt).map_err(map_error)?;
                self.messages.update(message).map_err(map_error)?;
                Ok(EngineResult::ReceiptApplied { message_id: receipt.message_id, changed })
            }
        }
    }

    pub fn snapshot(&self) -> Result<ClientSnapshot, EngineError> {
        let mut snapshot = self.overview_snapshot()?;
        snapshot.messages = self.messages.list().map_err(map_error)?;
        Ok(snapshot)
    }

    pub fn overview_snapshot(&self) -> Result<ClientSnapshot, EngineError> {
        Ok(ClientSnapshot {
            identity: self.identity.load().map_err(map_error)?,
            pairings: self.pairings.list().map_err(map_error)?,
            contacts: ContactRepository::list(&self.relationships).map_err(map_error)?,
            conversations: ConversationRepository::list(&self.relationships).map_err(map_error)?,
            messages: Vec::new(),
        })
    }

    fn load_pairing(&self, id: PairingSessionId) -> Result<PairingSession, EngineError> {
        self.pairings
            .get(id)
            .map_err(map_error)?
            .ok_or_else(|| EngineError("pairing session not found".into()))
    }

    fn load_message(&self, id: MessageId) -> Result<Message, EngineError> {
        self.messages
            .get(id)
            .map_err(map_error)?
            .ok_or_else(|| EngineError("message not found".into()))
    }
}

pub trait EngineRuntime: Send + 'static {
    fn dispatch(&mut self, command: EngineCommand) -> Result<EngineResult, EngineError>;
    fn snapshot(&self) -> Result<ClientSnapshot, EngineError>;
    fn overview_snapshot(&self) -> Result<ClientSnapshot, EngineError> {
        self.snapshot()
    }
}

impl<I, K, P, L, M, R> EngineRuntime for ClientEngine<I, K, P, L, M, R>
where
    I: IdentityRepository + Send + 'static,
    K: IdentityKeyProvider + Send + 'static,
    P: PairingRepository + Send + 'static,
    L: RelationshipRepository + Send + 'static,
    M: MessageRepository + Send + 'static,
    R: ReceiptRepository + Send + 'static,
{
    fn dispatch(&mut self, command: EngineCommand) -> Result<EngineResult, EngineError> {
        ClientEngine::dispatch(self, command)
    }
    fn snapshot(&self) -> Result<ClientSnapshot, EngineError> {
        ClientEngine::snapshot(self)
    }
    fn overview_snapshot(&self) -> Result<ClientSnapshot, EngineError> {
        ClientEngine::overview_snapshot(self)
    }
}

fn map_error(error: impl fmt::Display) -> EngineError {
    EngineError(error.to_string())
}

enum ActorRequest {
    Dispatch(EngineCommand, Sender<Result<EngineResult, EngineError>>),
    Snapshot(Sender<Result<ClientSnapshot, EngineError>>),
    OverviewSnapshot(Sender<Result<ClientSnapshot, EngineError>>),
    Shutdown,
}

#[derive(Clone)]
pub struct EngineHandle {
    sender: SyncSender<ActorRequest>,
}
impl EngineHandle {
    pub fn dispatch(&self, command: EngineCommand) -> Result<EngineResult, EngineError> {
        let (sender, receiver) = mpsc::channel();
        send_with_timeout(&self.sender, ActorRequest::Dispatch(command, sender))?;
        receiver
            .recv_timeout(Duration::from_secs(10))
            .map_err(|_| EngineError("engine response timed out".into()))?
    }
    pub fn snapshot(&self) -> Result<ClientSnapshot, EngineError> {
        let (sender, receiver) = mpsc::channel();
        send_with_timeout(&self.sender, ActorRequest::Snapshot(sender))?;
        receiver
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| EngineError("engine snapshot timed out".into()))?
    }
    pub fn overview_snapshot(&self) -> Result<ClientSnapshot, EngineError> {
        let (sender, receiver) = mpsc::channel();
        send_with_timeout(&self.sender, ActorRequest::OverviewSnapshot(sender))?;
        receiver
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| EngineError("engine overview timed out".into()))?
    }
}

pub struct ClientEngineActor {
    sender: SyncSender<ActorRequest>,
    join: Option<JoinHandle<()>>,
}
impl ClientEngineActor {
    pub fn spawn<E: EngineRuntime>(mut engine: E) -> (EngineHandle, Self) {
        let (sender, receiver): (SyncSender<ActorRequest>, Receiver<ActorRequest>) =
            mpsc::sync_channel(256);
        let handle = EngineHandle { sender: sender.clone() };
        let join = thread::spawn(move || {
            loop {
                let request = match receiver.recv_timeout(Duration::from_secs(1)) {
                    Ok(request) => request,
                    Err(mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                };
                match request {
                    ActorRequest::Dispatch(command, response) => {
                        let _ = response.send(engine.dispatch(command));
                    }
                    ActorRequest::Snapshot(response) => {
                        let _ = response.send(engine.snapshot());
                    }
                    ActorRequest::OverviewSnapshot(response) => {
                        let _ = response.send(engine.overview_snapshot());
                    }
                    ActorRequest::Shutdown => break,
                }
            }
        });
        (handle, Self { sender, join: Some(join) })
    }

    pub fn shutdown(mut self) -> Result<(), EngineError> {
        send_with_timeout(&self.sender, ActorRequest::Shutdown)?;
        if let Some(join) = self.join.take() {
            join.join().map_err(|_| EngineError("engine actor panicked".into()))?;
        }
        Ok(())
    }
}

fn send_with_timeout(
    sender: &SyncSender<ActorRequest>,
    mut request: ActorRequest,
) -> Result<(), EngineError> {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match sender.try_send(request) {
            Ok(()) => return Ok(()),
            Err(TrySendError::Disconnected(_)) => {
                return Err(EngineError("engine actor stopped".into()));
            }
            Err(TrySendError::Full(returned)) => {
                if Instant::now() >= deadline {
                    return Err(EngineError("engine actor mailbox timed out".into()));
                }
                request = returned;
                thread::yield_now();
            }
        }
    }
}
