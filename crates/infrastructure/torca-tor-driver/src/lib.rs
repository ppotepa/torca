//! Process-owned Tor lifecycle with bounded restart backoff.

use std::sync::{Arc, RwLock};
use std::time::Duration;

use torca_crypto::{CryptoProvider, RustCryptoProvider};
use torca_foundation::Timestamp;
use torca_runtime_host::{HostTorState, RuntimeDriverError, TorDriver};
use torca_transport_tor::{TorProcess, TorProcessConfig, TorState};

const RESTART_BASE_MS: u64 = 1_000;
const RESTART_MAX_MS: u64 = 60_000;

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
    config: TorProcessConfig,
    process: Option<TorProcess>,
    endpoint: SharedTorEndpoint,
    startup_timeout: Duration,
    failures: u32,
    next_restart_at: Option<Timestamp>,
    state: HostTorState,
    random: RustCryptoProvider,
}

impl OwnedTorDriver {
    pub fn bootstrap(
        config: TorProcessConfig,
        endpoint: SharedTorEndpoint,
        startup_timeout: Duration,
        now: Timestamp,
    ) -> Result<Self, RuntimeDriverError> {
        let mut driver = Self {
            config,
            process: None,
            endpoint,
            startup_timeout,
            failures: 0,
            next_restart_at: None,
            state: HostTorState::Starting,
            random: RustCryptoProvider,
        };
        driver.start(now)?;
        Ok(driver)
    }

    fn start(&mut self, now: Timestamp) -> Result<(), RuntimeDriverError> {
        self.endpoint.set(None);
        self.state = HostTorState::Starting;
        let mut process = TorProcess::new(self.config.clone());
        if process.start().is_err() || process.wait_until_ready(self.startup_timeout).is_err() {
            let _ = process.stop();
            self.process = None;
            self.state = HostTorState::Degraded;
            self.schedule_restart(now)?;
            return Err(RuntimeDriverError::Tor);
        }
        let onion = process
            .onion_hostname()
            .map_err(|_| RuntimeDriverError::Tor)?
            .ok_or(RuntimeDriverError::Tor)?;
        self.endpoint.set(Some(onion));
        self.failures = 0;
        self.next_restart_at = None;
        self.state = HostTorState::Ready;
        self.process = Some(process);
        Ok(())
    }

    fn schedule_restart(&mut self, now: Timestamp) -> Result<(), RuntimeDriverError> {
        self.failures = self.failures.saturating_add(1);
        let exponent = self.failures.saturating_sub(1).min(16);
        let base = RESTART_BASE_MS
            .saturating_mul(1_u64 << exponent)
            .min(RESTART_MAX_MS);
        let jitter_room = (base / 4).min(RESTART_MAX_MS.saturating_sub(base));
        let jitter = if jitter_room == 0 {
            0
        } else {
            let mut bytes = [0_u8; 8];
            self.random
                .fill_random(&mut bytes)
                .map_err(|_| RuntimeDriverError::Tor)?;
            u64::from_le_bytes(bytes) % (jitter_room + 1)
        };
        self.next_restart_at = now.checked_add(Duration::from_millis(base + jitter));
        if self.next_restart_at.is_none() {
            return Err(RuntimeDriverError::Tor);
        }
        Ok(())
    }

    fn detect_process_state(&mut self, now: Timestamp) -> Result<(), RuntimeDriverError> {
        let Some(process) = self.process.as_mut() else {
            return Ok(());
        };
        match process.refresh_state().map_err(|_| RuntimeDriverError::Tor)? {
            TorState::Ready => {
                self.state = HostTorState::Ready;
            }
            TorState::Starting => self.state = HostTorState::Starting,
            TorState::Degraded => self.state = HostTorState::Degraded,
            TorState::Failed | TorState::Stopped => {
                self.endpoint.set(None);
                self.process = None;
                self.state = HostTorState::Failed;
                self.schedule_restart(now)?;
            }
            TorState::Stopping => self.state = HostTorState::Stopped,
        }
        Ok(())
    }
}

impl TorDriver for OwnedTorDriver {
    fn maintenance(&mut self, now: Timestamp) -> Result<(), RuntimeDriverError> {
        self.detect_process_state(now)?;
        if self.process.is_none()
            && self.next_restart_at.is_some_and(|deadline| deadline <= now)
        {
            let _ = self.start(now);
        }
        Ok(())
    }

    fn state(&self) -> HostTorState {
        self.state
    }

    fn onion_address(&self) -> Option<String> {
        self.endpoint.get()
    }

    fn shutdown(&mut self) {
        self.next_restart_at = None;
        self.endpoint.set(None);
        if let Some(mut process) = self.process.take() {
            let _ = process.stop();
        }
        self.state = HostTorState::Stopped;
    }
}
