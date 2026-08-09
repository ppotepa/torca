//! In-process Tor lifecycle with bounded restart backoff.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use crate::{RuntimeDriverError, TorDriver, TorState};
use torca_foundation::Timestamp;
use torca_tor::TorService;

const RESTART_BACKOFF: [Duration; 3] =
    [Duration::from_secs(5), Duration::from_secs(15), Duration::from_secs(30)];
const MAX_BOOTSTRAP_ATTEMPTS: u32 = 3;
const ONION_SERVICE_TIMEOUT: Duration = Duration::from_secs(60);

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
    client: Option<Arc<TorService>>,
    endpoint: SharedTorEndpoint,
    startup_timeout: Duration,
    failures: u32,
    next_restart_at: Option<Timestamp>,
    state: TorState,
    last_diagnostic: Option<String>,
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
        let mut driver = Self {
            state_root: state_root.into(),
            peer_target,
            client: None,
            endpoint,
            startup_timeout,
            failures: 0,
            next_restart_at: None,
            state: TorState::Starting,
            last_diagnostic: None,
        };
        if let Err(error) = driver.start(peer_target, now) {
            let diagnostic = driver
                .last_diagnostic
                .clone()
                .unwrap_or_else(|| "unknown Tor startup failure".to_owned());
            return Err((error, diagnostic));
        }
        Ok(driver)
    }

    fn start(&mut self, peer_target: SocketAddr, now: Timestamp) -> Result<(), RuntimeDriverError> {
        self.endpoint.set(None);
        self.state = TorState::Starting;
        match TorService::bootstrap(&self.state_root, self.startup_timeout) {
            Ok(mut client) => {
                match client.publish_onion_service(peer_target, ONION_SERVICE_TIMEOUT) {
                    Ok(address) => {
                        self.endpoint.set(Some(address));
                        self.client = Some(Arc::new(client));
                        self.failures = 0;
                        self.next_restart_at = None;
                        self.state = TorState::Ready;
                        self.last_diagnostic = None;
                        Ok(())
                    }
                    Err(error) => {
                        self.last_diagnostic = Some(format!("Arti onion service failed: {error}"));
                        self.client = None;
                        self.state = TorState::Degraded;
                        self.schedule_restart(now)?;
                        Err(RuntimeDriverError::Tor)
                    }
                }
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
        if self.client.is_some() {
            self.state = TorState::Ready;
        }
        Ok(())
    }

    /// Returns a shared handle to the in-process Tor client.
    pub fn client_handle(&self) -> Option<Arc<TorService>> {
        self.client.clone()
    }
}

impl TorDriver for OwnedTorDriver {
    fn maintenance(&mut self, now: Timestamp) -> Result<(), RuntimeDriverError> {
        self.detect_process_state(now)?;
        if self.client.is_none() && self.next_restart_at.is_some_and(|deadline| deadline <= now) {
            let _ = self.start(self.peer_target, now);
        }
        Ok(())
    }

    fn state(&self) -> TorState {
        self.state
    }

    fn onion_address(&self) -> Option<String> {
        self.endpoint.get()
    }

    fn shutdown(&mut self) {
        self.next_restart_at = None;
        self.endpoint.set(None);
        self.client = None;
        self.state = TorState::Stopped;
    }
}
