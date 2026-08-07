//! Single background owner for Tor, pairing, peer sessions and durable delivery.

mod attachments;
pub use attachments::{AttachmentSendRequest, AttachmentView};

use std::collections::BTreeMap;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use torca_client_engine::{EngineCommand, EngineHandle};
use torca_contacts::{ContactId, ContactStatus};
use torca_conversations::ConversationId;
use torca_diagnostics::{Component, DiagnosticBuffer, DiagnosticCode, DiagnosticEvent, HealthState};
use torca_foundation::{ErrorCode, OpaqueId, Timestamp};
use torca_messaging::{MessageBody, MessageId};
use torca_pairing::{PairingCode, PairingSessionId};
use torca_peer_link::PeerConnectionState;

const RUNTIME_TICK: Duration = Duration::from_millis(100);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostTorState { Stopped, Starting, Ready, Degraded, Failed }

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairingInvitationView {
    pub session_id: PairingSessionId,
    pub code: PairingCode,
    pub uri: String,
    pub expires_at: Timestamp,
}

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NetworkSnapshot {
    pub tor: HostTorState,
    pub onion_address: Option<String>,
    pub peers: BTreeMap<ContactId, PeerConnectionState>,
    pub contact_names: BTreeMap<ContactId, String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeDriverError { Pairing, Communication, Tor, Engine }
impl core::fmt::Display for RuntimeDriverError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result { write!(f, "{self:?}") }
}
impl std::error::Error for RuntimeDriverError {}

pub trait PairingDriver: Send + 'static {
    fn create(&mut self, session_id: PairingSessionId, now: Timestamp) -> Result<PairingInvitationView, RuntimeDriverError>;
    fn join(&mut self, session_id: PairingSessionId, code: PairingCode, now: Timestamp) -> Result<(), RuntimeDriverError>;
    fn approve(&mut self, session_id: PairingSessionId, now: Timestamp) -> Result<(), RuntimeDriverError>;
    fn reject(&mut self, session_id: PairingSessionId) -> Result<(), RuntimeDriverError>;
    fn cancel(&mut self, session_id: PairingSessionId) -> Result<(), RuntimeDriverError>;
    fn maintenance(&mut self, now: Timestamp) -> Result<(), RuntimeDriverError>;
    fn shutdown(&mut self);
}

pub trait CommunicationDriver: Send + 'static {
    fn recover(&mut self, now: Timestamp) -> Result<(), RuntimeDriverError>;
    fn maintenance(&mut self, contacts: &[ContactId], now: Timestamp) -> Result<(), RuntimeDriverError>;
    fn connection_state(&self, contact_id: ContactId) -> PeerConnectionState;
    fn contact_names(&self) -> Result<BTreeMap<ContactId, String>, RuntimeDriverError>;
    fn rename_contact(&mut self, contact_id: ContactId, display_name: String, now: Timestamp) -> Result<(), RuntimeDriverError>;
    fn block_contact(&mut self, contact_id: ContactId, now: Timestamp) -> Result<(), RuntimeDriverError>;
    fn unblock_contact(&mut self, contact_id: ContactId, now: Timestamp) -> Result<(), RuntimeDriverError>;
    fn clear_conversation_history(&mut self, conversation_id: ConversationId) -> Result<(), RuntimeDriverError>;
    fn remove_contact(&mut self, contact_id: ContactId) -> Result<(), RuntimeDriverError>;
    fn mark_conversation_read(&mut self, conversation_id: OpaqueId, now: Timestamp) -> Result<(), RuntimeDriverError>;
    fn prepare_attachment(&mut self, request: &AttachmentSendRequest, now: Timestamp) -> Result<(), RuntimeDriverError>;
    fn retry_attachment(&mut self, attachment_id: OpaqueId, now: Timestamp) -> Result<(), RuntimeDriverError>;
    fn cancel_attachment(&mut self, attachment_id: OpaqueId, now: Timestamp) -> Result<(), RuntimeDriverError>;
    fn attachment_snapshot(&self) -> Result<Vec<AttachmentView>, RuntimeDriverError>;
    fn shutdown(&mut self);
}

pub trait TorDriver: Send + 'static {
    fn maintenance(&mut self, now: Timestamp) -> Result<(), RuntimeDriverError>;
    fn state(&self) -> HostTorState;
    fn onion_address(&self) -> Option<String>;
    fn shutdown(&mut self);
}

enum RuntimeCommand {
    CreatePairing(PairingSessionId, Sender<Result<PairingInvitationView, RuntimeDriverError>>),
    JoinPairing(PairingSessionId, PairingCode, Sender<Result<(), RuntimeDriverError>>),
    ApprovePairing(PairingSessionId, Sender<Result<(), RuntimeDriverError>>),
    RejectPairing(PairingSessionId, Sender<Result<(), RuntimeDriverError>>),
    CancelPairing(PairingSessionId, Sender<Result<(), RuntimeDriverError>>),
    RenameContact(ContactId, String, Sender<Result<(), RuntimeDriverError>>),
    BlockContact(ContactId, Sender<Result<(), RuntimeDriverError>>),
    UnblockContact(ContactId, Sender<Result<(), RuntimeDriverError>>),
    RemoveContact(ContactId, Sender<Result<(), RuntimeDriverError>>),
    ClearConversationHistory(ConversationId, Sender<Result<(), RuntimeDriverError>>),
    MarkConversationRead(OpaqueId, Sender<Result<(), RuntimeDriverError>>),
    QueueAttachment(AttachmentSendRequest, Sender<Result<(), RuntimeDriverError>>),
    RetryAttachment(OpaqueId, Sender<Result<(), RuntimeDriverError>>),
    CancelAttachment(OpaqueId, Sender<Result<(), RuntimeDriverError>>),
    AttachmentSnapshot(Sender<Result<Vec<AttachmentView>, RuntimeDriverError>>),
    NetworkSnapshot(Sender<Result<NetworkSnapshot, RuntimeDriverError>>),
    Diagnostics(Sender<String>),
    Wake,
    Shutdown(Sender<()>),
}

#[derive(Clone)]
pub struct RuntimeHostHandle { sender: Sender<RuntimeCommand> }
impl RuntimeHostHandle {
    pub fn create_pairing(&self, id: PairingSessionId) -> Result<PairingInvitationView, RuntimeDriverError> { request(&self.sender, |r| RuntimeCommand::CreatePairing(id, r)) }
    pub fn join_pairing(&self, id: PairingSessionId, code: PairingCode) -> Result<(), RuntimeDriverError> { request(&self.sender, |r| RuntimeCommand::JoinPairing(id, code, r)) }
    pub fn approve_pairing(&self, id: PairingSessionId) -> Result<(), RuntimeDriverError> { request(&self.sender, |r| RuntimeCommand::ApprovePairing(id, r)) }
    pub fn reject_pairing(&self, id: PairingSessionId) -> Result<(), RuntimeDriverError> { request(&self.sender, |r| RuntimeCommand::RejectPairing(id, r)) }
    pub fn cancel_pairing(&self, id: PairingSessionId) -> Result<(), RuntimeDriverError> { request(&self.sender, |r| RuntimeCommand::CancelPairing(id, r)) }
    pub fn rename_contact(&self, id: ContactId, name: String) -> Result<(), RuntimeDriverError> { request(&self.sender, |r| RuntimeCommand::RenameContact(id, name, r)) }
    pub fn block_contact(&self, id: ContactId) -> Result<(), RuntimeDriverError> { request(&self.sender, |r| RuntimeCommand::BlockContact(id, r)) }
    pub fn unblock_contact(&self, id: ContactId) -> Result<(), RuntimeDriverError> { request(&self.sender, |r| RuntimeCommand::UnblockContact(id, r)) }
    pub fn remove_contact(&self, id: ContactId) -> Result<(), RuntimeDriverError> { request(&self.sender, |r| RuntimeCommand::RemoveContact(id, r)) }
    pub fn clear_conversation_history(&self, id: ConversationId) -> Result<(), RuntimeDriverError> { request(&self.sender, |r| RuntimeCommand::ClearConversationHistory(id, r)) }
    pub fn mark_conversation_read(&self, id: OpaqueId) -> Result<(), RuntimeDriverError> { request(&self.sender, |r| RuntimeCommand::MarkConversationRead(id, r)) }
    pub fn queue_attachment(&self, request_value: AttachmentSendRequest) -> Result<(), RuntimeDriverError> { request(&self.sender, |r| RuntimeCommand::QueueAttachment(request_value, r)) }
    pub fn retry_attachment(&self, id: OpaqueId) -> Result<(), RuntimeDriverError> { request(&self.sender, |r| RuntimeCommand::RetryAttachment(id, r)) }
    pub fn cancel_attachment(&self, id: OpaqueId) -> Result<(), RuntimeDriverError> { request(&self.sender, |r| RuntimeCommand::CancelAttachment(id, r)) }
    pub fn attachment_snapshot(&self) -> Result<Vec<AttachmentView>, RuntimeDriverError> { request(&self.sender, RuntimeCommand::AttachmentSnapshot) }
    pub fn network_snapshot(&self) -> Result<NetworkSnapshot, RuntimeDriverError> { request(&self.sender, RuntimeCommand::NetworkSnapshot) }
    pub fn diagnostics_json(&self) -> Result<String, RuntimeDriverError> {
        let (tx, rx) = mpsc::channel();
        self.sender.send(RuntimeCommand::Diagnostics(tx)).map_err(|_| RuntimeDriverError::Communication)?;
        rx.recv().map_err(|_| RuntimeDriverError::Communication)
    }
    pub fn wake_delivery(&self) { let _ = self.sender.send(RuntimeCommand::Wake); }
}

pub struct RuntimeHostOwner { sender: Sender<RuntimeCommand>, join: Option<JoinHandle<()>> }
impl RuntimeHostOwner {
    pub fn spawn<P: PairingDriver, C: CommunicationDriver, T: TorDriver>(engine: EngineHandle, mut pairing: P, mut communication: C, mut tor: T) -> (RuntimeHostHandle, Self) {
        let (sender, receiver) = mpsc::channel();
        let handle = RuntimeHostHandle { sender: sender.clone() };
        let join = thread::spawn(move || {
            let mut diagnostics = DiagnosticBuffer::new(256);
            let mut sequence = 1_u128;
            let startup = current_timestamp().unwrap_or(Timestamp::UNIX_EPOCH);
            let _ = communication.recover(startup);
            record(&mut diagnostics, &mut sequence, startup, Component::Engine, HealthState::Starting, "RUNTIME_STARTED");
            run_loop(receiver, &engine, &mut pairing, &mut communication, &mut tor, &mut diagnostics, &mut sequence);
            communication.shutdown(); pairing.shutdown(); tor.shutdown();
        });
        (handle, Self { sender, join: Some(join) })
    }
    pub fn shutdown(mut self) -> Result<(), RuntimeDriverError> {
        let (tx, rx) = mpsc::channel();
        self.sender.send(RuntimeCommand::Shutdown(tx)).map_err(|_| RuntimeDriverError::Communication)?;
        let _ = rx.recv();
        if let Some(join) = self.join.take() { join.join().map_err(|_| RuntimeDriverError::Communication)?; }
        Ok(())
    }
}

fn run_loop<P: PairingDriver, C: CommunicationDriver, T: TorDriver>(receiver: Receiver<RuntimeCommand>, engine: &EngineHandle, pairing: &mut P, communication: &mut C, tor: &mut T, diagnostics: &mut DiagnosticBuffer, sequence: &mut u128) {
    loop {
        match receiver.recv_timeout(RUNTIME_TICK) {
            Ok(RuntimeCommand::Shutdown(response)) => { let _ = response.send(()); break; }
            Ok(command) => { let now = current_timestamp().unwrap_or(Timestamp::UNIX_EPOCH); handle_command(command, engine, pairing, communication, tor, diagnostics, now); }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        let now = current_timestamp().unwrap_or(Timestamp::UNIX_EPOCH);
        let _ = tor.maintenance(now);
        let _ = pairing.maintenance(now);
        if let Ok(snapshot) = engine.snapshot() {
            let contacts: Vec<_> = snapshot.contacts.iter()
                .filter(|contact| contact.status() == ContactStatus::Active)
                .map(|contact| contact.id())
                .collect();
            let _ = communication.maintenance(&contacts, now);
            record(diagnostics, sequence, now, Component::Tor, map_health(tor.state()), "TOR_STATE");
            record(diagnostics, sequence, now, Component::Peer, HealthState::Ready, "PEER_TICK");
            record(diagnostics, sequence, now, Component::Storage, HealthState::Ready, "DELIVERY_TICK");
        }
    }
}

fn handle_command<P: PairingDriver, C: CommunicationDriver, T: TorDriver>(command: RuntimeCommand, engine: &EngineHandle, pairing: &mut P, communication: &mut C, tor: &T, diagnostics: &mut DiagnosticBuffer, now: Timestamp) {
    match command {
        RuntimeCommand::CreatePairing(id, r) => { let _ = r.send(pairing.create(id, now)); }
        RuntimeCommand::JoinPairing(id, code, r) => { let _ = r.send(pairing.join(id, code, now)); }
        RuntimeCommand::ApprovePairing(id, r) => { let _ = r.send(pairing.approve(id, now)); }
        RuntimeCommand::RejectPairing(id, r) => { let _ = r.send(pairing.reject(id)); }
        RuntimeCommand::CancelPairing(id, r) => { let _ = r.send(pairing.cancel(id)); }
        RuntimeCommand::RenameContact(id, name, r) => { let _ = r.send(communication.rename_contact(id, name, now)); }
        RuntimeCommand::BlockContact(id, r) => { let _ = r.send(communication.block_contact(id, now)); }
        RuntimeCommand::UnblockContact(id, r) => { let _ = r.send(communication.unblock_contact(id, now)); }
        RuntimeCommand::RemoveContact(id, r) => { let _ = r.send(communication.remove_contact(id)); }
        RuntimeCommand::ClearConversationHistory(id, r) => { let _ = r.send(communication.clear_conversation_history(id)); }
        RuntimeCommand::MarkConversationRead(id, r) => { let _ = r.send(communication.mark_conversation_read(id, now)); }
        RuntimeCommand::QueueAttachment(request_value, r) => {
            let message_id = MessageId::from_opaque(request_value.message_id);
            let body = MessageBody::new(format!("Attachment: {}", request_value.name)).map_err(|_| RuntimeDriverError::Communication);
            let result = body.and_then(|body| {
                engine.dispatch(EngineCommand::QueueMessage {
                    message_id,
                    conversation_id: ConversationId::from_opaque(request_value.conversation_id),
                    body,
                    reply_to: None,
                    at: now,
                }).map_err(|_| RuntimeDriverError::Engine)?;
                match communication.prepare_attachment(&request_value, now) {
                    Ok(()) => Ok(()),
                    Err(preparation_error) => {
                        let failure_code = ErrorCode::new("ATTACHMENT_PREPARE").map_err(|_| RuntimeDriverError::Engine)?;
                        engine.dispatch(EngineCommand::BeginMessageSend { message_id, at: now })
                            .map_err(|_| RuntimeDriverError::Engine)?;
                        engine.dispatch(EngineCommand::MarkMessageFailed { message_id, at: now, error_code: failure_code })
                            .map_err(|_| RuntimeDriverError::Engine)?;
                        Err(preparation_error)
                    }
                }
            });
            let _ = r.send(result);
        }
        RuntimeCommand::RetryAttachment(id, r) => { let _ = r.send(communication.retry_attachment(id, now)); }
        RuntimeCommand::CancelAttachment(id, r) => { let _ = r.send(communication.cancel_attachment(id, now)); }
        RuntimeCommand::AttachmentSnapshot(r) => { let _ = r.send(communication.attachment_snapshot()); }
        RuntimeCommand::NetworkSnapshot(r) => {
            let result = (|| {
                let snapshot = engine.snapshot().map_err(|_| RuntimeDriverError::Engine)?;
                let peers = snapshot.contacts.iter()
                    .map(|contact| (contact.id(), communication.connection_state(contact.id())))
                    .collect();
                let contact_names = communication.contact_names()?;
                Ok(NetworkSnapshot { tor: tor.state(), onion_address: tor.onion_address(), peers, contact_names })
            })();
            let _ = r.send(result);
        }
        RuntimeCommand::Diagnostics(r) => { let _ = r.send(diagnostics.export_json()); }
        RuntimeCommand::Wake => {}
        RuntimeCommand::Shutdown(_) => unreachable!(),
    }
}

fn request<T>(sender: &Sender<RuntimeCommand>, make: impl FnOnce(Sender<Result<T, RuntimeDriverError>>) -> RuntimeCommand) -> Result<T, RuntimeDriverError> {
    let (tx, rx) = mpsc::channel();
    sender.send(make(tx)).map_err(|_| RuntimeDriverError::Communication)?;
    rx.recv().map_err(|_| RuntimeDriverError::Communication)?
}
fn current_timestamp() -> Result<Timestamp, RuntimeDriverError> {
    let duration = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| RuntimeDriverError::Engine)?;
    let millis = i64::try_from(duration.as_millis()).map_err(|_| RuntimeDriverError::Engine)?;
    Timestamp::from_unix_millis(millis).map_err(|_| RuntimeDriverError::Engine)
}
fn record(buffer: &mut DiagnosticBuffer, sequence: &mut u128, at: Timestamp, component: Component, state: HealthState, code: &str) {
    let event_id = OpaqueId::from_u128(*sequence);
    *sequence = sequence.saturating_add(1);
    if let Ok(code) = DiagnosticCode::new(code) { buffer.record(DiagnosticEvent { event_id, at, component, state, code, detail: None }); }
}
const fn map_health(state: HostTorState) -> HealthState {
    match state {
        HostTorState::Stopped => HealthState::Stopped,
        HostTorState::Starting => HealthState::Starting,
        HostTorState::Ready => HealthState::Ready,
        HostTorState::Degraded => HealthState::Degraded,
        HostTorState::Failed => HealthState::Failed,
    }
}
