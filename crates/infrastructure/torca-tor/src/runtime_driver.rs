//! In-process Tor lifecycle with bounded restart backoff.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TryRecvError};
use std::sync::{Arc, RwLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::{
    OnionServiceHealth, TorBootstrapEvent, TorBootstrapObserver, TorBootstrapStage, TorService,
    TorServiceHandle,
};
use torca_foundation::Timestamp;
use torca_runtime::{OnionServiceState, RuntimeDriverError, TorDriver, TorState};

const RESTART_BACKOFF: [Duration; 3] =
    [Duration::from_secs(5), Duration::from_secs(15), Duration::from_secs(30)];
const MAX_BOOTSTRAP_ATTEMPTS: u32 = 3;
// A publication attempt may wait on Arti internals, but must never make
// shutdown or a recovery cycle wait a minute. The durable worker retries.
const ONION_SERVICE_TIMEOUT: Duration = Duration::from_secs(8);
// Arti can self-heal a temporary directory/circuit loss.  Keep the published
// endpoint during this window; after it expires the durable publisher performs
// a controlled re-publication without restarting the Tor client.
// Descriptor uploads can remain degraded while Arti still has working
// introduction points. Give its internal retry loop enough time to recover;
// a client-side restart here would lose the service's progress.
const ONION_DEGRADED_GRACE: Duration = Duration::from_secs(300);
// A descriptor publication normally advances through several internal Arti
// states.  It must not be allowed to wait forever, however: a stalled
// publication leaves a client unable to accept peer sessions while the rest
// of the runtime looks healthy.  Recovery only replaces the onion service on
// the already bootstrapped Tor client.
// Descriptor publication is eventually consistent and can legitimately take
// several minutes on a cold Android/relay deployment.  A shorter deadline
// turns healthy Arti progress into a destructive republish loop: every retry
// throws away introduction-point work and starts from zero.  Keep the service
// alive for ten minutes before declaring it genuinely stalled.
const ONION_PUBLISHING_GRACE: Duration = Duration::from_secs(600);
const MAX_ONION_PUBLICATION_ATTEMPTS: u32 = 2;
// A recovery attempt must never occupy the application runtime owner.  The
// Tor service itself applies its own bounded retry policy; this is the bound
// for one attempt in that background lane.
const RESTART_BOOTSTRAP_TIMEOUT: Duration = Duration::from_secs(60);

enum OnionWorkerCommand {
    Shutdown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OnionRepublishReason {
    PublishingStalled,
    DegradedTimeout,
    Failed,
    Stopped,
    LaunchFailed,
    WorkerStopped,
}

impl OnionRepublishReason {
    const fn code(self) -> &'static str {
        match self {
            Self::PublishingStalled => "publishing_stalled",
            Self::DegradedTimeout => "degraded_timeout",
            Self::Failed => "failed",
            Self::Stopped => "stopped",
            Self::LaunchFailed => "launch_failed",
            Self::WorkerStopped => "worker_stopped",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OnionPublisherFailure {
    reason: OnionRepublishReason,
    attempts: u32,
}

enum OnionWaitOutcome {
    Shutdown,
    Republish { reason: OnionRepublishReason, was_reachable: bool },
}

#[derive(Default)]
struct OnionRecoveryTracker {
    degraded_since: Option<Instant>,
    publishing_since: Option<Instant>,
    publication_revision: Option<u64>,
    was_reachable: bool,
}

impl OnionRecoveryTracker {
    fn observe(
        &mut self,
        health: OnionServiceHealth,
        publication_revision: u64,
        now: Instant,
    ) -> Option<OnionRepublishReason> {
        match health {
            OnionServiceHealth::Reachable => {
                self.was_reachable = true;
                self.degraded_since = None;
                self.publishing_since = None;
                self.publication_revision = Some(publication_revision);
                None
            }
            OnionServiceHealth::Publishing => {
                self.degraded_since = None;
                if self.publication_revision != Some(publication_revision) {
                    self.publication_revision = Some(publication_revision);
                    self.publishing_since = Some(now);
                    return None;
                }
                let since = self.publishing_since.get_or_insert(now);
                (now.duration_since(*since) >= ONION_PUBLISHING_GRACE)
                    .then_some(OnionRepublishReason::PublishingStalled)
            }
            OnionServiceHealth::Degraded => {
                self.publishing_since = None;
                let since = self.degraded_since.get_or_insert(now);
                (now.duration_since(*since) >= ONION_DEGRADED_GRACE)
                    .then_some(OnionRepublishReason::DegradedTimeout)
            }
            OnionServiceHealth::Failed => Some(OnionRepublishReason::Failed),
            OnionServiceHealth::Stopped => Some(OnionRepublishReason::Stopped),
        }
    }
}

/// Owns only public endpoint publication. It deliberately does not own the
/// Tor client lifecycle, pairing, relay or peer maintenance.
struct OnionPublisher {
    commands: SyncSender<OnionWorkerCommand>,
    events: Receiver<OnionPublisherFailure>,
    worker: Option<JoinHandle<()>>,
}

/// A single asynchronous recovery attempt for the Tor client.  Initial warm-up
/// is deliberately sequential (the composition worker waits for Tor before it
/// composes Tor-dependent peer and relay adapters); subsequent recovery must
/// not be: doing Arti bootstrap from `TorDriver::maintenance` used to freeze
/// pairing, delivery and diagnostics for up to the bootstrap timeout.
struct TorBootstrapWorker {
    receiver: Receiver<Result<TorService, crate::TorError>>,
    worker: Option<JoinHandle<()>>,
}

impl TorBootstrapWorker {
    fn spawn(
        state_root: PathBuf,
        previous_client: Option<Arc<TorService>>,
    ) -> Result<Self, std::io::Error> {
        let (sender, receiver) = mpsc::sync_channel(1);
        let worker = thread::Builder::new().name("torca-tor-recovery".into()).spawn(move || {
            // The old runtime owns locks below the same Arti state root. Drop it
            // in this background lane before constructing the replacement so
            // maintenance and the ABI actor never block on runtime shutdown.
            drop(previous_client);
            let result = TorService::bootstrap(state_root, RESTART_BOOTSTRAP_TIMEOUT);
            let _ = sender.send(result);
        })?;
        Ok(Self { receiver, worker: Some(worker) })
    }

    fn try_take_result(&mut self) -> Option<Result<TorService, crate::TorError>> {
        match self.receiver.try_recv() {
            Ok(result) => {
                if let Some(worker) = self.worker.take() {
                    let _ = worker.join();
                }
                Some(result)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                Some(Err(crate::TorError("Tor recovery worker disconnected".into())))
            }
        }
    }
}

impl OnionPublisher {
    fn spawn(
        client: Arc<TorService>,
        target: SocketAddr,
        endpoint: SharedTorEndpoint,
        observer: Option<TorBootstrapObserver>,
    ) -> Result<Self, std::io::Error> {
        let (commands, receiver) = mpsc::sync_channel(1);
        let (event_sender, events) = mpsc::sync_channel(1);
        let worker =
            thread::Builder::new().name("torca-onion-publisher".into()).spawn(move || {
                let mut failures = 0_u32;
                loop {
                    let (reason, was_reachable) = match client
                        .publish_onion_service(target, ONION_SERVICE_TIMEOUT)
                    {
                        Ok(address) => {
                            endpoint.set(Some(address));
                            match wait_for_onion_recovery(&client, &receiver, observer.as_ref()) {
                                OnionWaitOutcome::Shutdown => {
                                    client.stop_onion_service();
                                    endpoint.set(None);
                                    return;
                                }
                                OnionWaitOutcome::Republish { reason, was_reachable } => {
                                    (reason, was_reachable)
                                }
                            }
                        }
                        Err(error) => {
                            eprintln!(
                                "torca-tor: onion publication launch failed: {error}"
                            );
                            (OnionRepublishReason::LaunchFailed, false)
                        }
                    };

                    if let Some(observer) = &observer {
                        observer(TorBootstrapEvent {
                            stage: TorBootstrapStage::OnionService,
                            progress: 8,
                            attempt: failures.saturating_add(1),
                            retry_after_ms: None,
                            code: "ONION_SERVICE_RETRYING",
                            summary: format!(
                                "Onion publication recovery scheduled after {}",
                                reason.code()
                            ),
                        });
                    }

                    if was_reachable {
                        failures = 0;
                    }
                    failures = failures.saturating_add(1);
                    endpoint.set(None);
                    client.stop_onion_service();

                    if failures >= MAX_ONION_PUBLICATION_ATTEMPTS {
                        client.mark_onion_publication_failed();
                        eprintln!(
                            "torca-tor: onion publication exhausted attempts={} reason={}; escalating to Tor recovery",
                            failures,
                            reason.code()
                        );
                        let _ = event_sender.send(OnionPublisherFailure {
                            reason,
                            attempts: failures,
                        });
                        return;
                    }

                    let index = usize::try_from(failures.saturating_sub(1))
                        .unwrap_or(usize::MAX)
                        .min(RESTART_BACKOFF.len() - 1);
                    let retry = RESTART_BACKOFF[index];
                    eprintln!(
                        "torca-tor: onion re-publication scheduled attempt={} reason={} retry_after_s={}",
                        failures.saturating_add(1),
                        reason.code(),
                        retry.as_secs()
                    );
                    match receiver.recv_timeout(retry) {
                        Ok(OnionWorkerCommand::Shutdown)
                        | Err(RecvTimeoutError::Disconnected) => return,
                        Err(RecvTimeoutError::Timeout) => {}
                    }
                }
            })?;
        Ok(Self { commands, events, worker: Some(worker) })
    }

    fn try_take_failure(&mut self) -> Option<OnionPublisherFailure> {
        let failure = match self.events.try_recv() {
            Ok(failure) => Some(failure),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected)
                if self.worker.as_ref().is_some_and(JoinHandle::is_finished) =>
            {
                Some(OnionPublisherFailure {
                    reason: OnionRepublishReason::WorkerStopped,
                    attempts: 0,
                })
            }
            Err(TryRecvError::Disconnected) => None,
        }?;
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        Some(failure)
    }

    fn shutdown(mut self) {
        let _ = self.commands.send(OnionWorkerCommand::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

/// Waits after a successful publication instead of treating that success as
/// the end of the publisher lifecycle. Returns `true` when a controlled
/// republish is needed and `false` for shutdown.
fn wait_for_onion_recovery(
    client: &TorService,
    receiver: &Receiver<OnionWorkerCommand>,
    observer: Option<&TorBootstrapObserver>,
) -> OnionWaitOutcome {
    let mut tracker = OnionRecoveryTracker::default();
    let mut last_health = None;
    loop {
        match receiver.recv_timeout(Duration::from_secs(1)) {
            Ok(OnionWorkerCommand::Shutdown) | Err(RecvTimeoutError::Disconnected) => {
                return OnionWaitOutcome::Shutdown;
            }
            Err(RecvTimeoutError::Timeout) => {}
        }
        let health = client.onion_service_health();
        if last_health != Some(health) {
            if let Some(observer) = observer {
                let (progress, code, summary) = match health {
                    OnionServiceHealth::Reachable => {
                        (100, "ONION_SERVICE_READY", "Private onion service is reachable")
                    }
                    OnionServiceHealth::Degraded => (
                        60,
                        "ONION_SERVICE_DEGRADED",
                        "Onion service is reachable with degraded publication",
                    ),
                    OnionServiceHealth::Publishing => {
                        (8, "ONION_SERVICE_PUBLISHING", "Publishing private onion service")
                    }
                    OnionServiceHealth::Failed => {
                        (0, "ONION_SERVICE_FAILED", "Onion service publication failed")
                    }
                    OnionServiceHealth::Stopped => {
                        (0, "ONION_SERVICE_STOPPED", "Onion service stopped")
                    }
                };
                observer(TorBootstrapEvent {
                    stage: TorBootstrapStage::OnionService,
                    progress,
                    attempt: 1,
                    retry_after_ms: None,
                    code,
                    summary: summary.into(),
                });
            }
            last_health = Some(health);
        }
        if let Some(reason) =
            tracker.observe(health, client.onion_publication_revision(), Instant::now())
        {
            eprintln!(
                "torca-tor: onion publication requires recovery reason={} was_reachable={}",
                reason.code(),
                tracker.was_reachable
            );
            return OnionWaitOutcome::Republish { reason, was_reachable: tracker.was_reachable };
        }
    }
}

#[derive(Clone, Default)]
pub struct SharedTorEndpoint {
    inner: Arc<RwLock<Option<String>>>,
}
impl SharedTorEndpoint {
    pub fn get(&self) -> Option<String> {
        self.inner.read().ok().and_then(|value| value.clone())
    }

    fn set(&self, value: Option<String>) {
        if let Ok(mut endpoint) = self.inner.write() {
            *endpoint = value;
        }
    }
}

pub struct OwnedTorDriver {
    state_root: PathBuf,
    peer_target: SocketAddr,
    client: Option<TorServiceHandle>,
    endpoint: SharedTorEndpoint,
    onion_publisher: Option<OnionPublisher>,
    bootstrap_worker: Option<TorBootstrapWorker>,
    startup_timeout: Duration,
    failures: u32,
    next_restart_at: Option<Timestamp>,
    state: TorState,
    last_diagnostic: Option<String>,
    observer: Option<TorBootstrapObserver>,
}

impl OwnedTorDriver {
    pub fn bootstrap(
        state_root: impl Into<PathBuf>,
        peer_target: SocketAddr,
        endpoint: SharedTorEndpoint,
        startup_timeout: Duration,
        now: Timestamp,
    ) -> Result<Self, RuntimeDriverError> {
        Self::bootstrap_with_diagnostic(state_root, peer_target, endpoint, startup_timeout, now)
            .map_err(|(error, _)| error)
    }

    /// Starts the shared in-process Tor runtime and preserves a diagnostic on failure.
    pub fn bootstrap_with_diagnostic(
        state_root: impl Into<PathBuf>,
        peer_target: SocketAddr,
        endpoint: SharedTorEndpoint,
        startup_timeout: Duration,
        now: Timestamp,
    ) -> Result<Self, (RuntimeDriverError, String)> {
        Self::bootstrap_observed(state_root, peer_target, endpoint, startup_timeout, now, None)
    }

    pub fn bootstrap_observed(
        state_root: impl Into<PathBuf>,
        peer_target: SocketAddr,
        endpoint: SharedTorEndpoint,
        startup_timeout: Duration,
        now: Timestamp,
        observer: Option<TorBootstrapObserver>,
    ) -> Result<Self, (RuntimeDriverError, String)> {
        let mut driver = Self {
            state_root: state_root.into(),
            peer_target,
            client: None,
            endpoint,
            onion_publisher: None,
            bootstrap_worker: None,
            startup_timeout,
            failures: 0,
            next_restart_at: None,
            state: TorState::Starting,
            last_diagnostic: None,
            observer: observer.clone(),
        };
        if let Err(error) = driver.start(peer_target, now, observer) {
            let diagnostic = driver
                .last_diagnostic
                .clone()
                .unwrap_or_else(|| "unknown Tor startup failure".to_owned());
            return Err((error, diagnostic));
        }
        Ok(driver)
    }

    fn start(
        &mut self,
        _peer_target: SocketAddr,
        now: Timestamp,
        observer: Option<TorBootstrapObserver>,
    ) -> Result<(), RuntimeDriverError> {
        self.observer.clone_from(&observer);
        self.endpoint.set(None);
        self.state = TorState::Starting;
        match TorService::bootstrap_observed(
            &self.state_root,
            self.startup_timeout,
            observer.clone(),
        ) {
            Ok(client) => {
                let client = Arc::new(client);
                self.client = Some(TorServiceHandle::new(Arc::clone(&client)));
                self.failures = 0;
                self.next_restart_at = None;
                self.state = TorState::Ready;
                self.last_diagnostic = None;
                notify_onion(&observer, 1, 5, None, "ONION_SERVICE_PUBLISHING");
                self.start_onion_publisher(client, observer)?;
                Ok(())
            }
            Err(error) => {
                self.last_diagnostic = Some(format!("Arti bootstrap failed: {error}"));
                self.client = None;
                self.state = TorState::Degraded;
                self.schedule_restart(now)?;
                Err(RuntimeDriverError::Tor)
            }
        }
    }

    fn schedule_restart(&mut self, now: Timestamp) -> Result<(), RuntimeDriverError> {
        self.failures = self.failures.saturating_add(1);
        if self.failures >= MAX_BOOTSTRAP_ATTEMPTS {
            self.next_restart_at = None;
            self.state = TorState::Failed;
            return Ok(());
        }
        let index = usize::try_from(self.failures.saturating_sub(1))
            .unwrap_or(usize::MAX)
            .min(RESTART_BACKOFF.len() - 1);
        self.next_restart_at = now.checked_add(RESTART_BACKOFF[index]);
        if self.next_restart_at.is_none() {
            return Err(RuntimeDriverError::Tor);
        }
        Ok(())
    }

    fn detect_process_state(&mut self, _now: Timestamp) -> Result<(), RuntimeDriverError> {
        if self.client.as_ref().is_some_and(|client| client.current().is_ok()) {
            self.state = TorState::Ready;
        }
        Ok(())
    }

    fn start_onion_publisher(
        &mut self,
        client: Arc<TorService>,
        observer: Option<TorBootstrapObserver>,
    ) -> Result<(), RuntimeDriverError> {
        if let Some(worker) = self.onion_publisher.take() {
            worker.shutdown();
        }
        self.endpoint.set(None);
        self.onion_publisher = Some(
            OnionPublisher::spawn(client, self.peer_target, self.endpoint.clone(), observer)
                .map_err(|_| RuntimeDriverError::Tor)?,
        );
        Ok(())
    }

    fn start_recovery(&mut self) -> Result<(), RuntimeDriverError> {
        if let Some(worker) = self.onion_publisher.take() {
            worker.shutdown();
        }
        self.endpoint.set(None);
        self.state = TorState::Starting;
        self.next_restart_at = None;
        let previous_client = self.client.as_ref().and_then(TorServiceHandle::clear);
        self.bootstrap_worker = Some(
            TorBootstrapWorker::spawn(self.state_root.clone(), previous_client)
                .map_err(|_| RuntimeDriverError::Tor)?,
        );
        Ok(())
    }

    fn reap_onion_publisher(&mut self) -> Result<(), RuntimeDriverError> {
        let Some(failure) =
            self.onion_publisher.as_mut().and_then(OnionPublisher::try_take_failure)
        else {
            return Ok(());
        };
        self.onion_publisher = None;
        self.last_diagnostic = Some(format!(
            "onion publication exhausted attempts={} reason={}",
            failure.attempts,
            failure.reason.code()
        ));
        self.state = TorState::Degraded;
        self.start_recovery()
    }

    fn reap_recovery(&mut self, now: Timestamp) -> Result<(), RuntimeDriverError> {
        let Some(worker) = self.bootstrap_worker.as_mut() else {
            return Ok(());
        };
        let Some(result) = worker.try_take_result() else {
            return Ok(());
        };
        self.bootstrap_worker = None;
        match result {
            Ok(client) => {
                let client = Arc::new(client);
                if let Some(handle) = self.client.as_ref() {
                    let previous = handle.replace(Arc::clone(&client));
                    drop(previous);
                } else {
                    self.client = Some(TorServiceHandle::new(Arc::clone(&client)));
                }
                self.failures = 0;
                self.next_restart_at = None;
                self.state = TorState::Ready;
                self.last_diagnostic = None;
                self.start_onion_publisher(client, self.observer.clone())
            }
            Err(error) => {
                self.last_diagnostic = Some(format!("Arti recovery bootstrap failed: {error}"));
                self.state = TorState::Degraded;
                self.schedule_restart(now)
            }
        }
    }

    /// Returns a shared handle to the in-process Tor client.
    pub fn client_handle(&self) -> Option<TorServiceHandle> {
        self.client.clone()
    }
}

fn notify_onion(
    observer: &Option<TorBootstrapObserver>,
    attempt: u32,
    progress: u8,
    retry_after_ms: Option<u64>,
    code: &'static str,
) {
    if let Some(observer) = observer {
        observer(TorBootstrapEvent {
            stage: TorBootstrapStage::OnionService,
            progress,
            attempt,
            retry_after_ms,
            code,
            summary: match code {
                "ONION_SERVICE_READY" => "Private onion service is reachable",
                "ONION_SERVICE_PUBLISHING" => "Publishing private onion service",
                "ONION_SERVICE_RETRYING" => "Onion publication retry scheduled",
                _ => "Publishing private onion service",
            }
            .into(),
        });
    }
}

impl TorDriver for OwnedTorDriver {
    fn maintenance(&mut self, now: Timestamp) -> Result<(), RuntimeDriverError> {
        self.detect_process_state(now)?;
        self.reap_recovery(now)?;
        self.reap_onion_publisher()?;
        if self.client.as_ref().is_none_or(|client| client.current().is_err())
            && self.bootstrap_worker.is_none()
            && self.next_restart_at.is_some_and(|deadline| deadline <= now)
        {
            if let Err(error) = self.start_recovery() {
                self.state = TorState::Degraded;
                self.schedule_restart(now)?;
                return Err(error);
            }
        }
        Ok(())
    }

    fn next_maintenance_delay(&self, _now: Timestamp) -> Option<Duration> {
        // During bootstrap, publication and recovery we still need to reap
        // worker results and advance bounded retry deadlines. The publisher's
        // completion channel still uses the one-second fallback until it gets
        // a direct runtime wake callback; peer/radio listeners are event-driven.
        if self.bootstrap_worker.is_some()
            || self.next_restart_at.is_some()
            // The publisher currently reports exhaustion through its
            // maintenance-owned channel. Keep this bounded fallback until
            // that worker receives a direct wake callback.
            || self.onion_publisher.is_some()
        {
            return Some(Duration::from_secs(1));
        }
        match self.onion_service_state() {
            OnionServiceState::Reachable => None,
            OnionServiceState::Unknown
            | OnionServiceState::Publishing
            | OnionServiceState::Degraded
            | OnionServiceState::Failed
            | OnionServiceState::Stopped => Some(Duration::from_secs(1)),
        }
    }

    fn state(&self) -> TorState {
        self.state
    }

    fn onion_address(&self) -> Option<String> {
        self.endpoint.get()
    }

    fn onion_service_state(&self) -> OnionServiceState {
        let Some(handle) = self.client.as_ref() else {
            return if self.state == TorState::Stopped {
                OnionServiceState::Stopped
            } else {
                OnionServiceState::Unknown
            };
        };
        let Ok(client) = handle.current() else {
            return OnionServiceState::Unknown;
        };
        match client.onion_service_health() {
            OnionServiceHealth::Stopped => OnionServiceState::Stopped,
            OnionServiceHealth::Publishing => OnionServiceState::Publishing,
            OnionServiceHealth::Reachable => OnionServiceState::Reachable,
            OnionServiceHealth::Degraded => OnionServiceState::Degraded,
            OnionServiceHealth::Failed => OnionServiceState::Failed,
        }
    }

    fn shutdown(&mut self) {
        self.next_restart_at = None;
        self.endpoint.set(None);
        // Do not join an in-flight Arti bootstrap here. It is externally
        // bounded but may still be waiting on platform I/O; blocking shutdown
        // would reintroduce the UI freeze this worker removes.
        self.bootstrap_worker.take();
        if let Some(worker) = self.onion_publisher.take() {
            worker.shutdown();
        }
        if let Some(client) = self.client.take() {
            let previous = client.clear();
            drop(previous);
        }
        self.state = TorState::Stopped;
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ONION_DEGRADED_GRACE, ONION_PUBLISHING_GRACE, OnionRecoveryTracker, OnionRepublishReason,
    };
    use crate::OnionServiceHealth;
    use std::time::{Duration, Instant};

    #[test]
    fn publishing_without_a_new_revision_requests_controlled_republication() {
        let start = Instant::now();
        let mut tracker = OnionRecoveryTracker::default();
        assert_eq!(tracker.observe(OnionServiceHealth::Publishing, 1, start), None);
        assert_eq!(
            tracker.observe(
                OnionServiceHealth::Publishing,
                1,
                (start + ONION_PUBLISHING_GRACE)
                    .checked_sub(Duration::from_secs(1))
                    .expect("grace period is longer than one second")
            ),
            None
        );
        assert_eq!(
            tracker.observe(OnionServiceHealth::Publishing, 1, start + ONION_PUBLISHING_GRACE),
            Some(OnionRepublishReason::PublishingStalled)
        );
    }

    #[test]
    fn a_new_publication_revision_restarts_the_publishing_deadline() {
        let start = Instant::now();
        let mut tracker = OnionRecoveryTracker::default();
        assert_eq!(tracker.observe(OnionServiceHealth::Publishing, 1, start), None);
        assert_eq!(
            tracker.observe(
                OnionServiceHealth::Publishing,
                2,
                (start + ONION_PUBLISHING_GRACE)
                    .checked_sub(Duration::from_secs(1))
                    .expect("grace period is longer than one second")
            ),
            None
        );
        assert_eq!(
            tracker.observe(
                OnionServiceHealth::Publishing,
                2,
                (start + ONION_PUBLISHING_GRACE * 2)
                    .checked_sub(Duration::from_secs(2))
                    .expect("grace period is longer than two seconds")
            ),
            None
        );
    }

    #[test]
    fn reachable_resets_the_degraded_deadline() {
        let start = Instant::now();
        let mut tracker = OnionRecoveryTracker::default();
        assert_eq!(tracker.observe(OnionServiceHealth::Degraded, 1, start), None);
        assert_eq!(
            tracker.observe(OnionServiceHealth::Reachable, 2, start + Duration::from_secs(30)),
            None
        );
        assert!(tracker.was_reachable);
        let degrading_again = start + Duration::from_secs(40);
        assert_eq!(tracker.observe(OnionServiceHealth::Degraded, 3, degrading_again), None);
        assert_eq!(
            tracker.observe(
                OnionServiceHealth::Degraded,
                3,
                degrading_again + ONION_DEGRADED_GRACE
            ),
            Some(OnionRepublishReason::DegradedTimeout)
        );
    }

    #[test]
    fn degraded_and_terminal_states_request_recovery() {
        let start = Instant::now();
        let mut tracker = OnionRecoveryTracker::default();
        assert_eq!(tracker.observe(OnionServiceHealth::Degraded, 1, start), None);
        assert_eq!(
            tracker.observe(OnionServiceHealth::Degraded, 1, start + ONION_DEGRADED_GRACE),
            Some(OnionRepublishReason::DegradedTimeout)
        );
        assert_eq!(
            tracker.observe(OnionServiceHealth::Failed, 1, start),
            Some(OnionRepublishReason::Failed)
        );
        assert_eq!(
            tracker.observe(OnionServiceHealth::Stopped, 1, start),
            Some(OnionRepublishReason::Stopped)
        );
    }
}
