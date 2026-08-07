//! Single background owner for Tor, pairing, peer sessions and durable delivery.

use std::collections::BTreeMap;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use torca_client_engine::{EngineHandle, EngineError};
use torca_contacts::ContactId;
use torca_diagnostics::{
    Component, DiagnosticBuffer, DiagnosticCode, DiagnosticEvent, HealthState,
};
use torca_foundation::{OpaqueId, Timestamp};
use torca_pairing::{PairingCode, PairingSessionId};
use torca_peer_link::PeerConnectionState;

const RUNTIME_TICK: Duration = Duration::from_millis(100);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostTorState {
    Stopped,
    Starting,
    Ready,
    Degraded,
    Failed,
}

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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeDriverError {
    Pairing,
    Communication,
    Tor,
    Engine,
}
impl core::fmt::Display for RuntimeDriverError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for RuntimeDriverError {}

/// Concrete pairing adapter owns rendezvous/X25519 state; RuntimeHost only schedules it.
pub trait PairingDriver: Send + 'static {
    fn create(
        &mut self,
        session_id: PairingSessionId,
        now: Timestamp,
    ) -> Result<PairingInvitationView, RuntimeDriverError>;
    fn join(
        &mut self,
        session_id: PairingSessionId,
        code: PairingCode,
        now: Timestamp,
    ) -> Result<(), RuntimeDriverError>;
    fn approve(
        &mut self,
        session_id: PairingSessionId,
        now: Timestamp,
    ) -> Result<(), RuntimeDriverError>;
    fn reject(&mut self, session_id: PairingSessionId) -> Result<(), RuntimeDriverError>;
    fn cancel(&mut self, session_id: PairingSessionId) -> Result<(), RuntimeDriverError>;
    fn maintenance(&mut self, now: Timestamp) -> Result<(), RuntimeDriverError>;
    fn shutdown(&mut self);
}

/// Concrete communication adapter owns PeerLink, durable workers and inbound processing.
pub trait CommunicationDriver: Send + 'static {
    fn recover(&mut self, now: Timestamp) -> Result<(), RuntimeDriverError>;
    fn maintenance(
        &mut self,
        contacts: &[ContactId],
        now: Timestamp,
    ) -> Result<(), RuntimeDriverError>;
    fn connection_state(&self, contact_id: ContactId) -> PeerConnectionState;
    fn mark_conversation_read(
        &mut self,
        conversation_id: OpaqueId,
        now: Timestamp,
    ) -> Result<(), RuntimeDriverError>;
    fn shutdown(&mut self);
}

/// Platform composition owns how the Tor binary is located; this driver owns its process lifecycle.
pub trait TorDriver: Send + 'static {
    fn maintenance(&mut self, now: Timestamp) -> Result<(), RuntimeDriverError>;
    fn state(&self) -> HostTorState;
    fn onion_address(&self) -> Option<String>;
    fn shutdown(&mut self);
}

enum RuntimeCommand {
    CreatePairing(
        PairingSessionId,
        Sender<Result<PairingInvitationView, RuntimeDriverError>>,
    ),
    JoinPairing(
        PairingSessionId,
        PairingCode,
        Sender<Result<(), RuntimeDriverError>>,
    ),
    ApprovePairing(
        PairingSessionId,
        Sender<Result<(), RuntimeDriverError>>,
    ),
    RejectPairing(
        PairingSessionId,
        Sender<Result<(), RuntimeDriverError>>,
    ),
    CancelPairing(
        PairingSessionId,
        Sender<Result<(), RuntimeDriverError>>,
    ),
    MarkConversationRead(
        OpaqueId,
        Sender<Result<(), RuntimeDriverError>>,
    ),
    NetworkSnapshot(Sender<Result<NetworkSnapshot, RuntimeDriverError>>),
    Diagnostics(Sender<String>),
    Wake,
    Shutdown(Sender<()>),
}

#[derive(Clone)]
pub struct RuntimeHostHandle {
    sender: Sender<RuntimeCommand>,
}
impl RuntimeHostHandle {
    pub fn create_pairing(
        &self,
        session_id: PairingSessionId,
    ) -> Result<PairingInvitationView, RuntimeDriverError> {
        request(&self.sender, |response| RuntimeCommand::CreatePairing(session_id, response))
    }

    pub fn join_pairing(
        &self,
        session_id: PairingSessionId,
        code: PairingCode,
    ) -> Result<(), RuntimeDriverError> {
        request(&self.sender, |response| RuntimeCommand::JoinPairing(session_id, code, response))
    }

    pub fn approve_pairing(
        &self,
        session_id: PairingSessionId,
    ) -> Result<(), RuntimeDriverError> {
        request(&self.sender, |response| RuntimeCommand::ApprovePairing(session_id, response))
    }

    pub fn reject_pairing(
        &self,
        session_id: PairingSessionId,
    ) -> Result<(), RuntimeDriverError> {
        request(&self.sender, |response| RuntimeCommand::RejectPairing(session_id, response))
    }

    pub fn cancel_pairing(
        &self,
        session_id: PairingSessionId,
    ) -> Result<(), RuntimeDriverError> {
        request(&self.sender, |response| RuntimeCommand::CancelPairing(session_id, response))
    }

    pub fn mark_conversation_read(&self, conversation_id: OpaqueId) -> Result<(), RuntimeDriverError> {
        request(&self.sender, |response| {
            RuntimeCommand::MarkConversationRead(conversation_id, response)
        })
    }

    pub fn network_snapshot(&self) -> Result<NetworkSnapshot, RuntimeDriverError> {
        request(&self.sender, RuntimeCommand::NetworkSnapshot)
    }

    pub fn diagnostics_json(&self) -> Result<String, RuntimeDriverError> {
        let (sender, receiver) = mpsc::channel();
        self.sender
            .send(RuntimeCommand::Diagnostics(sender))
            .map_err(|_| RuntimeDriverError::Communication)?;
        receiver.recv().map_err(|_| RuntimeDriverError::Communication)
    }

    pub fn wake_delivery(&self) {
        let _ = self.sender.send(RuntimeCommand::Wake);
    }
}

pub struct RuntimeHostOwner {
    sender: Sender<RuntimeCommand>,
    join: Option<JoinHandle<()>>,
}
impl RuntimeHostOwner {
    pub fn spawn<P, C, T>(
        engine: EngineHandle,
        mut pairing: P,
        mut communication: C,
        mut tor: T,
    ) -> (RuntimeHostHandle, Self)
    where
        P: PairingDriver,
        C: CommunicationDriver,
        T: TorDriver,
    {
        let (sender, receiver) = mpsc::channel();
        let handle = RuntimeHostHandle { sender: sender.clone() };
        let join = thread::spawn(move || {
            let mut diagnostics = DiagnosticBuffer::new(256);
            let mut diagnostic_sequence = 1_u128;
            let startup = current_timestamp().unwrap_or(Timestamp::UNIX_EPOCH);
            let _ = communication.recover(startup);
            record(
                &mut diagnostics,
                &mut diagnostic_sequence,
                startup,
                Component::Engine,
                HealthState::Starting,
                "RUNTIME_STARTED",
            );
            run_loop(
                receiver,
                &engine,
                &mut pairing,
                &mut communication,
                &mut tor,
                &mut diagnostics,
                &mut diagnostic_sequence,
            );
            communication.shutdown();
            pairing.shutdown();
            tor.shutdown();
        });
        (handle, Self { sender, join: Some(join) })
    }

    pub fn shutdown(mut self) -> Result<(), RuntimeDriverError> {
        let (sender, receiver) = mpsc::channel();
        self.sender
            .send(RuntimeCommand::Shutdown(sender))
            .map_err(|_| RuntimeDriverError::Communication)?;
        let _ = receiver.recv();
        if let Some(join) = self.join.take() {
            join.join().map_err(|_| RuntimeDriverError::Communication)?;
        }
        Ok(())
    }
}

fn run_loop<P, C, T>(
    receiver: Receiver<RuntimeCommand>,
    engine: &EngineHandle,
    pairing: &mut P,
    communication: &mut C,
    tor: &mut T,
    diagnostics: &mut DiagnosticBuffer,
    diagnostic_sequence: &mut u128,
) where
    P: PairingDriver,
    C: CommunicationDriver,
    T: TorDriver,
{
    loop {
        match receiver.recv_timeout(RUNTIME_TICK) {
            Ok(RuntimeCommand::Shutdown(response)) => {
                let _ = response.send(());
                break;
            }
            Ok(command) => {
                let now = current_timestamp().unwrap_or(Timestamp::UNIX_EPOCH);
                handle_command(command, pairing, communication, tor, diagnostics, now);
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        let now = current_timestamp().unwrap_or(Timestamp::UNIX_EPOCH);
        let _ = tor.maintenance(now);
        let _ = pairing.maintenance(now);
        if let Ok(snapshot) = engine.snapshot() {
            let contacts: Vec<_> = snapshot.contacts.iter().map(|contact| contact.id()).collect();
            let _ = communication.maintenance(&contacts, now);
            record(
                diagnostics,
                diagnostic_sequence,
                now,
                Component::Tor,
                map_health(tor.state()),
                "TOR_STATE",
            );
        }
    }
}

fn handle_command<P, C, T>(
    command: RuntimeCommand,
    pairing: &mut P,
    communication: &mut C,
    tor: &T,
    diagnostics: &mut DiagnosticBuffer,
    now: Timestamp,
) where
    P: PairingDriver,
    C: CommunicationDriver,
    T: TorDriver,
{
    match command {
        RuntimeCommand::CreatePairing(id, response) => {
            let _ = response.send(pairing.create(id, now));
        }
        RuntimeCommand::JoinPairing(id, code, response) => {
            let _ = response.send(pairing.join(id, code, now));
        }
        RuntimeCommand::ApprovePairing(id, response) => {
            let _ = response.send(pairing.approve(id, now));
        }
        RuntimeCommand::RejectPairing(id, response) => {
            let _ = response.send(pairing.reject(id));
        }
        RuntimeCommand::CancelPairing(id, response) => {
            let _ = response.send(pairing.cancel(id));
        }
        RuntimeCommand::MarkConversationRead(id, response) => {
            let _ = response.send(communication.mark_conversation_read(id, now));
        }
        RuntimeCommand::NetworkSnapshot(response) => {
            let peers = BTreeMap::new();
            let _ = response.send(Ok(NetworkSnapshot {
                tor: tor.state(),
                onion_address: tor.onion_address(),
                peers,
            }));
        }
        RuntimeCommand::Diagnostics(response) => {
            let _ = response.send(diagnostics.export_json());
        }
        RuntimeCommand::Wake => {}
        RuntimeCommand::Shutdown(_) => unreachable!(),
    }
}

fn request<T>(
    sender: &Sender<RuntimeCommand>,
    make: impl FnOnce(Sender<Result<T, RuntimeDriverError>>) -> RuntimeCommand,
) -> Result<T, RuntimeDriverError> {
    let (response_sender, response_receiver) = mpsc::channel();
    sender
        .send(make(response_sender))
        .map_err(|_| RuntimeDriverError::Communication)?;
    response_receiver
        .recv()
        .map_err(|_| RuntimeDriverError::Communication)?
}

fn current_timestamp() -> Result<Timestamp, RuntimeDriverError> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| RuntimeDriverError::Engine)?;
    let millis = i64::try_from(duration.as_millis()).map_err(|_| RuntimeDriverError::Engine)?;
    Timestamp::from_unix_millis(millis).map_err(|_| RuntimeDriverError::Engine)
}

fn record(
    buffer: &mut DiagnosticBuffer,
    sequence: &mut u128,
    at: Timestamp,
    component: Component,
    state: HealthState,
    code: &str,
) {
    let event_id = OpaqueId::from_u128(*sequence);
    *sequence = sequence.saturating_add(1);
    if let Ok(code) = DiagnosticCode::new(code) {
        buffer.record(DiagnosticEvent { event_id, at, component, state, code, detail: None });
    }
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

fn _map_engine(_: EngineError) -> RuntimeDriverError {
    RuntimeDriverError::Engine
}
