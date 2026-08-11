//! TorcaRuntime adapter for the completed pairing coordinator/runtime.

use std::collections::BTreeMap;
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use torca_client_engine::EngineHandle;
use torca_crypto::{CryptoProvider, RustCryptoProvider};
use torca_foundation::{OpaqueId, Timestamp};
use torca_pairing::{PairingCode, PairingSessionId, PairingState};
use torca_pairing_coordinator::{
    LocalPairingContext, PairingApprovalPort, PairingCryptoPort, PairingPeerSecretStore,
    PairingPollReport, PairingRendezvousPort, PairingRuntime, PairingRuntimeError,
};
use torca_runtime::{PairingDriver, PairingInvitationView, RuntimeDriverError};
use torca_storage_sqlite::SqlCipherRelationshipAdmin;
use torca_tor::SharedTorEndpoint;

pub struct RuntimePairingDriver<R, C, A, S> {
    runtime: PairingRuntime<R, C, A, S>,
    engine: EngineHandle,
    tor_endpoint: SharedTorEndpoint,
    random: RustCryptoProvider,
    contact_metadata: Option<SqlCipherRelationshipAdmin>,
    poll_schedule: BTreeMap<PairingSessionId, PairingPollSchedule>,
}

#[derive(Clone, Copy, Debug)]
struct PairingPollSchedule {
    next_at: Timestamp,
    consecutive_failures: u8,
}

const ACTIVE_POLL_INTERVAL: Duration = Duration::from_secs(1);
const IDLE_POLL_INTERVAL: Duration = Duration::from_secs(2);
const MAX_POLL_BACKOFF: Duration = Duration::from_secs(30);
const WORKER_TICK: Duration = Duration::from_millis(250);
/// Allow the first Tor stream establishment to complete before classifying a
/// command as pending. The outer runtime command timeout is ten seconds, so
/// this leaves a small margin for response propagation while avoiding the old
/// 250 ms false-queue path on cold Android/desktop boots.
const INTERACTIVE_REPLY_WAIT: Duration = Duration::from_secs(8);
impl<R, C, A, S> RuntimePairingDriver<R, C, A, S>
where
    R: PairingRendezvousPort,
    C: PairingCryptoPort,
    A: PairingApprovalPort,
    S: PairingPeerSecretStore,
{
    pub const fn new(
        runtime: PairingRuntime<R, C, A, S>,
        engine: EngineHandle,
        tor_endpoint: SharedTorEndpoint,
    ) -> Self {
        Self {
            runtime,
            engine,
            tor_endpoint,
            random: RustCryptoProvider,
            contact_metadata: None,
            poll_schedule: BTreeMap::new(),
        }
    }
    #[must_use]
    pub fn with_contact_metadata(mut self, metadata: SqlCipherRelationshipAdmin) -> Self {
        self.contact_metadata = Some(metadata);
        self
    }
    fn context(&mut self) -> Result<LocalPairingContext, RuntimeDriverError> {
        let identity = self
            .engine
            .snapshot()
            .map_err(|_| RuntimeDriverError::Engine)?
            .identity
            .ok_or(RuntimeDriverError::Pairing)?;
        // The local onion endpoint is a readiness dependency, not a protocol
        // rejection. Keep it retryable so a cold Android Tor bootstrap does
        // not create a permanent pairing failure.
        let onion_address = self.tor_endpoint.get().ok_or(RuntimeDriverError::Tor)?;
        Ok(LocalPairingContext {
            display_name: identity.profile().map_or_else(
                || "Torca".to_owned(),
                |profile| profile.display_name().as_str().to_owned(),
            ),
            public_identity: identity.public().clone(),
            onion_address,
            capability_id: self.random_id()?,
        })
    }
    fn random_id(&mut self) -> Result<OpaqueId, RuntimeDriverError> {
        for _ in 0..8 {
            let mut bytes = [0_u8; 16];
            self.random.fill_random(&mut bytes).map_err(|_| RuntimeDriverError::Pairing)?;
            let id = OpaqueId::from_bytes(bytes);
            if !id.is_nil() {
                return Ok(id);
            }
        }
        Err(RuntimeDriverError::Pairing)
    }
    fn active_sessions(&self) -> Result<Vec<PairingSessionId>, RuntimeDriverError> {
        self.engine.snapshot().map_err(|_| RuntimeDriverError::Pairing).map(|snapshot| {
            snapshot
                .pairings
                .into_iter()
                .filter(|s| {
                    !matches!(
                        s.state(),
                        PairingState::Rejected | PairingState::Cancelled | PairingState::Expired
                    )
                })
                .map(|s| s.id())
                .collect()
        })
    }
}
impl<R, C, A, S> PairingDriver for RuntimePairingDriver<R, C, A, S>
where
    R: PairingRendezvousPort + Send + 'static,
    C: PairingCryptoPort + Send + 'static,
    A: PairingApprovalPort + Send + 'static,
    S: PairingPeerSecretStore + Send + 'static,
{
    fn create(
        &mut self,
        session_id: PairingSessionId,
        now: Timestamp,
    ) -> Result<PairingInvitationView, RuntimeDriverError> {
        let context = self.context()?;
        let invitation =
            self.runtime.create_invitation(session_id, context, now).map_err(map_pairing_error)?;
        self.schedule_now(session_id, now);
        Ok(PairingInvitationView {
            session_id: invitation.session_id,
            code: invitation.code,
            uri: invitation.uri,
            expires_at: invitation.expires_at,
        })
    }
    fn join(
        &mut self,
        session_id: PairingSessionId,
        code: PairingCode,
        ticket: Option<[u8; 16]>,
        now: Timestamp,
    ) -> Result<(), RuntimeDriverError> {
        let context = self.context()?;
        self.runtime
            .join_invitation(session_id, code, context, now, ticket)
            .map_err(map_pairing_error)?;
        self.schedule_now(session_id, now);
        Ok(())
    }
    fn approve(
        &mut self,
        session_id: PairingSessionId,
        now: Timestamp,
    ) -> Result<(), RuntimeDriverError> {
        self.runtime.approve(session_id, now).map_err(map_pairing_error)?;
        self.schedule_now(session_id, now);
        Ok(())
    }
    fn reject(&mut self, session_id: PairingSessionId) -> Result<(), RuntimeDriverError> {
        self.runtime.reject(session_id).map_err(map_pairing_error)
    }
    fn cancel(&mut self, session_id: PairingSessionId) -> Result<(), RuntimeDriverError> {
        self.runtime.cancel(session_id).map_err(map_pairing_error)
    }
    fn maintenance(&mut self, now: Timestamp) -> Result<(), RuntimeDriverError> {
        self.runtime.maintenance(now).map_err(|_| RuntimeDriverError::Pairing)?;
        let active_sessions = self.active_sessions()?;
        self.poll_schedule.retain(|id, _| active_sessions.contains(id));
        for session_id in active_sessions {
            if self.poll_schedule.get(&session_id).is_some_and(|state| now < state.next_at) {
                continue;
            }
            match self.runtime.poll(session_id, now) {
                Ok(report) => {
                    let had_activity = report != PairingPollReport::default();
                    if let (Some(completed), Some(metadata)) =
                        (report.completed_contact, self.contact_metadata.as_mut())
                    {
                        let _ = metadata.rename_contact(
                            completed.contact_id,
                            completed.display_name,
                            now,
                        );
                    }
                    self.schedule_success(session_id, now, had_activity);
                }
                Err(torca_pairing_coordinator::PairingRuntimeError::SessionNotFound) => {
                    self.poll_schedule.remove(&session_id);
                }
                Err(_) => self.schedule_failure(session_id, now),
            }
        }
        Ok(())
    }
    fn network_changed(&mut self, now: Timestamp) {
        self.runtime.network_changed();
        for session_id in self.poll_schedule.keys().copied().collect::<Vec<_>>() {
            self.schedule_now(session_id, now);
        }
    }
    fn shutdown(&mut self) {
        if let Ok(sessions) = self.active_sessions() {
            for id in sessions {
                let _ = self.runtime.close_transport(id);
            }
        }
    }
}

impl<R, C, A, S> RuntimePairingDriver<R, C, A, S> {
    fn schedule_now(&mut self, session_id: PairingSessionId, now: Timestamp) {
        self.poll_schedule
            .insert(session_id, PairingPollSchedule { next_at: now, consecutive_failures: 0 });
    }

    fn schedule_success(&mut self, session_id: PairingSessionId, now: Timestamp, active: bool) {
        let delay = if active { ACTIVE_POLL_INTERVAL } else { IDLE_POLL_INTERVAL };
        let next_at = now.checked_add(delay).unwrap_or(now);
        self.poll_schedule
            .insert(session_id, PairingPollSchedule { next_at, consecutive_failures: 0 });
    }

    fn schedule_failure(&mut self, session_id: PairingSessionId, now: Timestamp) {
        let failures = self
            .poll_schedule
            .get(&session_id)
            .map_or(1, |state| state.consecutive_failures.saturating_add(1));
        let exponent = u32::from(failures.saturating_sub(1).min(5));
        let delay = Duration::from_secs(1_u64 << exponent).min(MAX_POLL_BACKOFF);
        let next_at = now.checked_add(delay).unwrap_or(now);
        self.poll_schedule
            .insert(session_id, PairingPollSchedule { next_at, consecutive_failures: failures });
    }
}

fn map_pairing_error(error: PairingRuntimeError) -> RuntimeDriverError {
    match error {
        // A rendezvous transport failure is transient and must participate in
        // the supervisor backoff loop. Protocol/session errors are terminal
        // for the current invitation and must not be retried forever.
        PairingRuntimeError::Coordinator(
            torca_pairing_coordinator::PairingCoordinatorError::Rendezvous,
        ) => RuntimeDriverError::Communication,
        PairingRuntimeError::SessionNotFound => RuntimeDriverError::Pairing,
        _ => RuntimeDriverError::Pairing,
    }
}

enum PairingWorkerCommand {
    Create {
        session_id: PairingSessionId,
        now: Timestamp,
        reply: SyncSender<Result<PairingInvitationView, RuntimeDriverError>>,
    },
    Join {
        session_id: PairingSessionId,
        code: PairingCode,
        ticket: Option<[u8; 16]>,
        now: Timestamp,
        reply: SyncSender<Result<(), RuntimeDriverError>>,
    },
    Approve {
        session_id: PairingSessionId,
        now: Timestamp,
        reply: SyncSender<Result<(), RuntimeDriverError>>,
    },
    Reject {
        session_id: PairingSessionId,
        reply: SyncSender<Result<(), RuntimeDriverError>>,
    },
    Cancel {
        session_id: PairingSessionId,
        reply: SyncSender<Result<(), RuntimeDriverError>>,
    },
    NetworkChanged(Timestamp),
    Shutdown,
}

/// Isolates relay I/O from the main runtime actor. Periodic polls are coalesced in a bounded
/// mailbox, so a slow Tor circuit cannot freeze snapshots, message delivery, or the UI.
pub struct PairingWorkerDriver {
    sender: SyncSender<PairingWorkerCommand>,
    worker: Option<JoinHandle<()>>,
}

impl PairingWorkerDriver {
    pub fn spawn<D: PairingDriver>(mut driver: D) -> Result<Self, RuntimeDriverError> {
        let (sender, receiver) = mpsc::sync_channel(8);
        let worker = std::thread::Builder::new()
            .name("torca-pairing-supervisor".to_owned())
            .spawn(move || run_pairing_worker(&mut driver, &receiver))
            .map_err(|_| RuntimeDriverError::Pairing)?;
        Ok(Self { sender, worker: Some(worker) })
    }

    fn request<T>(
        &self,
        build: impl FnOnce(SyncSender<Result<T, RuntimeDriverError>>) -> PairingWorkerCommand,
    ) -> Result<T, RuntimeDriverError> {
        let (reply, receiver) = mpsc::sync_channel(1);
        match self.sender.try_send(build(reply)) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => return Err(RuntimeDriverError::Pending),
            Err(TrySendError::Disconnected(_)) => return Err(RuntimeDriverError::Pairing),
        }
        receiver.recv_timeout(INTERACTIVE_REPLY_WAIT).map_err(|error| match error {
            mpsc::RecvTimeoutError::Timeout => RuntimeDriverError::Pending,
            mpsc::RecvTimeoutError::Disconnected => RuntimeDriverError::Pairing,
        })?
    }
}

impl PairingDriver for PairingWorkerDriver {
    fn create(
        &mut self,
        session_id: PairingSessionId,
        now: Timestamp,
    ) -> Result<PairingInvitationView, RuntimeDriverError> {
        self.request(|reply| PairingWorkerCommand::Create { session_id, now, reply })
    }

    fn join(
        &mut self,
        session_id: PairingSessionId,
        code: PairingCode,
        ticket: Option<[u8; 16]>,
        now: Timestamp,
    ) -> Result<(), RuntimeDriverError> {
        self.request(|reply| PairingWorkerCommand::Join { session_id, code, ticket, now, reply })
    }

    fn approve(
        &mut self,
        session_id: PairingSessionId,
        now: Timestamp,
    ) -> Result<(), RuntimeDriverError> {
        self.request(|reply| PairingWorkerCommand::Approve { session_id, now, reply })
    }

    fn reject(&mut self, session_id: PairingSessionId) -> Result<(), RuntimeDriverError> {
        self.request(|reply| PairingWorkerCommand::Reject { session_id, reply })
    }

    fn cancel(&mut self, session_id: PairingSessionId) -> Result<(), RuntimeDriverError> {
        self.request(|reply| PairingWorkerCommand::Cancel { session_id, reply })
    }

    fn maintenance(&mut self, now: Timestamp) -> Result<(), RuntimeDriverError> {
        let _ = now;
        // The worker owns its own deadline-driven poll/maintenance clock.
        // RuntimeOwner still calls this trait method for compatibility, but it
        // must not enqueue a periodic message behind interactive relay I/O.
        Ok(())
    }

    fn network_changed(&mut self, now: Timestamp) {
        let _ = self.sender.try_send(PairingWorkerCommand::NetworkChanged(now));
    }

    fn shutdown(&mut self) {
        let _ = self.sender.send(PairingWorkerCommand::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn run_pairing_worker<D: PairingDriver>(driver: &mut D, receiver: &Receiver<PairingWorkerCommand>) {
    loop {
        let command = match receiver.recv_timeout(WORKER_TICK) {
            Ok(command) => command,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if let Some(now) = worker_timestamp() {
                    let _ = driver.maintenance(now);
                }
                continue;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        };
        match command {
            PairingWorkerCommand::Create { session_id, now, reply } => {
                let _ = reply.send(driver.create(session_id, now));
            }
            PairingWorkerCommand::Join { session_id, code, ticket, now, reply } => {
                let _ = reply.send(driver.join(session_id, code, ticket, now));
            }
            PairingWorkerCommand::Approve { session_id, now, reply } => {
                let _ = reply.send(driver.approve(session_id, now));
            }
            PairingWorkerCommand::Reject { session_id, reply } => {
                let _ = reply.send(driver.reject(session_id));
            }
            PairingWorkerCommand::Cancel { session_id, reply } => {
                let _ = reply.send(driver.cancel(session_id));
            }
            PairingWorkerCommand::NetworkChanged(now) => driver.network_changed(now),
            PairingWorkerCommand::Shutdown => {
                driver.shutdown();
                break;
            }
        }
    }
}

fn worker_timestamp() -> Option<Timestamp> {
    let elapsed = SystemTime::now().duration_since(UNIX_EPOCH).ok()?;
    let millis = i64::try_from(elapsed.as_millis()).ok()?;
    Timestamp::from_unix_millis(millis).ok()
}
