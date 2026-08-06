//! Single-writer client engine baseline for identity and pairing workflows.

use core::fmt;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

use torca_contacts::{Contact, ContactId, ContactRepository, InMemoryContactRepository};
use torca_conversations::{ConversationId, ConversationRepository, DirectConversation, InMemoryConversationRepository};
use torca_foundation::Timestamp;
use torca_identity::{
    CreateIdentity, DeterministicKeyProvider, Identity, IdentityId, IdentityService,
    InMemoryIdentityRepository, Profile,
};
use torca_pairing::{
    InMemoryPairingRepository, PairingCode, PairingRepository, PairingSession, PairingSessionId,
    PeerProposal,
};

/// Commands accepted by the baseline engine.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EngineCommand {
    /// Creates the local installation identity.
    CreateIdentity { identity_id: IdentityId, profile: Profile, at: Timestamp },
    /// Opens a creator pairing session.
    StartPairing { session_id: PairingSessionId, code: PairingCode, expires_at: Timestamp },
    /// Records a joining peer proposal.
    PeerJoined { session_id: PairingSessionId, proposal: PeerProposal, at: Timestamp },
    /// Approves locally.
    ApprovePairing { session_id: PairingSessionId, at: Timestamp },
    /// Records remote approval.
    RemoteApproved { session_id: PairingSessionId, at: Timestamp },
    /// Completes pairing and creates verified contact plus direct conversation.
    CompletePairing { session_id: PairingSessionId, contact_id: ContactId, conversation_id: ConversationId, at: Timestamp },
}

/// Command result.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EngineResult { IdentityCreated, PairingStarted, PairingUpdated, PairingCompleted { contact_id: ContactId, conversation_id: ConversationId } }

/// Immutable engine snapshot rendered by clients.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientSnapshot { pub identity: Option<Identity>, pub pairings: Vec<PairingSession>, pub contacts: Vec<Contact>, pub conversations: Vec<DirectConversation> }

/// Redaction-safe engine error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EngineError(pub String);
impl fmt::Display for EngineError { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(&self.0) } }
impl std::error::Error for EngineError {}

/// In-memory baseline composition. Later batches replace ports without changing command ownership.
pub struct ClientEngine {
    identity: IdentityService<InMemoryIdentityRepository, DeterministicKeyProvider>,
    pairings: InMemoryPairingRepository,
    contacts: InMemoryContactRepository,
    conversations: InMemoryConversationRepository,
}
impl Default for ClientEngine {
    fn default() -> Self {
        Self { identity: IdentityService::new(InMemoryIdentityRepository::default(), DeterministicKeyProvider::default()), pairings: InMemoryPairingRepository::default(), contacts: InMemoryContactRepository::default(), conversations: InMemoryConversationRepository::default() }
    }
}
impl ClientEngine {
    /// Dispatches one mutation. Mutable access is the single-writer boundary.
    pub fn dispatch(&mut self, command: EngineCommand) -> Result<EngineResult, EngineError> {
        match command {
            EngineCommand::CreateIdentity { identity_id, profile, at } => {
                self.identity.create(CreateIdentity { identity_id, profile, at }).map_err(map_error)?;
                Ok(EngineResult::IdentityCreated)
            }
            EngineCommand::StartPairing { session_id, code, expires_at } => {
                self.pairings.insert(PairingSession::creator(session_id, code, expires_at)).map_err(map_error)?;
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
                let mut session = self.load_pairing(session_id)?;
                let proposal = session.complete(at).map_err(map_error)?;
                let contact = Contact::new(contact_id, proposal.public_identity, proposal.route, at);
                self.contacts.insert(contact).map_err(map_error)?;
                self.conversations.insert(DirectConversation::new(conversation_id, contact_id, at)).map_err(map_error)?;
                self.pairings.update(session).map_err(map_error)?;
                Ok(EngineResult::PairingCompleted { contact_id, conversation_id })
            }
        }
    }
    /// Builds an immutable snapshot.
    pub fn snapshot(&self) -> Result<ClientSnapshot, EngineError> {
        Ok(ClientSnapshot { identity: self.identity.load().map_err(map_error)?, pairings: self.pairings.list().map_err(map_error)?, contacts: self.contacts.list().map_err(map_error)?, conversations: self.conversations.list().map_err(map_error)? })
    }
    fn load_pairing(&self, id: PairingSessionId) -> Result<PairingSession, EngineError> { self.pairings.get(id).map_err(map_error)?.ok_or_else(|| EngineError("pairing session not found".into())) }
}

fn map_error(error: impl fmt::Display) -> EngineError { EngineError(error.to_string()) }

enum ActorRequest { Dispatch(EngineCommand, Sender<Result<EngineResult, EngineError>>), Snapshot(Sender<Result<ClientSnapshot, EngineError>>), Shutdown }

/// Cloneable handle to the single-writer actor.
#[derive(Clone)]
pub struct EngineHandle { sender: Sender<ActorRequest> }
impl EngineHandle {
    /// Dispatches a command synchronously through the actor thread.
    pub fn dispatch(&self, command: EngineCommand) -> Result<EngineResult, EngineError> {
        let (sender, receiver) = mpsc::channel();
        self.sender.send(ActorRequest::Dispatch(command, sender)).map_err(|_| EngineError("engine actor stopped".into()))?;
        receiver.recv().map_err(|_| EngineError("engine response channel closed".into()))?
    }
    /// Reads a snapshot through the actor thread.
    pub fn snapshot(&self) -> Result<ClientSnapshot, EngineError> {
        let (sender, receiver) = mpsc::channel();
        self.sender.send(ActorRequest::Snapshot(sender)).map_err(|_| EngineError("engine actor stopped".into()))?;
        receiver.recv().map_err(|_| EngineError("engine response channel closed".into()))?
    }
}

/// Owner of the engine actor thread.
pub struct ClientEngineActor { sender: Sender<ActorRequest>, join: Option<JoinHandle<()>> }
impl ClientEngineActor {
    /// Spawns the actor and returns its handle plus owner.
    pub fn spawn(mut engine: ClientEngine) -> (EngineHandle, Self) {
        let (sender, receiver): (Sender<ActorRequest>, Receiver<ActorRequest>) = mpsc::channel();
        let handle = EngineHandle { sender: sender.clone() };
        let join = thread::spawn(move || {
            while let Ok(request) = receiver.recv() {
                match request {
                    ActorRequest::Dispatch(command, response) => { let _ = response.send(engine.dispatch(command)); }
                    ActorRequest::Snapshot(response) => { let _ = response.send(engine.snapshot()); }
                    ActorRequest::Shutdown => break,
                }
            }
        });
        (handle, Self { sender, join: Some(join) })
    }
    /// Stops and joins the actor.
    pub fn shutdown(mut self) -> Result<(), EngineError> {
        self.sender.send(ActorRequest::Shutdown).map_err(|_| EngineError("engine actor stopped".into()))?;
        if let Some(join) = self.join.take() { join.join().map_err(|_| EngineError("engine actor panicked".into()))?; }
        Ok(())
    }
}
