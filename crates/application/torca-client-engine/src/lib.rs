//! Single-writer client engine coordinating identity, pairing and messaging.

use core::fmt;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use torca_contacts::{Contact, ContactId, ContactRepository, InMemoryContactRepository};
use torca_conversations::{ConversationId, ConversationRepository, DirectConversation, InMemoryConversationRepository};
use torca_foundation::{ErrorCode, Timestamp};
use torca_identity::{CreateIdentity, DeterministicKeyProvider, Identity, IdentityId, IdentityService, InMemoryIdentityRepository, Profile};
use torca_messaging::{InMemoryMessageRepository, Message, MessageBody, MessageId, MessageRepository, ReplyReference};
use torca_pairing::{InMemoryPairingRepository, PairingCode, PairingRepository, PairingSession, PairingSessionId, PeerProposal};
use torca_receipts::{InMemoryReceiptRepository, Receipt, ReceiptRepository};

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EngineCommand {
    CreateIdentity { identity_id: IdentityId, profile: Profile, at: Timestamp },
    StartPairing { session_id: PairingSessionId, code: PairingCode, expires_at: Timestamp },
    PeerJoined { session_id: PairingSessionId, proposal: PeerProposal, at: Timestamp },
    ApprovePairing { session_id: PairingSessionId, at: Timestamp },
    RemoteApproved { session_id: PairingSessionId, at: Timestamp },
    CompletePairing { session_id: PairingSessionId, contact_id: ContactId, conversation_id: ConversationId, at: Timestamp },
    QueueMessage { message_id: MessageId, conversation_id: ConversationId, body: MessageBody, reply_to: Option<ReplyReference>, at: Timestamp },
    BeginMessageSend { message_id: MessageId, at: Timestamp },
    MarkMessageSent { message_id: MessageId, at: Timestamp },
    MarkMessageFailed { message_id: MessageId, at: Timestamp, error_code: ErrorCode },
    RetryMessage { message_id: MessageId, at: Timestamp },
    ApplyReceipt(Receipt),
}
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EngineResult { IdentityCreated, PairingStarted, PairingUpdated, PairingCompleted { contact_id: ContactId, conversation_id: ConversationId }, MessageQueued { message_id: MessageId }, MessageUpdated { message_id: MessageId }, ReceiptApplied { message_id: MessageId, changed: bool } }
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientSnapshot { pub identity: Option<Identity>, pub pairings: Vec<PairingSession>, pub contacts: Vec<Contact>, pub conversations: Vec<DirectConversation>, pub messages: Vec<Message> }
#[derive(Clone, Debug, Eq, PartialEq)] pub struct EngineError(pub String);
impl fmt::Display for EngineError { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(&self.0) } } impl std::error::Error for EngineError {}

pub struct ClientEngine { identity: IdentityService<InMemoryIdentityRepository, DeterministicKeyProvider>, pairings: InMemoryPairingRepository, contacts: InMemoryContactRepository, conversations: InMemoryConversationRepository, messages: InMemoryMessageRepository, receipts: InMemoryReceiptRepository }
impl Default for ClientEngine { fn default() -> Self { Self { identity: IdentityService::new(InMemoryIdentityRepository::default(), DeterministicKeyProvider::default()), pairings: InMemoryPairingRepository::default(), contacts: InMemoryContactRepository::default(), conversations: InMemoryConversationRepository::default(), messages: InMemoryMessageRepository::default(), receipts: InMemoryReceiptRepository::default() } } }
impl ClientEngine {
    pub fn dispatch(&mut self, command: EngineCommand) -> Result<EngineResult, EngineError> {
        match command {
            EngineCommand::CreateIdentity { identity_id, profile, at } => { self.identity.create(CreateIdentity { identity_id, profile, at }).map_err(map_error)?; Ok(EngineResult::IdentityCreated) }
            EngineCommand::StartPairing { session_id, code, expires_at } => { self.pairings.insert(PairingSession::creator(session_id, code, expires_at)).map_err(map_error)?; Ok(EngineResult::PairingStarted) }
            EngineCommand::PeerJoined { session_id, proposal, at } => { let mut session = self.load_pairing(session_id)?; session.peer_joined(proposal, at).map_err(map_error)?; self.pairings.update(session).map_err(map_error)?; Ok(EngineResult::PairingUpdated) }
            EngineCommand::ApprovePairing { session_id, at } => { let mut session = self.load_pairing(session_id)?; session.approve_local(at).map_err(map_error)?; self.pairings.update(session).map_err(map_error)?; Ok(EngineResult::PairingUpdated) }
            EngineCommand::RemoteApproved { session_id, at } => { let mut session = self.load_pairing(session_id)?; session.approve_remote(at).map_err(map_error)?; self.pairings.update(session).map_err(map_error)?; Ok(EngineResult::PairingUpdated) }
            EngineCommand::CompletePairing { session_id, contact_id, conversation_id, at } => { if self.contacts.get(contact_id).map_err(map_error)?.is_some() || self.conversations.for_contact(contact_id).map_err(map_error)?.is_some() { return Err(EngineError("contact or conversation already exists".into())); } let mut session = self.load_pairing(session_id)?; let proposal = session.complete(at).map_err(map_error)?; self.contacts.insert(Contact::new(contact_id, proposal.public_identity, proposal.route, at)).map_err(map_error)?; self.conversations.insert(DirectConversation::new(conversation_id, contact_id, at)).map_err(map_error)?; self.pairings.update(session).map_err(map_error)?; Ok(EngineResult::PairingCompleted { contact_id, conversation_id }) }
            EngineCommand::QueueMessage { message_id, conversation_id, body, reply_to, at } => { if self.conversations.get(conversation_id).map_err(map_error)?.is_none() { return Err(EngineError("conversation not found".into())); } self.messages.insert(Message::outbound(message_id, conversation_id, body, reply_to, at)).map_err(map_error)?; Ok(EngineResult::MessageQueued { message_id }) }
            EngineCommand::BeginMessageSend { message_id, at } => { let mut message = self.load_message(message_id)?; let _ = message.begin_send(at).map_err(map_error)?; self.messages.update(message).map_err(map_error)?; Ok(EngineResult::MessageUpdated { message_id }) }
            EngineCommand::MarkMessageSent { message_id, at } => { let mut message = self.load_message(message_id)?; message.mark_sent(at).map_err(map_error)?; self.messages.update(message).map_err(map_error)?; Ok(EngineResult::MessageUpdated { message_id }) }
            EngineCommand::MarkMessageFailed { message_id, at, error_code } => { let mut message = self.load_message(message_id)?; message.mark_failed(at, error_code).map_err(map_error)?; self.messages.update(message).map_err(map_error)?; Ok(EngineResult::MessageUpdated { message_id }) }
            EngineCommand::RetryMessage { message_id, at } => { let mut message = self.load_message(message_id)?; message.retry(at).map_err(map_error)?; self.messages.update(message).map_err(map_error)?; Ok(EngineResult::MessageUpdated { message_id }) }
            EngineCommand::ApplyReceipt(receipt) => { let mut message = self.load_message(receipt.message_id)?; let changed = receipt.apply(&mut message).map_err(map_error)?; let _ = self.receipts.record(receipt).map_err(map_error)?; self.messages.update(message).map_err(map_error)?; Ok(EngineResult::ReceiptApplied { message_id: receipt.message_id, changed }) }
        }
    }
    pub fn snapshot(&self) -> Result<ClientSnapshot, EngineError> { Ok(ClientSnapshot { identity: self.identity.load().map_err(map_error)?, pairings: self.pairings.list().map_err(map_error)?, contacts: self.contacts.list().map_err(map_error)?, conversations: self.conversations.list().map_err(map_error)?, messages: self.messages.list().map_err(map_error)? }) }
    fn load_pairing(&self, id: PairingSessionId) -> Result<PairingSession, EngineError> { self.pairings.get(id).map_err(map_error)?.ok_or_else(|| EngineError("pairing session not found".into())) }
    fn load_message(&self, id: MessageId) -> Result<Message, EngineError> { self.messages.get(id).map_err(map_error)?.ok_or_else(|| EngineError("message not found".into())) }
}
fn map_error(error: impl fmt::Display) -> EngineError { EngineError(error.to_string()) }
enum ActorRequest { Dispatch(EngineCommand, Sender<Result<EngineResult, EngineError>>), Snapshot(Sender<Result<ClientSnapshot, EngineError>>), Shutdown }
#[derive(Clone)] pub struct EngineHandle { sender: Sender<ActorRequest> }
impl EngineHandle { pub fn dispatch(&self, command: EngineCommand) -> Result<EngineResult, EngineError> { let (sender, receiver) = mpsc::channel(); self.sender.send(ActorRequest::Dispatch(command, sender)).map_err(|_| EngineError("engine actor stopped".into()))?; receiver.recv().map_err(|_| EngineError("engine response channel closed".into()))? } pub fn snapshot(&self) -> Result<ClientSnapshot, EngineError> { let (sender, receiver) = mpsc::channel(); self.sender.send(ActorRequest::Snapshot(sender)).map_err(|_| EngineError("engine actor stopped".into()))?; receiver.recv().map_err(|_| EngineError("engine response channel closed".into()))? } }
pub struct ClientEngineActor { sender: Sender<ActorRequest>, join: Option<JoinHandle<()>> }
impl ClientEngineActor { pub fn spawn(mut engine: ClientEngine) -> (EngineHandle, Self) { let (sender, receiver): (Sender<ActorRequest>, Receiver<ActorRequest>) = mpsc::channel(); let handle = EngineHandle { sender: sender.clone() }; let join = thread::spawn(move || { while let Ok(request) = receiver.recv() { match request { ActorRequest::Dispatch(command, response) => { let _ = response.send(engine.dispatch(command)); } ActorRequest::Snapshot(response) => { let _ = response.send(engine.snapshot()); } ActorRequest::Shutdown => break } } }); (handle, Self { sender, join: Some(join) }) } pub fn shutdown(mut self) -> Result<(), EngineError> { self.sender.send(ActorRequest::Shutdown).map_err(|_| EngineError("engine actor stopped".into()))?; if let Some(join) = self.join.take() { join.join().map_err(|_| EngineError("engine actor panicked".into()))?; } Ok(()) } }
