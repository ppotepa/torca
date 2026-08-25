//! In-process Tor lifecycle with bounded restart backoff.

mod bootstrap_worker;
mod endpoint;
mod onion_publisher;
mod recovery_epoch;
mod timing;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use bootstrap_worker::TorBootstrapWorker;
pub use endpoint::SharedTorEndpoint;
use onion_publisher::OnionPublisher;
use recovery_epoch::RecoveryEpoch;
use timing::{MAX_BOOTSTRAP_ATTEMPTS, RESTART_BACKOFF};

use crate::{
    OnionServiceHealth, TorActivityMode, TorBootstrapEvent, TorBootstrapObserver,
    TorBootstrapStage, TorService, TorServiceHandle,
};
use torca_foundation::{Timestamp, WakeSlot};
use torca_runtime::{
    CommunicationLifecycle, CommunicationState, IncomingReachabilityState, RuntimeDriverError,
};

type TorWake = Arc<WakeSlot>;

fn notify_tor_wake(wake: &TorWake) {
    let _ = wake.wake();
}

pub struct OwnedTorDriver {
    state_root: PathBuf,
    peer_target: SocketAddr,
    client: Option<TorServiceHandle>,
    endpoint: SharedTorEndpoint,
    onion_publisher: Option<OnionPublisher>,
    bootstrap_worker: Option<TorBootstrapWorker>,
    recovery_epoch: RecoveryEpoch,
    startup_timeout: Duration,
    failures: u32,
    next_restart_at: Option<Timestamp>,
    state: CommunicationState,
    last_diagnostic: Option<String>,
    observer: Option<TorBootstrapObserver>,
    wake: TorWake,
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
            recovery_epoch: RecoveryEpoch::default(),
            startup_timeout,
            failures: 0,
            next_restart_at: None,
            state: CommunicationState::Starting,
            last_diagnostic: None,
            observer: observer.clone(),
            wake: Arc::new(WakeSlot::default()),
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
        self.state = CommunicationState::Starting;
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
                self.state = CommunicationState::Ready;
                self.last_diagnostic = None;
                notify_onion(&observer, 1, 5, None, "ONION_SERVICE_PUBLISHING");
                self.start_onion_publisher(client, observer)?;
                Ok(())
            }
            Err(error) => {
                self.last_diagnostic = Some(format!("Arti bootstrap failed: {error}"));
                self.client = None;
                self.state = CommunicationState::Degraded;
                self.schedule_restart(now)?;
                Err(RuntimeDriverError::Communication)
            }
        }
    }

    fn schedule_restart(&mut self, now: Timestamp) -> Result<(), RuntimeDriverError> {
        self.failures = self.failures.saturating_add(1);
        if self.failures >= MAX_BOOTSTRAP_ATTEMPTS {
            self.next_restart_at = None;
            self.state = CommunicationState::Failed;
            return Ok(());
        }
        let index = usize::try_from(self.failures.saturating_sub(1))
            .unwrap_or(usize::MAX)
            .min(RESTART_BACKOFF.len() - 1);
        self.next_restart_at = now.checked_add(RESTART_BACKOFF[index]);
        if self.next_restart_at.is_none() {
            return Err(RuntimeDriverError::Communication);
        }
        Ok(())
    }

    fn detect_process_state(&mut self, _now: Timestamp) -> Result<(), RuntimeDriverError> {
        if self.client.as_ref().is_some_and(|client| client.current().is_ok()) {
            self.state = CommunicationState::Ready;
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
            OnionPublisher::spawn(
                client,
                self.peer_target,
                self.endpoint.clone(),
                observer,
                Arc::clone(&self.wake),
            )
            .map_err(|_| RuntimeDriverError::Communication)?,
        );
        Ok(())
    }

    fn start_recovery(&mut self) -> Result<(), RuntimeDriverError> {
        if let Some(worker) = self.onion_publisher.take() {
            worker.shutdown();
        }
        self.endpoint.set(None);
        self.state = CommunicationState::Starting;
        self.next_restart_at = None;
        let previous_client = self.client.as_ref().and_then(TorServiceHandle::clear);
        let epoch = self.recovery_epoch.advance();
        self.bootstrap_worker = Some(
            TorBootstrapWorker::spawn(
                epoch,
                self.state_root.clone(),
                previous_client,
                Arc::clone(&self.wake),
            )
            .map_err(|_| RuntimeDriverError::Communication)?,
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
        self.state = CommunicationState::Degraded;
        self.start_recovery()
    }

    fn reap_recovery(&mut self, now: Timestamp) -> Result<(), RuntimeDriverError> {
        let Some(worker) = self.bootstrap_worker.as_mut() else {
            return Ok(());
        };
        let Some((epoch, result)) = worker.try_take_result() else {
            return Ok(());
        };
        self.bootstrap_worker = None;
        if !self.recovery_epoch.matches(epoch) {
            drop(result);
            return Ok(());
        }
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
                self.state = CommunicationState::Ready;
                self.last_diagnostic = None;
                self.start_onion_publisher(client, self.observer.clone())
            }
            Err(error) => {
                self.last_diagnostic = Some(format!("Arti recovery bootstrap failed: {error}"));
                self.state = CommunicationState::Degraded;
                self.schedule_restart(now)
            }
        }
    }

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

impl CommunicationLifecycle for OwnedTorDriver {
    fn provider(&self) -> torca_transport_api::TransportKind {
        torca_transport_api::TransportKind::Tor
    }

    fn maintenance(&mut self, now: Timestamp) -> Result<(), RuntimeDriverError> {
        self.detect_process_state(now)?;
        self.reap_recovery(now)?;
        self.reap_onion_publisher()?;
        if self.client.as_ref().is_none_or(|client| client.current().is_err())
            && self.bootstrap_worker.is_none()
            && self.next_restart_at.is_some_and(|deadline| deadline <= now)
        {
            if let Err(error) = self.start_recovery() {
                self.state = CommunicationState::Degraded;
                self.schedule_restart(now)?;
                return Err(error);
            }
        }
        Ok(())
    }

    fn next_maintenance_delay(&self, _now: Timestamp) -> Option<Duration> {
        if self.bootstrap_worker.is_some() || self.next_restart_at.is_some() {
            return Some(Duration::from_secs(1));
        }
        if self.onion_publisher.is_some() {
            return None;
        }
        match self.incoming_reachability_state() {
            IncomingReachabilityState::Reachable => None,
            IncomingReachabilityState::Unknown
            | IncomingReachabilityState::Publishing
            | IncomingReachabilityState::Degraded
            | IncomingReachabilityState::Failed
            | IncomingReachabilityState::Stopped => Some(Duration::from_secs(1)),
        }
    }

    fn set_waker(&mut self, waker: Arc<dyn Fn() + Send + Sync>) {
        let _ = self.wake.set(waker);
    }

    fn set_dormant(&mut self, dormant: bool) -> Result<(), RuntimeDriverError> {
        let Some(client) = self.client.as_ref() else {
            return Ok(());
        };
        client
            .set_activity_mode(if dormant {
                TorActivityMode::SoftDormant
            } else {
                TorActivityMode::Active
            })
            .map_err(|_| RuntimeDriverError::Communication)
    }

    fn state(&self) -> CommunicationState {
        self.state
    }

    fn local_endpoint_summary(&self) -> Option<String> {
        self.endpoint.get()
    }

    fn incoming_reachability_state(&self) -> IncomingReachabilityState {
        let Some(handle) = self.client.as_ref() else {
            return if self.state == CommunicationState::Stopped {
                IncomingReachabilityState::Stopped
            } else {
                IncomingReachabilityState::Unknown
            };
        };
        let Ok(client) = handle.current() else {
            return IncomingReachabilityState::Unknown;
        };
        match client.onion_service_health() {
            OnionServiceHealth::Stopped => IncomingReachabilityState::Stopped,
            OnionServiceHealth::Publishing => IncomingReachabilityState::Publishing,
            OnionServiceHealth::Reachable => IncomingReachabilityState::Reachable,
            OnionServiceHealth::Degraded => IncomingReachabilityState::Degraded,
            OnionServiceHealth::Failed => IncomingReachabilityState::Failed,
        }
    }

    fn shutdown(&mut self) {
        self.next_restart_at = None;
        self.endpoint.set(None);
        self.recovery_epoch.advance();
        let _ = self.wake.clear();
        self.bootstrap_worker.take();
        if let Some(worker) = self.onion_publisher.take() {
            worker.shutdown();
        }
        if let Some(client) = self.client.take() {
            let previous = client.clear();
            drop(previous);
        }
        self.state = CommunicationState::Stopped;
    }
}
