//! Single-writer client engine coordinating identity, pairing and messaging.

use core::fmt;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

use torca_contacts::{Contact, ContactId, ContactRepository, InMemoryContactRepository};
use torca_conversations::{
    ConversationId, ConversationRepository, DirectConversation, InMemoryConversationRepository,
};
use torca_foundation::{ErrorCode, Timestamp};
use torca_identity::{
    CreateIdentity, DeterministicKeyProvider, Identity, IdentityId, IdentityKeyProvider,
    IdentityRepository, IdentityService, InMemoryIdentityRepository, Profile,
};
use torca_messaging::{
    InMemoryMessageRepository, Message, MessageBody, MessageId, MessageRepository, ReplyReference,
};
use torca_pairing::{
    InMemoryPairingRepository, PairingCode, PairingRepository, PairingSession, PairingSessionId,
    PeerProposal,
};
use torca_receipts::{InMemoryReceiptRepository, Receipt, ReceiptRepository};

/// Command accepted by the single-writer client engine.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EngineCommand {
    CreateIdentity {
        identity_id: IdentityId,
        profile: Profile,
        at: Timestamp,
    },
    StartPairing {
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
    RemoteApproved {
        session_id: PairingSessionId,
        at: Timestamp,
    },
    CompletePairing {
        session_id: PairingSessionId,
        contact_id: ContactId,
        conversation_id: ConversationId,
        at: Timestamp,
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

/// Result returned after one engine command.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EngineResult {
    IdentityCreated,
    PairingStarted,
    PairingUpdated,
    PairingCompleted { contact_id: ContactId, conversation_id: ConversationId },
    MessageQueued { message_id: MessageId },
    MessageUpdated { message_id: MessageId },
    ReceiptApplied { message_id: MessageId, changed: bool },
}

/// Immutable application snapshot consumed by projections and presentation bridges.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientSnapshot {
    pub identity: Option<Identity>,
    pub pairings: Vec<PairingSession>,
    pub contacts: Vec<Contact>,
    pub conversations: Vec<DirectConversation>,
    pub messages: Vec<Message>,
}

/// Redaction-safe application engine failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EngineError(pub String);
impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for EngineError {}

/// Client engine parameterized by inward-facing persistence and key-management ports.
///
/// The default type parameters intentionally preserve the lightweight in-memory composition for
/// tests and explicit previews. Production composition injects SQLCipher repositories and a
/// protected production identity key provider without changing engine workflow code.
pub struct ClientEngine<
    I = InMemoryIdentityRepository,
    K = DeterministicKeyProvider,
    P = InMemoryPairingRepository,
    C = InMemoryContactRepository,
    V = InMemoryConversationRepository,
    M = InMemoryMessageRepository,
    R = InMemoryReceiptRepository,
> {
    identity: IdentityService<I, K>,
    pairings: P,
    contacts: C,
    conversations: V,
    messages: M,
    receipts: R,
}

impl Default
    for ClientEngine<
        InMemoryIdentityRepository,
        DeterministicKeyProvider,
        InMemoryPairingRepository,
        InMemoryContactRepository,
        InMemoryConversationRepository,
        InMemoryMessageRepository,
        InMemoryReceiptRepository,
    >
{
    fn default() -> Self {
        Self::new(
            InMemoryIdentityRepository::default(),
            DeterministicKeyProvider::default(),
            InMemoryPairingRepository::default(),
            InMemoryContactRepository::default(),
            InMemoryConversationRepository::default(),
            InMemoryMessageRepository::default(),
            InMemoryReceiptRepository::default(),
        )
    }
}

impl<I, K, P, C, V, M, R> ClientEngine<I, K, P, C, V, M, R>
where
    I: IdentityRepository,
    K: IdentityKeyProvider,
    P: PairingRepository,
    C: ContactRepository,
    V: ConversationRepository,
    M: MessageRepository,
    R: ReceiptRepository,
{
    /// Creates an engine from explicit ports.
    pub const fn new(
        identity_repository: I,
        key_provider: K,
        pairings: P,
        contacts: C,
        conversations: V,
        messages: M,
        receipts: R,
    ) -> Self {
        Self {
            identity: IdentityService::new(identity_repository, key_provider),
            pairings,
            contacts,
            conversations,
            messages,
            receipts,
        }
    }

    /// Executes one serialized application command.
    pub fn dispatch(&mut self, command: EngineCommand) -> Result<EngineResult, EngineError> {
        match command {
            EngineCommand::CreateIdentity { identity_id, profile, at } => {
                let (_identity, _event) = self
                    .identity
                    .create(CreateIdentity { identity_id, profile, at })
                    .map_err(map_error)?;
                Ok(EngineResult::IdentityCreated)
            }
            EngineCommand::StartPairing { session_id, code, expires_at } => {
                self.pairings
                    .insert(PairingSession::creator(session_id, code, expires_at))
                    .map_err(map_error)?;
                Ok(EngineResult::PairingStarted)
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
            EngineCommand::RemoteApproved { session_id, at } => {
                let mut session = self.load_pairing(session_id)?;
                session.approve_remote(at).map_err(map_error)?;
                self.pairings.update(session).map_err(map_error)?;
                Ok(EngineResult::PairingUpdated)
            }
            EngineCommand::CompletePairing { session_id, contact_id, conversation_id, at } => {
                if self.contacts.get(contact_id).map_err(map_error)?.is_some()
                    || self.conversations.get(conversation_id).map_err(map_error)?.is_some()
                    || self.conversations.for_contact(contact_id).map_err(map_error)?.is_some()
                {
                    return Err(EngineError("contact or conversation already exists".into()));
                }
                let mut session = self.load_pairing(session_id)?;
                let proposal = session.complete(at).map_err(map_error)?;
                self.contacts
                    .insert(Contact::new(contact_id, proposal.public_identity, proposal.route, at))
                    .map_err(map_error)?;
                self.conversations
                    .insert(DirectConversation::new(conversation_id, contact_id, at))
                    .map_err(map_error)?;
                self.pairings.update(session).map_err(map_error)?;
                Ok(EngineResult::PairingCompleted { contact_id, conversation_id })
            }
            EngineCommand::QueueMessage { message_id, conversation_id, body, reply_to, at } => {
                if self.conversations.get(conversation_id).map_err(map_error)?.is_none() {
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

    /// Reads the current application snapshot through injected repositories.
    pub fn snapshot(&self) -> Result<ClientSnapshot, EngineError> {
        Ok(ClientSnapshot {
            identity: self.identity.load().map_err(map_error)?,
            pairings: self.pairings.list().map_err(map_error)?,
            contacts: self.contacts.list().map_err(map_error)?,
            conversations: self.conversations.list().map_err(map_error)?,
            messages: self.messages.list().map_err(map_error)?,
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

/// Runtime interface hidden behind the single-writer actor.
pub trait EngineRuntime: Send + 'static {
    /// Executes one command.
    fn dispatch(&mut self, command: EngineCommand) -> Result<EngineResult, EngineError>;
    /// Reads one snapshot.
    fn snapshot(&self) -> Result<ClientSnapshot, EngineError>;
}

impl<I, K, P, C, V, M, R> EngineRuntime for ClientEngine<I, K, P, C, V, M, R>
where
    I: IdentityRepository + Send + 'static,
    K: IdentityKeyProvider + Send + 'static,
    P: PairingRepository + Send + 'static,
    C: ContactRepository + Send + 'static,
    V: ConversationRepository + Send + 'static,
    M: MessageRepository + Send + 'static,
    R: ReceiptRepository + Send + 'static,
{
    fn dispatch(&mut self, command: EngineCommand) -> Result<EngineResult, EngineError> {
        ClientEngine::dispatch(self, command)
    }

    fn snapshot(&self) -> Result<ClientSnapshot, EngineError> {
        ClientEngine::snapshot(self)
    }
}

fn map_error(error: impl fmt::Display) -> EngineError {
    EngineError(error.to_string())
}

enum ActorRequest {
    Dispatch(EngineCommand, Sender<Result<EngineResult, EngineError>>),
    Snapshot(Sender<Result<ClientSnapshot, EngineError>>),
    Shutdown,
}

/// Cloneable handle used by bridges/workers to communicate with the engine actor.
#[derive(Clone)]
pub struct EngineHandle {
    sender: Sender<ActorRequest>,
}
impl EngineHandle {
    /// Dispatches a command through the engine actor.
    pub fn dispatch(&self, command: EngineCommand) -> Result<EngineResult, EngineError> {
        let (sender, receiver) = mpsc::channel();
        self.sender
            .send(ActorRequest::Dispatch(command, sender))
            .map_err(|_| EngineError("engine actor stopped".into()))?;
        receiver.recv().map_err(|_| EngineError("engine response channel closed".into()))?
    }

    /// Requests a current snapshot from the engine actor.
    pub fn snapshot(&self) -> Result<ClientSnapshot, EngineError> {
        let (sender, receiver) = mpsc::channel();
        self.sender
            .send(ActorRequest::Snapshot(sender))
            .map_err(|_| EngineError("engine actor stopped".into()))?;
        receiver.recv().map_err(|_| EngineError("engine response channel closed".into()))?
    }
}

/// Owner of the single engine thread.
pub struct ClientEngineActor {
    sender: Sender<ActorRequest>,
    join: Option<JoinHandle<()>>,
}
impl ClientEngineActor {
    /// Starts an actor around any engine runtime satisfying the application contract.
    pub fn spawn<E: EngineRuntime>(mut engine: E) -> (EngineHandle, Self) {
        let (sender, receiver): (Sender<ActorRequest>, Receiver<ActorRequest>) = mpsc::channel();
        let handle = EngineHandle { sender: sender.clone() };
        let join = thread::spawn(move || {
            while let Ok(request) = receiver.recv() {
                match request {
                    ActorRequest::Dispatch(command, response) => {
                        let _ = response.send(engine.dispatch(command));
                    }
                    ActorRequest::Snapshot(response) => {
                        let _ = response.send(engine.snapshot());
                    }
                    ActorRequest::Shutdown => break,
                }
            }
        });
        (handle, Self { sender, join: Some(join) })
    }

    /// Stops and joins the engine actor.
    pub fn shutdown(mut self) -> Result<(), EngineError> {
        self.sender
            .send(ActorRequest::Shutdown)
            .map_err(|_| EngineError("engine actor stopped".into()))?;
        if let Some(join) = self.join.take() {
            join.join().map_err(|_| EngineError("engine actor panicked".into()))?;
        }
        Ok(())
    }
}
