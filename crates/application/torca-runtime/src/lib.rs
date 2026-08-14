//! Single background owner for Tor, pairing, peer sessions and durable delivery.

mod attachments;
pub use attachments::{AttachmentSendRequest, AttachmentView};

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, SyncSender, TrySendError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use torca_attachments::AttachmentId;
use torca_client_engine::{EngineCommand, EngineHandle};
use torca_connectivity::{
    ConnectivityObserver, ConnectivitySnapshot, PeerProbeCandidate, PeerProbeSupervisor,
    RelayHealthHandle, RelayHealthPort, RelayHealthSnapshot, RelayHealthWorker,
};
use torca_contacts::{ContactId, ContactStatus};
use torca_conversations::ConversationId;
use torca_diagnostics::{
    Component, DiagnosticBuffer, DiagnosticCode, DiagnosticEvent, HealthState,
};
use torca_foundation::{
    ClassifiedError, ErrorCategory, ErrorCode, ErrorDescriptor, OpaqueId, RetryAdvice, Timestamp,
};
use torca_messaging::{MessageBody, MessageId};
use torca_delivery::ReactionPayload;
use torca_pairing::{PairingCode, PairingSessionId};
use torca_probing::{ProbeKind, ProbeResult, ProbeStatus, ProbeSupervisor, ProbeTarget};
use torca_runtime_policy::Freshness;

const IDLE_MAINTENANCE_INTERVAL: Duration = Duration::from_secs(1);
const ACTIVE_MAINTENANCE_INTERVAL: Duration = Duration::from_millis(100);
const COMMAND_WAIT: Duration = Duration::from_secs(10);
const QUERY_WAIT: Duration = Duration::from_secs(5);
const SHUTDOWN_WAIT: Duration = Duration::from_secs(15);
const MAILBOX_CAPACITY: usize = 256;
const ENQUEUE_WAIT: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TorState {
    Stopped,
    Starting,
    Ready,
    Degraded,
    Failed,
}
/// Publication health of this device's onion endpoint.  This is deliberately
/// separate from `TorState`: a bootstrapped Tor client can still be waiting
/// for introduction points or descriptor publication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OnionServiceState {
    Unknown,
    Publishing,
    Reachable,
    Degraded,
    Failed,
    Stopped,
}
/// Application-owned peer session state. Infrastructure adapters map their provider state here.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PeerConnectionStatus {
    Disconnected,
    Connecting,
    Handshaking,
    Ready,
    Reconnecting,
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
    pub state: PeerConnectionStatus,
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
    peers: BTreeMap<ContactId, TransportActivitySnapshot>,
}

impl TransportActivityLedger {
    fn mark_peer(&mut self, contact_id: ContactId, now: Timestamp) {
        Self::mark(self.peers.entry(contact_id).or_default(), now);
    }

    fn mark(activity: &mut TransportActivitySnapshot, now: Timestamp) {
        activity.last_activity_at = Some(now);
        activity.sequence = activity.sequence.saturating_add(1);
    }
}
impl PeerHealthSnapshot {
    pub const fn from_connection_state(state: PeerConnectionStatus) -> Self {
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
    pub peers: BTreeMap<ContactId, PeerConnectionStatus>,
    pub peer_health: BTreeMap<ContactId, PeerHealthSnapshot>,
    pub contact_names: BTreeMap<ContactId, String>,
    pub contact_verifications: BTreeMap<ContactId, ContactVerificationSnapshot>,
    pub peer_activity: BTreeMap<ContactId, TransportActivitySnapshot>,
    pub probes: Vec<ProbeResult>,
    pub connectivity: ConnectivitySnapshot,
    pub relay_info: Option<RelayServiceInfo>,
}

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelayServiceInfo {
    pub product_version: String,
    pub build_id: String,
    pub source_commit: String,
    pub protocol_version: u16,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeDriverError {
    Pairing,
    Communication,
    /// A stable, redacted descriptor preserved from a narrower application
    /// port.  The runtime must not erase the cause into `Communication`.
    Classified(ErrorDescriptor),
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

impl ClassifiedError for RuntimeDriverError {
    fn descriptor(&self) -> ErrorDescriptor {
        let (code, category, retry) = match self {
            Self::Pairing => {
                ("runtime.pairing_failed", ErrorCategory::Conflict, RetryAdvice::Never)
            }
            Self::Communication => (
                "runtime.communication_unavailable",
                ErrorCategory::Unavailable,
                RetryAdvice::Backoff,
            ),
            Self::Classified(descriptor) => return *descriptor,
            Self::Tor => {
                ("runtime.tor_unavailable", ErrorCategory::Unavailable, RetryAdvice::Backoff)
            }
            Self::Engine => ("runtime.engine_failed", ErrorCategory::Internal, RetryAdvice::Never),
            Self::Pending => {
                ("runtime.pending", ErrorCategory::Unavailable, RetryAdvice::Immediate)
            }
        };
        ErrorDescriptor::new(ErrorCode::new(code), category, retry)
    }
}

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
        ticket: Option<[u8; 16]>,
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
    /// Returns the next useful maintenance deadline. `None` means the worker
    /// can sleep until a command or network event arrives; it must not wake
    /// just to discover that there is no pairing work.
    fn next_maintenance_delay(&self, _now: Timestamp) -> Option<Duration> {
        Some(Duration::from_secs(1))
    }
    fn network_changed(&mut self, _now: Timestamp) {}
    fn shutdown(&mut self);
}
/// Owns only background delivery/inbound maintenance and peer session state.
pub trait PeerSessionPort: Send + 'static {
    fn recover(&mut self, now: Timestamp) -> Result<(), RuntimeDriverError>;
    fn maintenance(
        &mut self,
        contacts: &[ContactId],
        now: Timestamp,
    ) -> Result<(), RuntimeDriverError>;
    /// Invalidates stale transport sessions and resets reconnect backoff after
    /// an OS route/network change.
    fn network_changed(&mut self, _now: Timestamp) {}
    fn connection_state(&self, contact_id: ContactId) -> PeerConnectionStatus;
    fn peer_health(&self, contact_id: ContactId) -> PeerHealthSnapshot {
        PeerHealthSnapshot::from_connection_state(self.connection_state(contact_id))
    }
    /// Whether this device is the deterministic initiator of the keepalive
    /// for this relationship. The adapter supplies the transport capability;
    /// application owns cadence and retry policy.
    fn peer_probe_eligible(&self, _contact_id: ContactId) -> bool {
        true
    }
    /// Starts one bounded keepalive I/O operation. Implementations must return
    /// promptly after accepting it into their single-flight worker.
    fn begin_peer_probe(
        &mut self,
        _contact_id: ContactId,
        _probe_id: OpaqueId,
        _reported_rtt_ms: u64,
    ) -> Result<(), RuntimeDriverError> {
        Ok(())
    }
    /// Returns the contact whose pending keepalive completed. Health details
    /// remain available through `peer_health`, avoiding infrastructure errors
    /// in the application vocabulary.
    fn take_peer_probe_completion(
        &mut self,
        _now: Timestamp,
    ) -> Result<Option<ContactId>, RuntimeDriverError> {
        Ok(None)
    }
    fn shutdown(&mut self);
}

/// Contact administration is not a transport command, despite some actions
/// causing a peer session to be closed by its infrastructure implementation.
pub trait RelationshipAdminPort: Send + 'static {
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
}

pub trait ConversationReadPort: Send + 'static {
    fn mark_conversation_read(
        &mut self,
        conversation_id: OpaqueId,
        now: Timestamp,
    ) -> Result<(), RuntimeDriverError>;
}

pub trait AttachmentTransferPort: Send + 'static {
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
    fn attachment_snapshot(&self) -> Result<Vec<AttachmentView>, RuntimeDriverError>;
}

pub trait AttachmentExportPort: Send + 'static {
    fn export_attachment(
        &mut self,
        attachment_id: AttachmentId,
        destination: PathBuf,
    ) -> Result<(), RuntimeDriverError>;
    fn export_attachment_preview(
        &mut self,
        attachment_id: AttachmentId,
        destination: PathBuf,
    ) -> Result<(), RuntimeDriverError>;
}

/// Compatibility composition for the process runtime. New use cases should
/// depend on one of the narrow ports above, not this aggregate.
pub trait CommunicationDriver:
    PeerSessionPort
    + RelationshipAdminPort
    + ConversationReadPort
    + AttachmentTransferPort
    + AttachmentExportPort
{
    fn queue_reaction(
        &mut self,
        contact_id: ContactId,
        reaction: ReactionPayload,
        at: Timestamp,
    ) -> Result<(), RuntimeDriverError>;
}
pub trait TorDriver: Send + 'static {
    fn maintenance(&mut self, now: Timestamp) -> Result<(), RuntimeDriverError>;
    fn state(&self) -> TorState;
    fn onion_address(&self) -> Option<String>;
    fn onion_service_state(&self) -> OnionServiceState {
        if self.onion_address().is_some() {
            OnionServiceState::Publishing
        } else {
            OnionServiceState::Unknown
        }
    }
    fn shutdown(&mut self);
}

/// Relay connectivity is supervised outside the actor's critical path. A
/// probe implementation must be cheap to clone through `Arc` and may perform
/// blocking network work on the worker thread created by the supervisor.
pub trait RelayProbe: Send + Sync + 'static {
    fn probe(&self) -> Result<(), ErrorCode>;

    fn service_info(&self) -> Option<RelayServiceInfo> {
        None
    }
}

struct RuntimeRelayHealthPort(Arc<dyn RelayProbe>);

impl RelayHealthPort for RuntimeRelayHealthPort {
    fn check_relay_health(&self) -> Result<(), ErrorCode> {
        self.0.probe()
    }
}

enum RuntimeCommand {
    CreatePairing(PairingSessionId, Sender<Result<PairingInvitationView, RuntimeDriverError>>),
    JoinPairing(
        PairingSessionId,
        PairingCode,
        Option<[u8; 16]>,
        Sender<Result<(), RuntimeDriverError>>,
    ),
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
    MarkConversationRead(OpaqueId, Sender<Result<(), RuntimeDriverError>>),
    QueueAttachment(AttachmentSendRequest, Sender<Result<(), RuntimeDriverError>>),
    QueueReaction(ContactId, ReactionPayload, Timestamp, Sender<Result<(), RuntimeDriverError>>),
    RetryAttachment(OpaqueId, Sender<Result<(), RuntimeDriverError>>),
    CancelAttachment(OpaqueId, Sender<Result<(), RuntimeDriverError>>),
    ExportAttachment(AttachmentId, PathBuf, Sender<Result<(), RuntimeDriverError>>),
    ExportAttachmentPreview(AttachmentId, PathBuf, Sender<Result<(), RuntimeDriverError>>),
    AttachmentSnapshot(Sender<Result<Vec<AttachmentView>, RuntimeDriverError>>),
    NetworkSnapshot(Sender<Result<NetworkSnapshot, RuntimeDriverError>>),
    Diagnostics(Sender<String>),
    NetworkChanged,
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
        request_command(&self.sender, |r| RuntimeCommand::JoinPairing(id, code, None, r))
    }

    pub fn join_pairing_with_ticket(
        &self,
        id: PairingSessionId,
        code: PairingCode,
        ticket: Option<[u8; 16]>,
    ) -> Result<(), RuntimeDriverError> {
        request_command(&self.sender, |r| RuntimeCommand::JoinPairing(id, code, ticket, r))
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
        request_command(&self.sender, |r| RuntimeCommand::MarkConversationRead(id, r))
    }
    pub fn queue_attachment(&self, value: AttachmentSendRequest) -> Result<(), RuntimeDriverError> {
        request_command(&self.sender, |r| RuntimeCommand::QueueAttachment(value, r))
    }
    pub fn queue_reaction(
        &self,
        contact_id: ContactId,
        reaction: ReactionPayload,
        at: Timestamp,
    ) -> Result<(), RuntimeDriverError> {
        request_command(&self.sender, |r| RuntimeCommand::QueueReaction(contact_id, reaction, at, r))
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
    pub fn export_attachment_preview(
        &self,
        id: AttachmentId,
        destination: PathBuf,
    ) -> Result<(), RuntimeDriverError> {
        request_blocking(&self.sender, |r| {
            RuntimeCommand::ExportAttachmentPreview(id, destination, r)
        })
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

    /// Notify the actor that the platform network changed. This resets the
    /// relay supervisor immediately instead of waiting for its backoff timer.
    pub fn network_changed(&self) {
        let _ = send_with_timeout(&self.sender, RuntimeCommand::NetworkChanged);
    }
}

pub struct RuntimeOwner {
    sender: SyncSender<RuntimeCommand>,
    join: Option<JoinHandle<()>>,
    relay_worker: Option<RelayHealthWorker>,
}
impl RuntimeOwner {
    pub fn spawn<P: PairingDriver, C: CommunicationDriver, T: TorDriver>(
        engine: EngineHandle,
        pairing: P,
        communication: C,
        tor: T,
    ) -> (RuntimeHandle, Self) {
        Self::spawn_with_connectivity(
            engine,
            pairing,
            communication,
            tor,
            None,
            ConnectivityObserver::default(),
        )
    }

    pub fn spawn_with_relay_probe<P: PairingDriver, C: CommunicationDriver, T: TorDriver>(
        engine: EngineHandle,
        pairing: P,
        communication: C,
        tor: T,
        relay_probe: Option<Arc<dyn RelayProbe>>,
    ) -> (RuntimeHandle, Self) {
        Self::spawn_with_connectivity(
            engine,
            pairing,
            communication,
            tor,
            relay_probe,
            ConnectivityObserver::default(),
        )
    }

    pub fn spawn_with_connectivity<P: PairingDriver, C: CommunicationDriver, T: TorDriver>(
        engine: EngineHandle,
        mut pairing: P,
        mut communication: C,
        mut tor: T,
        relay_probe: Option<Arc<dyn RelayProbe>>,
        connectivity: ConnectivityObserver,
    ) -> (RuntimeHandle, Self) {
        let relay_info = relay_probe.clone();
        let relay_worker = relay_probe.and_then(|probe| {
            RelayHealthWorker::spawn(Arc::new(RuntimeRelayHealthPort(probe)))
                .map_err(|error| {
                    eprintln!("torca-runtime: relay supervisor unavailable: {error}");
                    error
                })
                .ok()
        });
        let relay_health = relay_worker.as_ref().map(RelayHealthWorker::handle);
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
                relay_health,
                relay_info,
                connectivity,
            );
            communication.shutdown();
            pairing.shutdown();
            tor.shutdown();
        });
        (handle, Self { sender, join: Some(join), relay_worker })
    }
    pub fn shutdown(mut self) -> Result<(), RuntimeDriverError> {
        let (tx, rx) = mpsc::channel();
        send_with_timeout(&self.sender, RuntimeCommand::Shutdown(tx))?;
        rx.recv_timeout(SHUTDOWN_WAIT).map_err(|_| RuntimeDriverError::Communication)?;
        if let Some(join) = self.join.take() {
            join.join().map_err(|_| RuntimeDriverError::Communication)?;
        }
        if let Some(worker) = self.relay_worker.take() {
            worker.shutdown();
        }
        Ok(())
    }

    pub fn network_changed(&self) {
        let _ = send_with_timeout(&self.sender, RuntimeCommand::NetworkChanged);
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
    relay_health: Option<RelayHealthHandle>,
    relay_info: Option<Arc<dyn RelayProbe>>,
    connectivity: ConnectivityObserver,
) {
    let mut last_tor_state = None;
    let mut last_onion_state = None;
    let mut last_relay_state = None::<(ProbeStatus, ErrorCode)>;
    let mut last_peer_states = BTreeMap::<ContactId, PeerConnectionStatus>::new();
    let mut last_peer_successes = BTreeMap::<ContactId, Option<Timestamp>>::new();
    let mut tor_failed = false;
    let mut pairing_failed = false;
    let mut communication_failed = false;
    let mut probes = ProbeSupervisor::default();
    let mut peer_probes = PeerProbeSupervisor::default();
    let mut transport_activity = TransportActivityLedger::default();
    let mut next_maintenance_at = std::time::Instant::now();
    let mut peer_probe_deadline = None;
    let mut contacts = Vec::<ContactId>::new();
    let mut refresh_contacts = true;
    loop {
        let wait = next_maintenance_at.saturating_duration_since(std::time::Instant::now());
        match receiver.recv_timeout(wait) {
            Ok(RuntimeCommand::Shutdown(response)) => {
                let _ = response.send(());
                break;
            }
            Ok(RuntimeCommand::NetworkChanged) => {
                refresh_contacts = true;
                if let Some(relay) = &relay_health {
                    relay.network_changed();
                }
                let now = current_timestamp().unwrap_or(Timestamp::UNIX_EPOCH);
                pairing.network_changed(now);
                communication.network_changed(now);
                record(
                    diagnostics,
                    sequence,
                    now,
                    Component::Relay,
                    HealthState::Starting,
                    "RELAY_NETWORK_CHANGED",
                );
            }
            Ok(command) => {
                refresh_contacts = true;
                let now = current_timestamp().unwrap_or(Timestamp::UNIX_EPOCH);
                handle_command(
                    command,
                    engine,
                    pairing,
                    communication,
                    tor,
                    &probes,
                    relay_info.as_ref(),
                    relay_health.as_ref(),
                    &mut transport_activity,
                    &connectivity,
                    diagnostics,
                    sequence,
                    now,
                );
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        let now = current_timestamp().unwrap_or(Timestamp::UNIX_EPOCH);
        if refresh_contacts {
            if let Ok(snapshot) = engine.snapshot() {
                contacts = snapshot
                    .contacts
                    .iter()
                    .filter(|contact| contact.status() == ContactStatus::Active)
                    .map(torca_contacts::Contact::id)
                    .collect();
            }
            refresh_contacts = false;
        }
        let relay_snapshot = relay_health
            .as_ref()
            .map_or_else(RelayHealthSnapshot::default, RelayHealthHandle::snapshot);
        let relay_state = (relay_snapshot.status, relay_snapshot.diagnostic_code);
        if last_relay_state.as_ref() != Some(&relay_state) {
            record(
                diagnostics,
                sequence,
                now,
                Component::Relay,
                map_probe_health(relay_snapshot.status),
                relay_event_code(relay_snapshot.status),
            );
            last_relay_state = Some(relay_state);
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
        let onion_state = tor.onion_service_state();
        record_runtime_probes(
            &mut probes,
            tor_state,
            onion_state,
            communication_failed,
            relay_probe_result(relay_snapshot, now),
            now,
        );
        for probe in probes.latest() {
            connectivity.record_probe(&probe);
        }
        if last_tor_state != Some(tor_state) {
            last_tor_state = Some(tor_state);
            record(
                diagnostics,
                sequence,
                now,
                Component::Tor,
                map_health(tor_state),
                "TOR_STATE_CHANGED",
            );
        }
        if last_onion_state != Some(onion_state) {
            last_onion_state = Some(onion_state);
            record(
                diagnostics,
                sequence,
                now,
                Component::Tor,
                map_onion_health(onion_state),
                onion_event_code(onion_state),
            );
        }
        let maintenance_result = communication.maintenance(&contacts, now).and_then(|()| {
            peer_probe_deadline = maintain_peer_probes(
                communication,
                &contacts,
                &mut peer_probes,
                now,
            )?;
            Ok(())
        });
        observe_maintenance(
            maintenance_result,
            &mut communication_failed,
            diagnostics,
            sequence,
            now,
            Component::Peer,
            "COMMUNICATION_MAINTENANCE_FAILED",
            "COMMUNICATION_MAINTENANCE_RECOVERED",
        );
        let mut current = BTreeMap::new();
        let mut current_successes = BTreeMap::new();
        for id in contacts.iter().copied() {
            let state = communication.connection_state(id);
            if last_peer_states.get(&id) != Some(&state) {
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
            let health = communication.peer_health(id);
            if health.last_success_at.is_some()
                && last_peer_successes.get(&id) != Some(&health.last_success_at)
            {
                transport_activity.mark_peer(id, now);
            }
            current.insert(id, state);
            connectivity.set_peer_ready(id.to_opaque(), state == PeerConnectionStatus::Ready);
            current_successes.insert(id, health.last_success_at);
        }
        let active_transport = current.values().any(|state| {
            matches!(
                state,
                PeerConnectionStatus::Connecting
                    | PeerConnectionStatus::Handshaking
                    | PeerConnectionStatus::Reconnecting
            )
        });
        last_peer_states = current;
        last_peer_successes = current_successes;

        let mut next_delay = if active_transport {
            ACTIVE_MAINTENANCE_INTERVAL
        } else {
            IDLE_MAINTENANCE_INTERVAL
        };
        if let Some(pairing_delay) = pairing.next_maintenance_delay(now) {
            next_delay = next_delay.min(pairing_delay);
        }
        if !active_transport
            && let Some(deadline) = peer_probe_deadline
            && let Some(delay) = deadline.duration_since(now)
        {
            next_delay = next_delay.min(delay);
        }
        next_maintenance_at = std::time::Instant::now() + next_delay;
    }
}

/// Application owns peer probe cadence and retry timing. The communication
/// adapter only validates eligibility, executes one bounded keepalive and
/// maintains its transport-health sample.
fn maintain_peer_probes<C: PeerSessionPort>(
    communication: &mut C,
    contacts: &[ContactId],
    supervisor: &mut PeerProbeSupervisor,
    now: Timestamp,
) -> Result<Option<Timestamp>, RuntimeDriverError> {
    if let Some(contact_id) = communication.take_peer_probe_completion(now)? {
        let health = communication.peer_health(contact_id);
        supervisor.complete(
            contact_id.to_opaque(),
            health.consecutive_failures == 0 && health.last_success_at.is_some(),
            now,
        );
    }
    let candidates = contacts
        .iter()
        .copied()
        .map(|contact_id| {
            let health = communication.peer_health(contact_id);
            PeerProbeCandidate {
                peer_id: contact_id.to_opaque(),
                ready: health.state == PeerConnectionStatus::Ready,
                eligible: communication.peer_probe_eligible(contact_id),
                freshness: peer_freshness(health.last_success_at, now),
                reported_rtt_ms: health.rtt_ms,
            }
        })
        .collect::<Vec<_>>();
    supervisor.reconcile(&candidates, now);
    let Some(request) = supervisor.next_due(&candidates, now) else {
        return Ok(supervisor.next_deadline());
    };
    let Some(contact_id) =
        contacts.iter().copied().find(|contact_id| contact_id.to_opaque() == request.peer_id)
    else {
        supervisor.complete(request.peer_id, false, now);
        return Ok(supervisor.next_deadline());
    };
    if let Err(error) =
        communication.begin_peer_probe(contact_id, request.probe_id, request.reported_rtt_ms)
    {
        supervisor.complete(request.peer_id, false, now);
        return Err(error);
    }
    Ok(supervisor.next_deadline())
}

fn peer_freshness(last_success_at: Option<Timestamp>, now: Timestamp) -> Freshness {
    let Some(last_success_at) = last_success_at else { return Freshness::Unknown };
    let Some(age) = now.duration_since(last_success_at) else { return Freshness::Stale };
    if age <= Duration::from_secs(15) {
        Freshness::Live
    } else if age <= Duration::from_secs(120) {
        Freshness::Recent
    } else {
        Freshness::Stale
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
    onion_state: OnionServiceState,
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
        match onion_state {
            OnionServiceState::Reachable => ProbeStatus::Healthy,
            OnionServiceState::Publishing => ProbeStatus::Checking,
            OnionServiceState::Degraded => ProbeStatus::Degraded,
            OnionServiceState::Failed => ProbeStatus::Failed,
            OnionServiceState::Unknown | OnionServiceState::Stopped => ProbeStatus::Unknown,
        },
        match onion_state {
            OnionServiceState::Reachable => "ONION_REACHABLE",
            OnionServiceState::Publishing => "ONION_PUBLISHING",
            OnionServiceState::Degraded => "ONION_DEGRADED",
            OnionServiceState::Failed => "ONION_FAILED",
            OnionServiceState::Unknown | OnionServiceState::Stopped => "ONION_UNAVAILABLE",
        },
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

fn relay_probe_result(snapshot: RelayHealthSnapshot, measured_at: Timestamp) -> ProbeResult {
    ProbeResult {
        target: ProbeTarget::Relay,
        kind: ProbeKind::Connectivity,
        status: snapshot.status,
        diagnostic_code: snapshot.diagnostic_code.to_string(),
        latency_ms: snapshot.latency_ms,
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
    relay_info: Option<&Arc<dyn RelayProbe>>,
    relay_health: Option<&RelayHealthHandle>,
    transport_activity: &mut TransportActivityLedger,
    connectivity: &ConnectivityObserver,
    diagnostics: &mut DiagnosticBuffer,
    sequence: &mut u128,
    now: Timestamp,
) {
    match command {
        RuntimeCommand::CreatePairing(id, r) => {
            wake_relay(relay_health);
            let result = pairing.create(id, now);
            record_pairing_result(&result, "CREATE", diagnostics, sequence, now);
            let _ = r.send(result);
        }
        RuntimeCommand::JoinPairing(id, code, ticket, r) => {
            wake_relay(relay_health);
            let result = pairing.join(id, code, ticket, now);
            record_pairing_result(&result, "JOIN", diagnostics, sequence, now);
            let _ = r.send(result);
        }
        RuntimeCommand::ApprovePairing(id, r) => {
            wake_relay(relay_health);
            let result = pairing.approve(id, now);
            record_pairing_result(&result, "APPROVE", diagnostics, sequence, now);
            let _ = r.send(result);
        }
        RuntimeCommand::RejectPairing(id, r) => {
            wake_relay(relay_health);
            let result = pairing.reject(id);
            record_pairing_result(&result, "REJECT", diagnostics, sequence, now);
            let _ = r.send(result);
        }
        RuntimeCommand::CancelPairing(id, r) => {
            wake_relay(relay_health);
            let result = pairing.cancel(id);
            record_pairing_result(&result, "CANCEL", diagnostics, sequence, now);
            let _ = r.send(result);
        }
        RuntimeCommand::VerifyContact(id, r) => {
            transport_activity.mark_peer(id, now);
            let _ = r.send(communication.verify_contact(id, now));
        }
        RuntimeCommand::ResetContactVerification(id, r) => {
            transport_activity.mark_peer(id, now);
            let _ = r.send(communication.reset_contact_verification(id));
        }
        RuntimeCommand::RenameContact(id, name, r) => {
            transport_activity.mark_peer(id, now);
            let _ = r.send(communication.rename_contact(id, name, now));
        }
        RuntimeCommand::BlockContact(id, r) => {
            transport_activity.mark_peer(id, now);
            let _ = r.send(communication.block_contact(id, now));
        }
        RuntimeCommand::UnblockContact(id, r) => {
            transport_activity.mark_peer(id, now);
            let _ = r.send(communication.unblock_contact(id, now));
        }
        RuntimeCommand::RemoveContact(id, r) => {
            transport_activity.mark_peer(id, now);
            let _ = r.send(communication.remove_contact(id));
        }
        RuntimeCommand::ClearConversationHistory(id, r) => {
            let _ = r.send(communication.clear_conversation_history(id));
        }
        RuntimeCommand::MarkConversationRead(id, r) => {
            let _ = r.send(communication.mark_conversation_read(id, now));
        }
        RuntimeCommand::QueueAttachment(request_value, r) => {
            let message_id = MessageId::from_opaque(request_value.message_id);
            let body = MessageBody::new(format!("Attachment: {}", request_value.name))
                .map_err(|_| RuntimeDriverError::Communication);
            let result = body.and_then(|body| {
                // `attachments.message_id` is a foreign key. The companion
                // message must be durable before attachment metadata can be
                // committed; this actor cannot run delivery maintenance until
                // the command returns, so the message is not sent in between.
                if engine
                    .dispatch(EngineCommand::QueueMessage {
                        message_id,
                        conversation_id: ConversationId::from_opaque(request_value.conversation_id),
                        body,
                        reply_to: None,
                        at: now,
                    })
                    .is_err()
                {
                    return Err(RuntimeDriverError::Engine);
                }
                if let Err(error) = communication.prepare_attachment(&request_value, now) {
                    // A failed local attachment must not leave a deliverable
                    // placeholder message behind. Cancelling also prevents the
                    // durable outbox from transmitting it on a later tick.
                    let _ = engine.dispatch(EngineCommand::CancelMessage { message_id, at: now });
                    return Err(error);
                }
                Ok(())
            });
            let _ = r.send(result);
        }
        RuntimeCommand::QueueReaction(contact_id, reaction, at, r) => {
            let _ = r.send(communication.queue_reaction(contact_id, reaction, at));
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
        RuntimeCommand::ExportAttachmentPreview(id, destination, r) => {
            let _ = r.send(communication.export_attachment_preview(id, destination));
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
                    peer_activity: transport_activity.peers.clone(),
                    probes: probes.latest(),
                    connectivity: connectivity.snapshot(),
                    relay_info: relay_info.and_then(|source| source.service_info()),
                })
            })();
            let _ = r.send(result);
        }
        RuntimeCommand::Diagnostics(r) => {
            let _ = r.send(diagnostics.export_json());
        }
        RuntimeCommand::Wake => {}
        RuntimeCommand::NetworkChanged => unreachable!(),
        RuntimeCommand::Shutdown(_) => unreachable!(),
    }
}

fn wake_relay(relay_health: Option<&RelayHealthHandle>) {
    if let Some(relay_health) = relay_health {
        relay_health.wake();
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

fn record_pairing_result<T>(
    result: &Result<T, RuntimeDriverError>,
    action: &str,
    diagnostics: &mut DiagnosticBuffer,
    sequence: &mut u128,
    now: Timestamp,
) {
    let (state, suffix) = match result {
        Ok(_) => (HealthState::Ready, "ACCEPTED"),
        Err(RuntimeDriverError::Pending) => (HealthState::Degraded, "QUEUED"),
        Err(RuntimeDriverError::Communication | RuntimeDriverError::Tor) => {
            (HealthState::Degraded, "RETRYING")
        }
        Err(_) => (HealthState::Failed, "FAILED"),
    };
    let code = format!("PAIRING_{action}_{suffix}");
    record(diagnostics, sequence, now, Component::Engine, state, &code);
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
const fn map_onion_health(state: OnionServiceState) -> HealthState {
    match state {
        OnionServiceState::Reachable => HealthState::Ready,
        OnionServiceState::Degraded => HealthState::Degraded,
        OnionServiceState::Failed => HealthState::Failed,
        OnionServiceState::Stopped => HealthState::Stopped,
        OnionServiceState::Unknown | OnionServiceState::Publishing => HealthState::Starting,
    }
}
const fn onion_event_code(state: OnionServiceState) -> &'static str {
    match state {
        OnionServiceState::Unknown => "ONION_UNKNOWN",
        OnionServiceState::Publishing => "ONION_PUBLISHING",
        OnionServiceState::Reachable => "ONION_REACHABLE",
        OnionServiceState::Degraded => "ONION_DEGRADED",
        OnionServiceState::Failed => "ONION_FAILED",
        OnionServiceState::Stopped => "ONION_STOPPED",
    }
}
const fn map_peer_health(state: PeerConnectionStatus) -> HealthState {
    match state {
        PeerConnectionStatus::Ready => HealthState::Ready,
        PeerConnectionStatus::Failed => HealthState::Failed,
        PeerConnectionStatus::Disconnected => HealthState::Stopped,
        PeerConnectionStatus::Connecting
        | PeerConnectionStatus::Handshaking
        | PeerConnectionStatus::Reconnecting => HealthState::Starting,
    }
}
const fn map_probe_health(state: ProbeStatus) -> HealthState {
    match state {
        ProbeStatus::Healthy => HealthState::Ready,
        ProbeStatus::Failed | ProbeStatus::Unreachable | ProbeStatus::Degraded => {
            HealthState::Degraded
        }
        ProbeStatus::Checking | ProbeStatus::Unknown | ProbeStatus::Disabled => {
            HealthState::Starting
        }
    }
}
const fn relay_event_code(state: ProbeStatus) -> &'static str {
    match state {
        ProbeStatus::Healthy => "RELAY_CONNECTED",
        ProbeStatus::Degraded => "RELAY_DEGRADED",
        ProbeStatus::Failed | ProbeStatus::Unreachable => "RELAY_DISCONNECTED",
        ProbeStatus::Checking => "RELAY_CONNECTING",
        ProbeStatus::Unknown | ProbeStatus::Disabled => "RELAY_UNKNOWN",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contact_activity_is_monotonic_and_redacted() {
        let at = Timestamp::from_unix_millis(1).expect("valid timestamp");
        let contact = ContactId::from_opaque(OpaqueId::from_u128(1));
        let mut ledger = TransportActivityLedger::default();
        ledger.mark_peer(contact, at);
        ledger.mark_peer(contact, at);

        let activity = ledger.peers.get(&contact).expect("contact activity");
        assert_eq!(activity.sequence, 2);
        assert_eq!(activity.last_activity_at, Some(at));
    }
}
