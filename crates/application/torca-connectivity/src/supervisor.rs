//! Long-lived, payload-free connectivity workers.
//!
//! A worker owns one probe lane for its complete lifetime.  This prevents the
//! previous "spawn a thread per probe" pattern from overlapping abandoned Tor
//! dials after a timeout or network change.

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender};
use std::sync::{Arc, RwLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use torca_foundation::ErrorCode;
use torca_probing::ProbeStatus;

const HEALTHY_INTERVAL: Duration = Duration::from_secs(15);
const RETRY_BACKOFF: [Duration; 4] = [
    Duration::from_secs(5),
    Duration::from_secs(15),
    Duration::from_secs(30),
    Duration::from_secs(60),
];

/// Infrastructure implements this narrow port.  It must apply a bounded
/// deadline to the complete request and return a stable, redacted error code.
pub trait RelayHealthPort: Send + Sync + 'static {
    fn check_relay_health(&self) -> Result<(), ErrorCode>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelayHealthSnapshot {
    pub status: ProbeStatus,
    pub diagnostic_code: ErrorCode,
    pub latency_ms: Option<u64>,
    pub failures: u32,
}

impl Default for RelayHealthSnapshot {
    fn default() -> Self {
        Self {
            status: ProbeStatus::Unknown,
            diagnostic_code: ErrorCode::new("relay.probe_unconfigured"),
            latency_ms: None,
            failures: 0,
        }
    }
}

enum Command {
    Wake,
    Shutdown,
}

/// Cloneable read/control handle; it does not own the worker join handle.
#[derive(Clone)]
pub struct RelayHealthHandle {
    state: Arc<RwLock<RelayHealthSnapshot>>,
    commands: SyncSender<Command>,
}

impl RelayHealthHandle {
    pub fn snapshot(&self) -> RelayHealthSnapshot {
        self.state.read().map_or_else(
            |_| RelayHealthSnapshot {
                status: ProbeStatus::Failed,
                diagnostic_code: ErrorCode::new("relay.supervisor_poisoned"),
                latency_ms: None,
                failures: 0,
            },
            |value| value.clone(),
        )
    }

    /// Coalesces repeated Android/network callbacks into one immediate next
    /// probe. A running check remains the only in-flight request.
    pub fn network_changed(&self) {
        let _ = self.commands.try_send(Command::Wake);
    }
}

/// Application-owned relay supervisor.  The worker is intentionally one
/// durable thread instead of a fresh thread for every probe attempt.
pub struct RelayHealthWorker {
    handle: RelayHealthHandle,
    worker: Option<JoinHandle<()>>,
}

impl RelayHealthWorker {
    pub fn spawn(port: Arc<dyn RelayHealthPort>) -> Result<Self, std::io::Error> {
        let state = Arc::new(RwLock::new(RelayHealthSnapshot {
            status: ProbeStatus::Checking,
            diagnostic_code: ErrorCode::new("relay.probe_starting"),
            latency_ms: None,
            failures: 0,
        }));
        let (commands, receiver) = mpsc::sync_channel(1);
        let handle = RelayHealthHandle { state: Arc::clone(&state), commands };
        let worker = thread::Builder::new()
            .name("torca-relay-health".into())
            .spawn(move || run(port, state, receiver))?;
        Ok(Self { handle, worker: Some(worker) })
    }

    pub fn handle(&self) -> RelayHealthHandle {
        self.handle.clone()
    }

    pub fn shutdown(mut self) {
        let _ = self.handle.commands.send(Command::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn run(
    port: Arc<dyn RelayHealthPort>,
    state: Arc<RwLock<RelayHealthSnapshot>>,
    receiver: Receiver<Command>,
) {
    let mut failures = 0_u32;
    let mut next_at = Instant::now();
    loop {
        let now = Instant::now();
        if now < next_at {
            match receiver.recv_timeout(next_at.duration_since(now)) {
                Ok(Command::Shutdown) | Err(RecvTimeoutError::Disconnected) => break,
                Ok(Command::Wake) => {
                    next_at = Instant::now();
                    continue;
                }
                Err(RecvTimeoutError::Timeout) => {}
            }
        }
        // Keep the last usable status while a probe is in flight. Projecting
        // `Checking` on every 15-second refresh made the UI show a false
        // disconnect/reconnect flicker and caused callers to treat a healthy
        // persistent stream as unavailable. Only an uninitialized worker
        // should expose Checking; an established connection is stale-while-
        // revalidate until the probe actually fails.
        if let Ok(current) = state.read() {
            if matches!(current.status, ProbeStatus::Unknown | ProbeStatus::Checking) {
                drop(current);
                set_state(
                    &state,
                    RelayHealthSnapshot {
                        status: ProbeStatus::Checking,
                        diagnostic_code: ErrorCode::new("relay.probe_running"),
                        latency_ms: None,
                        failures,
                    },
                );
            }
        }
        let started = Instant::now();
        match port.check_relay_health() {
            Ok(()) => {
                failures = 0;
                set_state(
                    &state,
                    RelayHealthSnapshot {
                        status: ProbeStatus::Healthy,
                        diagnostic_code: ErrorCode::new("relay.ready"),
                        latency_ms: Some(
                            u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
                        ),
                        failures,
                    },
                );
                next_at = Instant::now() + HEALTHY_INTERVAL;
            }
            Err(code) if code == ErrorCode::new("relay.connection_busy") => {
                // A foreground exchange owns the transport. Preserve the
                // last usable state and retry quickly without counting this
                // expected contention as a relay failure.
                next_at = Instant::now() + Duration::from_secs(1);
            }
            Err(code) => {
                failures = failures.saturating_add(1);
                set_state(
                    &state,
                    RelayHealthSnapshot {
                        // One failed request is a transient observation, not
                        // a verdict on a persistent stream. Preserve the
                        // failure code but only project degraded after two
                        // consecutive failures.
                        status: if failures >= 2 {
                            ProbeStatus::Degraded
                        } else {
                            ProbeStatus::Checking
                        },
                        diagnostic_code: code,
                        latency_ms: None,
                        failures,
                    },
                );
                next_at = Instant::now() + retry_delay(failures);
            }
        }
    }
}

/// Adds bounded full-jitter to the exponential retry schedule.  Independent
/// clients otherwise reconnect at exactly the same five/15/30/60 second
/// boundaries after a relay or network outage, creating avoidable bursts on
/// the onion service.  The wake command still bypasses this delay after an
/// explicit network-change notification.
fn retry_delay(failures: u32) -> Duration {
    let index = usize::try_from(failures.saturating_sub(1))
        .unwrap_or(usize::MAX)
        .min(RETRY_BACKOFF.len() - 1);
    let base = RETRY_BACKOFF[index];
    let jitter_window = (base / 2).max(Duration::from_secs(1));
    let jitter_millis = u64::try_from(jitter_window.as_millis()).unwrap_or(u64::MAX);
    let entropy = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| u64::from(duration.subsec_nanos()))
        .unwrap_or(0);
    let offset = if jitter_millis == 0 { 0 } else { entropy % jitter_millis };
    base + Duration::from_millis(offset)
}

fn set_state(state: &RwLock<RelayHealthSnapshot>, next: RelayHealthSnapshot) {
    if let Ok(mut value) = state.write() {
        *value = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    struct Failing;
    impl RelayHealthPort for Failing {
        fn check_relay_health(&self) -> Result<(), ErrorCode> {
            Err(ErrorCode::new("relay.connect_failed"))
        }
    }

    #[test]
    fn first_failed_probe_is_retrying_not_degraded() {
        let worker = RelayHealthWorker::spawn(Arc::new(Failing)).expect("spawn worker");
        let deadline = Instant::now() + Duration::from_secs(1);
        while worker.handle().snapshot().diagnostic_code != ErrorCode::new("relay.connect_failed")
            && Instant::now() < deadline
        {
            thread::yield_now();
        }
        let snapshot = worker.handle().snapshot();
        assert_eq!(snapshot.status, ProbeStatus::Checking);
        assert_eq!(snapshot.diagnostic_code, ErrorCode::new("relay.connect_failed"));
        worker.shutdown();
    }

    struct Scripted {
        results: Mutex<VecDeque<Result<(), ErrorCode>>>,
    }

    impl RelayHealthPort for Scripted {
        fn check_relay_health(&self) -> Result<(), ErrorCode> {
            self.results.lock().expect("script lock").pop_front().unwrap_or(Ok(()))
        }
    }

    #[test]
    fn network_wake_recovers_after_transient_relay_failure() {
        let port = Arc::new(Scripted {
            results: Mutex::new(VecDeque::from([
                Err(ErrorCode::new("relay.route_changed")),
                Ok(()),
            ])),
        });
        let worker = RelayHealthWorker::spawn(port).expect("spawn worker");
        let failure_deadline = Instant::now() + Duration::from_secs(1);
        while worker.handle().snapshot().diagnostic_code != ErrorCode::new("relay.route_changed")
            && Instant::now() < failure_deadline
        {
            thread::yield_now();
        }
        assert_eq!(worker.handle().snapshot().status, ProbeStatus::Checking);

        // A route/network callback must wake the worker immediately instead of
        // waiting for the five-second retry backoff.
        worker.handle().network_changed();
        let recovery_deadline = Instant::now() + Duration::from_secs(1);
        while worker.handle().snapshot().status != ProbeStatus::Healthy
            && Instant::now() < recovery_deadline
        {
            thread::yield_now();
        }
        assert_eq!(worker.handle().snapshot().status, ProbeStatus::Healthy);
        assert_eq!(worker.handle().snapshot().failures, 0);
        worker.shutdown();
    }

    #[test]
    fn two_failures_degrade_then_network_wake_recovers() {
        let port = Arc::new(Scripted {
            results: Mutex::new(VecDeque::from([
                Err(ErrorCode::new("relay.timeout")),
                Err(ErrorCode::new("relay.timeout")),
                Ok(()),
            ])),
        });
        let worker = RelayHealthWorker::spawn(port).expect("spawn worker");
        let first_deadline = Instant::now() + Duration::from_secs(1);
        while worker.handle().snapshot().failures < 1 && Instant::now() < first_deadline {
            thread::yield_now();
        }
        assert_eq!(worker.handle().snapshot().status, ProbeStatus::Checking);

        worker.handle().network_changed();
        let degraded_deadline = Instant::now() + Duration::from_secs(1);
        while worker.handle().snapshot().failures < 2 && Instant::now() < degraded_deadline {
            thread::yield_now();
        }
        assert_eq!(worker.handle().snapshot().status, ProbeStatus::Degraded);

        worker.handle().network_changed();
        let recovery_deadline = Instant::now() + Duration::from_secs(1);
        while worker.handle().snapshot().status != ProbeStatus::Healthy
            && Instant::now() < recovery_deadline
        {
            thread::yield_now();
        }
        assert_eq!(worker.handle().snapshot().status, ProbeStatus::Healthy);
        assert_eq!(worker.handle().snapshot().failures, 0);
        worker.shutdown();
    }

    #[test]
    fn transport_contention_does_not_count_as_relay_failure() {
        let port = Arc::new(Scripted {
            results: Mutex::new(VecDeque::from([
                Err(ErrorCode::new("relay.connection_busy")),
                Ok(()),
            ])),
        });
        let worker = RelayHealthWorker::spawn(port).expect("spawn worker");
        let busy_deadline = Instant::now() + Duration::from_secs(1);
        while worker.handle().snapshot().diagnostic_code != ErrorCode::new("relay.connection_busy")
            && Instant::now() < busy_deadline
        {
            thread::yield_now();
        }
        let snapshot = worker.handle().snapshot();
        assert_eq!(snapshot.failures, 0);
        assert_ne!(snapshot.status, ProbeStatus::Degraded);

        worker.handle().network_changed();
        let recovery_deadline = Instant::now() + Duration::from_secs(1);
        while worker.handle().snapshot().status != ProbeStatus::Healthy
            && Instant::now() < recovery_deadline
        {
            thread::yield_now();
        }
        assert_eq!(worker.handle().snapshot().status, ProbeStatus::Healthy);
        worker.shutdown();
    }
}
