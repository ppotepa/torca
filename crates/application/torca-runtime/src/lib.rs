//! Single background owner for Tor, pairing, peer sessions and durable delivery.

mod attachments;
mod tor_driver;
pub use attachments::{AttachmentSendRequest, AttachmentView};
pub use tor_driver::{OwnedTorDriver, SharedTorEndpoint};

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, SyncSender, TrySendError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use torca_attachments::AttachmentId;
use torca_client_engine::{EngineCommand, EngineHandle};
use torca_contacts::{ContactId, ContactStatus};
use torca_conversations::ConversationId;
use torca_diagnostics::{
    Component, DiagnosticBuffer, DiagnosticCode, DiagnosticEvent, HealthState,
};
use torca_foundation::{ErrorCode, OpaqueId, Timestamp};
use torca_messaging::{MessageBody, MessageId};
use torca_pairing::{PairingCode, PairingSessionId};
use torca_peer_link::PeerConnectionState;
use torca_probing::{ProbeKind, ProbeResult, ProbeStatus, ProbeSupervisor, ProbeTarget};

const RUNTIME_TICK: Duration = Duration::from_millis(100);
const COMMAND_WAIT: Duration = Duration::from_secs(10);
const QUERY_WAIT: Duration = Duration::from_secs(5);
const SHUTDOWN_WAIT: Duration = Duration::from_secs(15);
const MAILBOX_CAPACITY: usize = 256;
const ENQUEUE_WAIT: Duration = Duration::from_secs(2);
const RELAY_PROBE_TIMEOUT: Duration = Duration::from_secs(15);
const RELAY_RETRY_BACKOFF: [Duration; 4] = [
    Duration::from_secs(5),
    Duration::from_secs(15),
    Duration::from_secs(30),
    Duration::from_secs(60),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TorState {
    Stopped,
    Starting,
    Ready,
    Degraded,
    Failed,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PeerHealthQuality {
    Unknown,
    Excellent,
    Good,
    Fair,
    Poor,
}
#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PeerHealthSnapshot {
    pub state: PeerConnectionState,
    pub quality: PeerHealthQuality,
    pub rtt_ms: Option<u64>,
    pub last_success_at: Option<Timestamp>,
    pub consecutive_failures: u32,
    pub reconnect_attempt: u32,
}

/// Redacted transport activity used only to animate presentation indicators.
/// It intentionally carries no payload, address, key or message content.
#[must_use]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TransportActivitySnapshot {
    pub last_activity_at: Option<Timestamp>,
    pub sequence: u64,
}

#[derive(Default)]
struct TransportActivityLedger {
    tor: TransportActivitySnapshot,
    relay: TransportActivitySnapshot,
    peers: BTreeMap<ContactId, TransportActivitySnapshot>,
}

impl TransportActivityLedger {
    fn mark_tor(&mut self, now: Timestamp) {
        Self::mark(&mut self.tor, now);
    }

    fn mark_relay(&mut self, now: Timestamp) {
        Self::mark(&mut self.relay, now);
    }

    fn mark_peer(&mut self, contact_id: ContactId, now: Timestamp) {
        Self::mark(self.peers.entry(contact_id).or_default(), now);
    }

    fn mark(activity: &mut TransportActivitySnapshot, now: Timestamp) {
        activity.last_activity_at = Some(now);
        activity.sequence = activity.sequence.saturating_add(1);
    }
}
impl PeerHealthSnapshot {
    pub const fn from_connection_state(state: PeerConnectionState) -> Self {
        Self {
            state,
            quality: PeerHealthQuality::Unknown,
            rtt_ms: None,
            last_success_at: None,
            consecutive_failures: 0,
            reconnect_attempt: 0,
        }
    }
}

#[must_use]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ContactVerificationSnapshot {
    pub verified: bool,
    pub verified_at: Option<Timestamp>,
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
    pub tor: TorState,
    pub onion_address: Option<String>,
    pub peers: BTreeMap<ContactId, PeerConnectionState>,
    pub peer_health: BTreeMap<ContactId, PeerHealthSnapshot>,
    pub contact_names: BTreeMap<ContactId, String>,
    pub contact_verifications: BTreeMap<ContactId, ContactVerificationSnapshot>,
    pub tor_activity: TransportActivitySnapshot,
    pub relay_activity: TransportActivitySnapshot,
    pub peer_activity: BTreeMap<ContactId, TransportActivitySnapshot>,
    pub probes: Vec<ProbeResult>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeDriverError {
    Pairing,
    Communication,
    Tor,
    Engine,
    Pending,
}
impl core::fmt::Display for RuntimeDriverError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for RuntimeDriverError {}

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
pub trait CommunicationDriver: Send + 'static {
    fn recover(&mut self, now: Timestamp) -> Result<(), RuntimeDriverError>;
    fn maintenance(
        &mut self,
        contacts: &[ContactId],
        now: Timestamp,
    ) -> Result<(), RuntimeDriverError>;
    fn connection_state(&self, contact_id: ContactId) -> PeerConnectionState;
    fn peer_health(&self, contact_id: ContactId) -> PeerHealthSnapshot {
        PeerHealthSnapshot::from_connection_state(self.connection_state(contact_id))
    }
    fn contact_names(&self) -> Result<BTreeMap<ContactId, String>, RuntimeDriverError>;
    fn contact_verifications(
        &self,
    ) -> Result<BTreeMap<ContactId, ContactVerificationSnapshot>, RuntimeDriverError>;
    fn verify_contact(
        &mut self,
        contact_id: ContactId,
        now: Timestamp,
    ) -> Result<(), RuntimeDriverError>;
    fn reset_contact_verification(
        &mut self,
        contact_id: ContactId,
    ) -> Result<(), RuntimeDriverError>;
    fn rename_contact(
        &mut self,
        contact_id: ContactId,
        display_name: String,
        now: Timestamp,
    ) -> Result<(), RuntimeDriverError>;
    fn block_contact(
        &mut self,
        contact_id: ContactId,
        now: Timestamp,
    ) -> Result<(), RuntimeDriverError>;
    fn unblock_contact(
        &mut self,
        contact_id: ContactId,
        now: Timestamp,
    ) -> Result<(), RuntimeDriverError>;
    fn clear_conversation_history(
        &mut self,
        conversation_id: ConversationId,
    ) -> Result<(), RuntimeDriverError>;
    fn remove_contact(&mut self, contact_id: ContactId) -> Result<(), RuntimeDriverError>;
    fn mark_conversation_read(
        &mut self,
        conversation_id: OpaqueId,
        now: Timestamp,
    ) -> Result<(), RuntimeDriverError>;
    fn mark_conversation_read_with_policy(
        &mut self,
        conversation_id: OpaqueId,
        now: Timestamp,
        _send_receipt: bool,
    ) -> Result<(), RuntimeDriverError> {
        self.mark_conversation_read(conversation_id, now)
    }
    fn prepare_attachment(
        &mut self,
        request: &AttachmentSendRequest,
        now: Timestamp,
    ) -> Result<(), RuntimeDriverError>;
    fn retry_attachment(
        &mut self,
        attachment_id: OpaqueId,
        now: Timestamp,
    ) -> Result<(), RuntimeDriverError>;
    fn cancel_attachment(
        &mut self,
        attachment_id: OpaqueId,
        now: Timestamp,
    ) -> Result<(), RuntimeDriverError>;
    fn export_attachment(
        &mut self,
        attachment_id: AttachmentId,
        destination: PathBuf,
    ) -> Result<(), RuntimeDriverError>;
    fn attachment_snapshot(&self) -> Result<Vec<AttachmentView>, RuntimeDriverError>;
    fn shutdown(&mut self);
}
pub trait TorDriver: Send + 'static {
    fn maintenance(&mut self, now: Timestamp) -> Result<(), RuntimeDriverError>;
    fn state(&self) -> TorState;
    fn onion_address(&self) -> Option<String>;
    fn shutdown(&mut self);
}

/// Relay connectivity is supervised outside the actor's critical path. A
/// probe implementation must be cheap to clone through `Arc` and may perform
/// blocking network work on the worker thread created by the supervisor.
pub trait RelayProbe: Send + Sync + 'static {
    fn probe(&self) -> Result<(), RuntimeDriverError>;
}

struct RelayProbeState {
    probe: Option<Arc<dyn RelayProbe>>,
    result_rx: Option<Receiver<Result<(), RuntimeDriverError>>>,
    started_at: Option<std::time::Instant>,
    next_retry_at: std::time::Instant,
    failures: u32,
    status: ProbeStatus,
    latency_ms: Option<u64>,
    code: &'static str,
}

impl RelayProbeState {
    fn new(probe: Option<Arc<dyn RelayProbe>>) -> Self {
        let configured = probe.is_some();
        Self {
            probe,
            result_rx: None,
            started_at: None,
            next_retry_at: std::time::Instant::now(),
            failures: 0,
            status: if configured { ProbeStatus::Checking } else { ProbeStatus::Unknown },
            latency_ms: None,
            code: if configured { "RELAY_PROBE_PENDING" } else { "RELAY_PROBE_UNCONFIGURED" },
        }
    }

    fn tick(&mut self) {
        let Some(probe) = self.probe.clone() else { return };
        if let Some(receiver) = self.result_rx.as_ref() {
            match receiver.try_recv() {
                Ok(result) => {
                    let started = self.started_at.take().unwrap_or_else(std::time::Instant::now);
                    self.result_rx = None;
                    self.status = match result {
                        Ok(()) => {
                            self.failures = 0;
                            self.next_retry_at = std::time::Instant::now() + RELAY_RETRY_BACKOFF[3];
                            self.latency_ms = Some(
                                u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
                            );
                            self.code = "RELAY_READY";
                            ProbeStatus::Healthy
                        }
                        Err(_) => self.failed_retry(),
                    };
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.result_rx = None;
                    self.started_at = None;
                    self.status = self.failed_retry();
                }
                Err(mpsc::TryRecvError::Empty) => {
                    if self
                        .started_at
                        .is_some_and(|started| started.elapsed() >= RELAY_PROBE_TIMEOUT)
                    {
                        self.result_rx = None;
                        self.started_at = None;
                        self.status = self.failed_retry();
                    }
                }
            }
        }
        if self.result_rx.is_none() && std::time::Instant::now() >= self.next_retry_at {
            let (sender, receiver) = mpsc::channel();
            self.result_rx = Some(receiver);
            self.started_at = Some(std::time::Instant::now());
            self.status = ProbeStatus::Checking;
            self.code = "RELAY_PROBE_RUNNING";
            thread::spawn(move || {
                let _ = sender.send(probe.probe());
            });
        }
    }

    fn failed_retry(&mut self) -> ProbeStatus {
        self.failures = self.failures.saturating_add(1);
        let index = usize::try_from(self.failures.saturating_sub(1))
            .unwrap_or(usize::MAX)
            .min(RELAY_RETRY_BACKOFF.len() - 1);
        self.next_retry_at = std::time::Instant::now() + RELAY_RETRY_BACKOFF[index];
        self.latency_ms = None;
        self.code = "RELAY_UNREACHABLE";
        ProbeStatus::Degraded
    }

    fn result(&self, now: Timestamp) -> ProbeResult {
        ProbeResult {
            target: ProbeTarget::Relay,
            kind: ProbeKind::Connectivity,
            status: self.status,
            diagnostic_code: self.code.into(),
            latency_ms: self.latency_ms,
            measured_at: now,
        }
    }
}

enum RuntimeCommand {
    CreatePairing(PairingSessionId, Sender<Result<PairingInvitationView, RuntimeDriverError>>),
    JoinPairing(PairingSessionId, PairingCode, Sender<Result<(), RuntimeDriverError>>),
    ApprovePairing(PairingSessionId, Sender<Result<(), RuntimeDriverError>>),
    RejectPairing(PairingSessionId, Sender<Result<(), RuntimeDriverError>>),
    CancelPairing(PairingSessionId, Sender<Result<(), RuntimeDriverError>>),
    VerifyContact(ContactId, Sender<Result<(), RuntimeDriverError>>),
    ResetContactVerification(ContactId, Sender<Result<(), RuntimeDriverError>>),
    RenameContact(ContactId, String, Sender<Result<(), RuntimeDriverError>>),
    BlockContact(ContactId, Sender<Result<(), RuntimeDriverError>>),
    UnblockContact(ContactId, Sender<Result<(), RuntimeDriverError>>),
    RemoveContact(ContactId, Sender<Result<(), RuntimeDriverError>>),
    ClearConversationHistory(ConversationId, Sender<Result<(), RuntimeDriverError>>),
    MarkConversationRead(OpaqueId, bool, Sender<Result<(), RuntimeDriverError>>),
    QueueAttachment(AttachmentSendRequest, Sender<Result<(), RuntimeDriverError>>),
    RetryAttachment(OpaqueId, Sender<Result<(), RuntimeDriverError>>),
    CancelAttachment(OpaqueId, Sender<Result<(), RuntimeDriverError>>),
    ExportAttachment(AttachmentId, PathBuf, Sender<Result<(), RuntimeDriverError>>),
    AttachmentSnapshot(Sender<Result<Vec<AttachmentView>, RuntimeDriverError>>),
    NetworkSnapshot(Sender<Result<NetworkSnapshot, RuntimeDriverError>>),
    Diagnostics(Sender<String>),
    Wake,
    Shutdown(Sender<()>),
}

#[derive(Clone)]
pub struct RuntimeHandle {
    sender: SyncSender<RuntimeCommand>,
}
impl RuntimeHandle {
    pub fn create_pairing(
        &self,
        id: PairingSessionId,
    ) -> Result<PairingInvitationView, RuntimeDriverError> {
        request_command(&self.sender, |r| RuntimeCommand::CreatePairing(id, r))
    }
    pub fn join_pairing(
        &self,
        id: PairingSessionId,
        code: PairingCode,
    ) -> Result<(), RuntimeDriverError> {
        request_command(&self.sender, |r| RuntimeCommand::JoinPairing(id, code, r))
    }
    pub fn approve_pairing(&self, id: PairingSessionId) -> Result<(), RuntimeDriverError> {
        request_command(&self.sender, |r| RuntimeCommand::ApprovePairing(id, r))
    }
    pub fn reject_pairing(&self, id: PairingSessionId) -> Result<(), RuntimeDriverError> {
        request_command(&self.sender, |r| RuntimeCommand::RejectPairing(id, r))
    }
    pub fn cancel_pairing(&self, id: PairingSessionId) -> Result<(), RuntimeDriverError> {
        request_command(&self.sender, |r| RuntimeCommand::CancelPairing(id, r))
    }
    pub fn verify_contact(&self, id: ContactId) -> Result<(), RuntimeDriverError> {
        request_command(&self.sender, |r| RuntimeCommand::VerifyContact(id, r))
    }
    pub fn reset_contact_verification(&self, id: ContactId) -> Result<(), RuntimeDriverError> {
        request_command(&self.sender, |r| RuntimeCommand::ResetContactVerification(id, r))
    }
    pub fn rename_contact(&self, id: ContactId, name: String) -> Result<(), RuntimeDriverError> {
        request_command(&self.sender, |r| RuntimeCommand::RenameContact(id, name, r))
    }
    pub fn block_contact(&self, id: ContactId) -> Result<(), RuntimeDriverError> {
        request_command(&self.sender, |r| RuntimeCommand::BlockContact(id, r))
    }
    pub fn unblock_contact(&self, id: ContactId) -> Result<(), RuntimeDriverError> {
        request_command(&self.sender, |r| RuntimeCommand::UnblockContact(id, r))
    }
    pub fn remove_contact(&self, id: ContactId) -> Result<(), RuntimeDriverError> {
        request_command(&self.sender, |r| RuntimeCommand::RemoveContact(id, r))
    }
    pub fn clear_conversation_history(&self, id: ConversationId) -> Result<(), RuntimeDriverError> {
        request_command(&self.sender, |r| RuntimeCommand::ClearConversationHistory(id, r))
    }
    pub fn mark_conversation_read(&self, id: OpaqueId) -> Result<(), RuntimeDriverError> {
        self.mark_conversation_read_with_policy(id, true)
    }
    pub fn mark_conversation_read_with_policy(
        &self,
        id: OpaqueId,
        send_receipt: bool,
    ) -> Result<(), RuntimeDriverError> {
        request_command(&self.sender, |r| RuntimeCommand::MarkConversationRead(id, send_receipt, r))
    }
    pub fn queue_attachment(&self, value: AttachmentSendRequest) -> Result<(), RuntimeDriverError> {
        request_command(&self.sender, |r| RuntimeCommand::QueueAttachment(value, r))
    }
    pub fn retry_attachment(&self, id: OpaqueId) -> Result<(), RuntimeDriverError> {
        request_command(&self.sender, |r| RuntimeCommand::RetryAttachment(id, r))
    }
    pub fn cancel_attachment(&self, id: OpaqueId) -> Result<(), RuntimeDriverError> {
        request_command(&self.sender, |r| RuntimeCommand::CancelAttachment(id, r))
    }
    pub fn export_attachment(
        &self,
        id: AttachmentId,
        destination: PathBuf,
    ) -> Result<(), RuntimeDriverError> {
        request_blocking(&self.sender, |r| RuntimeCommand::ExportAttachment(id, destination, r))
    }
    pub fn attachment_snapshot(&self) -> Result<Vec<AttachmentView>, RuntimeDriverError> {
        request_query(&self.sender, RuntimeCommand::AttachmentSnapshot)
    }
    pub fn network_snapshot(&self) -> Result<NetworkSnapshot, RuntimeDriverError> {
        request_query(&self.sender, RuntimeCommand::NetworkSnapshot)
    }
    pub fn diagnostics_json(&self) -> Result<String, RuntimeDriverError> {
        let (tx, rx) = mpsc::channel();
        send_with_timeout(&self.sender, RuntimeCommand::Diagnostics(tx))?;
        rx.recv_timeout(QUERY_WAIT).map_err(|_| RuntimeDriverError::Communication)
    }
    pub fn wake_delivery(&self) {
        let _ = send_with_timeout(&self.sender, RuntimeCommand::Wake);
    }
}

pub struct RuntimeOwner {
    sender: SyncSender<RuntimeCommand>,
    join: Option<JoinHandle<()>>,
}
impl RuntimeOwner {
    pub fn spawn<P: PairingDriver, C: CommunicationDriver, T: TorDriver>(
        engine: EngineHandle,
        pairing: P,
        communication: C,
        tor: T,
    ) -> (RuntimeHandle, Self) {
        Self::spawn_with_relay_probe(engine, pairing, communication, tor, None)
    }

    pub fn spawn_with_relay_probe<P: PairingDriver, C: CommunicationDriver, T: TorDriver>(
        engine: EngineHandle,
        mut pairing: P,
        mut communication: C,
        mut tor: T,
        relay_probe: Option<Arc<dyn RelayProbe>>,
    ) -> (RuntimeHandle, Self) {
        let (sender, receiver) = mpsc::sync_channel(MAILBOX_CAPACITY);
        let handle = RuntimeHandle { sender: sender.clone() };
        let join = thread::spawn(move || {
            let mut diagnostics = DiagnosticBuffer::new(256);
            let mut sequence = 1_u128;
            let startup = current_timestamp().unwrap_or(Timestamp::UNIX_EPOCH);
            match communication.recover(startup) {
                Ok(()) => record(
                    &mut diagnostics,
                    &mut sequence,
                    startup,
                    Component::Storage,
                    HealthState::Ready,
                    "DELIVERY_RECOVERY_READY",
                ),
                Err(_) => record(
                    &mut diagnostics,
                    &mut sequence,
                    startup,
                    Component::Storage,
                    HealthState::Failed,
                    "DELIVERY_RECOVERY_FAILED",
                ),
            }
            record(
                &mut diagnostics,
                &mut sequence,
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
                &mut sequence,
                relay_probe,
            );
            communication.shutdown();
            pairing.shutdown();
            tor.shutdown();
        });
        (handle, Self { sender, join: Some(join) })
    }
    pub fn shutdown(mut self) -> Result<(), RuntimeDriverError> {
        let (tx, rx) = mpsc::channel();
        send_with_timeout(&self.sender, RuntimeCommand::Shutdown(tx))?;
        rx.recv_timeout(SHUTDOWN_WAIT).map_err(|_| RuntimeDriverError::Communication)?;
        if let Some(join) = self.join.take() {
            join.join().map_err(|_| RuntimeDriverError::Communication)?;
        }
        Ok(())
    }
}

fn run_loop<P: PairingDriver, C: CommunicationDriver, T: TorDriver>(
    receiver: Receiver<RuntimeCommand>,
    engine: &EngineHandle,
    pairing: &mut P,
    communication: &mut C,
    tor: &mut T,
    diagnostics: &mut DiagnosticBuffer,
    sequence: &mut u128,
    relay_probe: Option<Arc<dyn RelayProbe>>,
) {
    let mut last_tor_state = None;
    let mut last_peer_states = BTreeMap::<ContactId, PeerConnectionState>::new();
    let mut tor_failed = false;
    let mut pairing_failed = false;
    let mut communication_failed = false;
    let mut probes = ProbeSupervisor::default();
    let mut relay = RelayProbeState::new(relay_probe);
    let mut transport_activity = TransportActivityLedger::default();
    loop {
        match receiver.recv_timeout(RUNTIME_TICK) {
            Ok(RuntimeCommand::Shutdown(response)) => {
                let _ = response.send(());
                break;
            }
            Ok(command) => {
                let now = current_timestamp().unwrap_or(Timestamp::UNIX_EPOCH);
                handle_command(
                    command,
                    engine,
                    pairing,
                    communication,
                    tor,
                    &probes,
                    &mut transport_activity,
                    diagnostics,
                    now,
                );
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        let now = current_timestamp().unwrap_or(Timestamp::UNIX_EPOCH);
        let relay_before = (relay.status, relay.code, relay.latency_ms, relay.failures);
        relay.tick();
        if relay_before != (relay.status, relay.code, relay.latency_ms, relay.failures) {
            // A relay probe always traverses embedded Tor, so both indicators
            // receive the same redacted transport observation.
            transport_activity.mark_tor(now);
            transport_activity.mark_relay(now);
        }
        observe_maintenance(
            tor.maintenance(now),
            &mut tor_failed,
            diagnostics,
            sequence,
            now,
            Component::Tor,
            "TOR_MAINTENANCE_FAILED",
            "TOR_MAINTENANCE_RECOVERED",
        );
        observe_maintenance(
            pairing.maintenance(now),
            &mut pairing_failed,
            diagnostics,
            sequence,
            now,
            Component::Relay,
            "PAIRING_MAINTENANCE_FAILED",
            "PAIRING_MAINTENANCE_RECOVERED",
        );
        let tor_state = tor.state();
        record_runtime_probes(
            &mut probes,
            tor_state,
            tor.onion_address().is_some(),
            communication_failed,
            relay.result(now),
            now,
        );
        if last_tor_state != Some(tor_state) {
            last_tor_state = Some(tor_state);
            transport_activity.mark_tor(now);
            record(
                diagnostics,
                sequence,
                now,
                Component::Tor,
                map_health(tor_state),
                "TOR_STATE_CHANGED",
            );
        }
        if let Ok(snapshot) = engine.snapshot() {
            let contacts: Vec<_> = snapshot
                .contacts
                .iter()
                .filter(|c| c.status() == ContactStatus::Active)
                .map(torca_contacts::Contact::id)
                .collect();
            observe_maintenance(
                communication.maintenance(&contacts, now),
                &mut communication_failed,
                diagnostics,
                sequence,
                now,
                Component::Peer,
                "COMMUNICATION_MAINTENANCE_FAILED",
                "COMMUNICATION_MAINTENANCE_RECOVERED",
            );
            let mut current = BTreeMap::new();
            for id in contacts {
                let state = communication.connection_state(id);
                if last_peer_states.get(&id) != Some(&state) {
                    transport_activity.mark_tor(now);
                    transport_activity.mark_peer(id, now);
                    record(
                        diagnostics,
                        sequence,
                        now,
                        Component::Peer,
                        map_peer_health(state),
                        "PEER_STATE_CHANGED",
                    );
                }
                current.insert(id, state);
            }
            last_peer_states = current;
        }
    }
}

fn observe_maintenance(
    result: Result<(), RuntimeDriverError>,
    failed: &mut bool,
    diagnostics: &mut DiagnosticBuffer,
    sequence: &mut u128,
    now: Timestamp,
    component: Component,
    failure_code: &str,
    recovery_code: &str,
) {
    match (result, *failed) {
        (Err(_), false) => {
            *failed = true;
            record(diagnostics, sequence, now, component, HealthState::Failed, failure_code);
        }
        (Ok(()), true) => {
            *failed = false;
            record(diagnostics, sequence, now, component, HealthState::Ready, recovery_code);
        }
        _ => {}
    }
}

fn record_runtime_probes(
    probes: &mut ProbeSupervisor,
    tor_state: TorState,
    onion_ready: bool,
    peer_failed: bool,
    relay_result: ProbeResult,
    now: Timestamp,
) {
    for target in [
        ProbeTarget::NativeBridge,
        ProbeTarget::SecureStorage,
        ProbeTarget::Database,
        ProbeTarget::Engine,
    ] {
        probes.record(runtime_probe(target, ProbeKind::Readiness, ProbeStatus::Healthy, "OK", now));
    }
    probes.record(runtime_probe(
        ProbeTarget::Tor,
        ProbeKind::Readiness,
        match tor_state {
            TorState::Ready => ProbeStatus::Healthy,
            TorState::Starting => ProbeStatus::Checking,
            TorState::Degraded => ProbeStatus::Degraded,
            TorState::Failed => ProbeStatus::Failed,
            TorState::Stopped => ProbeStatus::Unknown,
        },
        if matches!(tor_state, TorState::Ready) { "TOR_READY" } else { "TOR_NOT_READY" },
        now,
    ));
    probes.record(relay_result);
    probes.record(runtime_probe(
        ProbeTarget::OnionService,
        ProbeKind::Readiness,
        if onion_ready { ProbeStatus::Healthy } else { ProbeStatus::Checking },
        if onion_ready { "ONION_READY" } else { "ONION_PENDING" },
        now,
    ));
    probes.record(runtime_probe(
        ProbeTarget::Peer,
        ProbeKind::Connectivity,
        if peer_failed { ProbeStatus::Degraded } else { ProbeStatus::Healthy },
        if peer_failed { "PEER_MAINTENANCE_FAILED" } else { "PEER_MAINTENANCE_READY" },
        now,
    ));
}

fn runtime_probe(
    target: ProbeTarget,
    kind: ProbeKind,
    status: ProbeStatus,
    diagnostic_code: &str,
    measured_at: Timestamp,
) -> ProbeResult {
    ProbeResult {
        target,
        kind,
        status,
        diagnostic_code: diagnostic_code.into(),
        latency_ms: None,
        measured_at,
    }
}

fn handle_command<P: PairingDriver, C: CommunicationDriver, T: TorDriver>(
    command: RuntimeCommand,
    engine: &EngineHandle,
    pairing: &mut P,
    communication: &mut C,
    tor: &T,
    probes: &ProbeSupervisor,
    transport_activity: &mut TransportActivityLedger,
    diagnostics: &mut DiagnosticBuffer,
    now: Timestamp,
) {
    match command {
        RuntimeCommand::CreatePairing(id, r) => {
            transport_activity.mark_tor(now);
            transport_activity.mark_relay(now);
            let _ = r.send(pairing.create(id, now));
        }
        RuntimeCommand::JoinPairing(id, code, r) => {
            transport_activity.mark_tor(now);
            transport_activity.mark_relay(now);
            let _ = r.send(pairing.join(id, code, now));
        }
        RuntimeCommand::ApprovePairing(id, r) => {
            transport_activity.mark_tor(now);
            transport_activity.mark_relay(now);
            let _ = r.send(pairing.approve(id, now));
        }
        RuntimeCommand::RejectPairing(id, r) => {
            transport_activity.mark_tor(now);
            transport_activity.mark_relay(now);
            let _ = r.send(pairing.reject(id));
        }
        RuntimeCommand::CancelPairing(id, r) => {
            transport_activity.mark_tor(now);
            transport_activity.mark_relay(now);
            let _ = r.send(pairing.cancel(id));
        }
        RuntimeCommand::VerifyContact(id, r) => {
            let _ = r.send(communication.verify_contact(id, now));
        }
        RuntimeCommand::ResetContactVerification(id, r) => {
            let _ = r.send(communication.reset_contact_verification(id));
        }
        RuntimeCommand::RenameContact(id, name, r) => {
            let _ = r.send(communication.rename_contact(id, name, now));
        }
        RuntimeCommand::BlockContact(id, r) => {
            let _ = r.send(communication.block_contact(id, now));
        }
        RuntimeCommand::UnblockContact(id, r) => {
            let _ = r.send(communication.unblock_contact(id, now));
        }
        RuntimeCommand::RemoveContact(id, r) => {
            let _ = r.send(communication.remove_contact(id));
        }
        RuntimeCommand::ClearConversationHistory(id, r) => {
            let _ = r.send(communication.clear_conversation_history(id));
        }
        RuntimeCommand::MarkConversationRead(id, send_receipt, r) => {
            let _ = r.send(communication.mark_conversation_read_with_policy(id, now, send_receipt));
        }
        RuntimeCommand::QueueAttachment(request_value, r) => {
            let message_id = MessageId::from_opaque(request_value.message_id);
            let body = MessageBody::new(format!("Attachment: {}", request_value.name))
                .map_err(|_| RuntimeDriverError::Communication);
            let result = body.and_then(|body| {
                let _ = engine
                    .dispatch(EngineCommand::QueueMessage {
                        message_id,
                        conversation_id: ConversationId::from_opaque(request_value.conversation_id),
                        body,
                        reply_to: None,
                        at: now,
                    })
                    .map_err(|_| RuntimeDriverError::Engine)?;
                match communication.prepare_attachment(&request_value, now) {
                    Ok(()) => Ok(()),
                    Err(error) => {
                        let failure_code = ErrorCode::new("ATTACHMENT_PREPARE");
                        let _ = engine
                            .dispatch(EngineCommand::BeginMessageSend { message_id, at: now })
                            .map_err(|_| RuntimeDriverError::Engine)?;
                        let _ = engine
                            .dispatch(EngineCommand::MarkMessageFailed {
                                message_id,
                                at: now,
                                error_code: failure_code,
                            })
                            .map_err(|_| RuntimeDriverError::Engine)?;
                        Err(error)
                    }
                }
            });
            let _ = r.send(result);
        }
        RuntimeCommand::RetryAttachment(id, r) => {
            let _ = r.send(communication.retry_attachment(id, now));
        }
        RuntimeCommand::CancelAttachment(id, r) => {
            let _ = r.send(communication.cancel_attachment(id, now));
        }
        RuntimeCommand::ExportAttachment(id, destination, r) => {
            let _ = r.send(communication.export_attachment(id, destination));
        }
        RuntimeCommand::AttachmentSnapshot(r) => {
            let _ = r.send(communication.attachment_snapshot());
        }
        RuntimeCommand::NetworkSnapshot(r) => {
            let result = (|| {
                let snapshot = engine.snapshot().map_err(|_| RuntimeDriverError::Engine)?;
                let peers = snapshot
                    .contacts
                    .iter()
                    .map(|c| (c.id(), communication.connection_state(c.id())))
                    .collect();
                let peer_health = snapshot
                    .contacts
                    .iter()
                    .map(|c| (c.id(), communication.peer_health(c.id())))
                    .collect();
                let contact_names = communication.contact_names()?;
                let contact_verifications = communication.contact_verifications()?;
                Ok(NetworkSnapshot {
                    tor: tor.state(),
                    onion_address: tor.onion_address(),
                    peers,
                    peer_health,
                    contact_names,
                    contact_verifications,
                    tor_activity: transport_activity.tor,
                    relay_activity: transport_activity.relay,
                    peer_activity: transport_activity.peers.clone(),
                    probes: probes.latest(),
                })
            })();
            let _ = r.send(result);
        }
        RuntimeCommand::Diagnostics(r) => {
            let _ = r.send(diagnostics.export_json());
        }
        RuntimeCommand::Wake => {}
        RuntimeCommand::Shutdown(_) => unreachable!(),
    }
}

fn request_command<T>(
    sender: &SyncSender<RuntimeCommand>,
    make: impl FnOnce(Sender<Result<T, RuntimeDriverError>>) -> RuntimeCommand,
) -> Result<T, RuntimeDriverError> {
    let (tx, rx) = mpsc::channel();
    send_with_timeout(sender, make(tx))?;
    match rx.recv_timeout(COMMAND_WAIT) {
        Ok(result) => result,
        Err(RecvTimeoutError::Timeout) => Err(RuntimeDriverError::Pending),
        Err(RecvTimeoutError::Disconnected) => Err(RuntimeDriverError::Communication),
    }
}
fn request_query<T>(
    sender: &SyncSender<RuntimeCommand>,
    make: impl FnOnce(Sender<Result<T, RuntimeDriverError>>) -> RuntimeCommand,
) -> Result<T, RuntimeDriverError> {
    let (tx, rx) = mpsc::channel();
    send_with_timeout(sender, make(tx))?;
    match rx.recv_timeout(QUERY_WAIT) {
        Ok(result) => result,
        Err(_) => Err(RuntimeDriverError::Communication),
    }
}
fn request_blocking<T>(
    sender: &SyncSender<RuntimeCommand>,
    make: impl FnOnce(Sender<Result<T, RuntimeDriverError>>) -> RuntimeCommand,
) -> Result<T, RuntimeDriverError> {
    let (tx, rx) = mpsc::channel();
    send_with_timeout(sender, make(tx))?;
    rx.recv_timeout(QUERY_WAIT).map_err(|_| RuntimeDriverError::Communication)?
}

fn send_with_timeout(
    sender: &SyncSender<RuntimeCommand>,
    mut command: RuntimeCommand,
) -> Result<(), RuntimeDriverError> {
    let deadline = std::time::Instant::now() + ENQUEUE_WAIT;
    loop {
        match sender.try_send(command) {
            Ok(()) => return Ok(()),
            Err(TrySendError::Disconnected(_)) => return Err(RuntimeDriverError::Communication),
            Err(TrySendError::Full(returned)) => {
                if std::time::Instant::now() >= deadline {
                    return Err(RuntimeDriverError::Pending);
                }
                command = returned;
                thread::yield_now();
            }
        }
    }
}
fn current_timestamp() -> Result<Timestamp, RuntimeDriverError> {
    let duration =
        SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| RuntimeDriverError::Engine)?;
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
const fn map_health(state: TorState) -> HealthState {
    match state {
        TorState::Stopped => HealthState::Stopped,
        TorState::Starting => HealthState::Starting,
        TorState::Ready => HealthState::Ready,
        TorState::Degraded => HealthState::Degraded,
        TorState::Failed => HealthState::Failed,
    }
}
const fn map_peer_health(state: PeerConnectionState) -> HealthState {
    match state {
        PeerConnectionState::Ready => HealthState::Ready,
        PeerConnectionState::Failed => HealthState::Failed,
        PeerConnectionState::Disconnected => HealthState::Stopped,
        PeerConnectionState::Connecting
        | PeerConnectionState::Handshaking
        | PeerConnectionState::Reconnecting => HealthState::Starting,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_activity_is_monotonic_and_redacted() {
        let at = Timestamp::from_unix_millis(1).expect("valid timestamp");
        let mut ledger = TransportActivityLedger::default();
        ledger.mark_tor(at);
        ledger.mark_relay(at);
        ledger.mark_tor(at);

        assert_eq!(ledger.tor.sequence, 2);
        assert_eq!(ledger.relay.sequence, 1);
        assert_eq!(ledger.tor.last_activity_at, Some(at));
        assert_eq!(ledger.relay.last_activity_at, Some(at));
    }
}
