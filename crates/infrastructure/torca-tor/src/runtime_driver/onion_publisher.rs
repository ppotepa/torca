use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TryRecvError};
use std::thread::{self, JoinHandle};
use std::time::Instant;

use super::endpoint::SharedTorEndpoint;
use super::timing::{
    MAX_ONION_PUBLICATION_ATTEMPTS, ONION_DEGRADED_GRACE,
    ONION_HEALTH_INTERVAL_REACHABLE, ONION_HEALTH_INTERVAL_TRANSITIONING,
    ONION_PUBLISHING_GRACE, ONION_SERVICE_TIMEOUT, RESTART_BACKOFF,
};
use super::{TorWake, notify_tor_wake};
use crate::{
    OnionServiceHealth, TorBootstrapEvent, TorBootstrapObserver, TorBootstrapStage, TorService,
};

pub(super) enum OnionWorkerCommand {
    Shutdown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum OnionRepublishReason {
    PublishingStalled,
    DegradedTimeout,
    Failed,
    Stopped,
    LaunchFailed,
    WorkerStopped,
}

impl OnionRepublishReason {
    pub(super) const fn code(self) -> &'static str {
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
pub(super) struct OnionPublisherFailure {
    pub(super) reason: OnionRepublishReason,
    pub(super) attempts: u32,
}

enum OnionWaitOutcome {
    Shutdown,
    Republish {
        reason: OnionRepublishReason,
        was_reachable: bool,
    },
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
pub(super) struct OnionPublisher {
    commands: SyncSender<OnionWorkerCommand>,
    events: Receiver<OnionPublisherFailure>,
    worker: Option<JoinHandle<()>>,
}

impl OnionPublisher {
    pub(super) fn spawn(
        client: Arc<TorService>,
        target: SocketAddr,
        endpoint: SharedTorEndpoint,
        observer: Option<TorBootstrapObserver>,
        wake: TorWake,
    ) -> Result<Self, std::io::Error> {
        let observer = observer.map(|observer| {
            let wake = Arc::clone(&wake);
            Arc::new(move |event: TorBootstrapEvent| {
                observer(event);
                notify_tor_wake(&wake);
            }) as TorBootstrapObserver
        });
        let (commands, receiver) = mpsc::sync_channel(1);
        let (event_sender, events) = mpsc::sync_channel(1);
        let worker = thread::Builder::new()
            .name("torca-onion-publisher".into())
            .spawn(move || {
                let mut failures = 0_u32;
                loop {
                    let (reason, was_reachable) =
                        match client.publish_onion_service(target, ONION_SERVICE_TIMEOUT) {
                            Ok(address) => {
                                endpoint.set(Some(address));
                                match wait_for_onion_recovery(
                                    &client,
                                    &receiver,
                                    observer.as_ref(),
                                ) {
                                    OnionWaitOutcome::Shutdown => {
                                        client.stop_onion_service();
                                        endpoint.set(None);
                                        return;
                                    }
                                    OnionWaitOutcome::Republish {
                                        reason,
                                        was_reachable,
                                    } => (reason, was_reachable),
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
                        notify_tor_wake(&wake);
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
        Ok(Self {
            commands,
            events,
            worker: Some(worker),
        })
    }

    pub(super) fn try_take_failure(&mut self) -> Option<OnionPublisherFailure> {
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

    pub(super) fn shutdown(mut self) {
        let _ = self.commands.send(OnionWorkerCommand::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn wait_for_onion_recovery(
    client: &TorService,
    receiver: &Receiver<OnionWorkerCommand>,
    observer: Option<&TorBootstrapObserver>,
) -> OnionWaitOutcome {
    let mut tracker = OnionRecoveryTracker::default();
    let mut last_health = None;
    loop {
        let interval = match last_health {
            Some(OnionServiceHealth::Reachable) => ONION_HEALTH_INTERVAL_REACHABLE,
            _ => ONION_HEALTH_INTERVAL_TRANSITIONING,
        };
        match receiver.recv_timeout(interval) {
            Ok(OnionWorkerCommand::Shutdown) | Err(RecvTimeoutError::Disconnected) => {
                return OnionWaitOutcome::Shutdown;
            }
            Err(RecvTimeoutError::Timeout) => {}
        }
        let health = client.onion_service_health();
        if last_health != Some(health) {
            if let Some(observer) = observer {
                let (progress, code, summary) = match health {
                    OnionServiceHealth::Reachable => (
                        100,
                        "ONION_SERVICE_READY",
                        "Private onion service is reachable",
                    ),
                    OnionServiceHealth::Degraded => (
                        60,
                        "ONION_SERVICE_DEGRADED",
                        "Onion service is reachable with degraded publication",
                    ),
                    OnionServiceHealth::Publishing => (
                        8,
                        "ONION_SERVICE_PUBLISHING",
                        "Publishing private onion service",
                    ),
                    OnionServiceHealth::Failed => (
                        0,
                        "ONION_SERVICE_FAILED",
                        "Onion service publication failed",
                    ),
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
            return OnionWaitOutcome::Republish {
                reason,
                was_reachable: tracker.was_reachable,
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{OnionRecoveryTracker, OnionRepublishReason};
    use crate::OnionServiceHealth;
    use std::time::{Duration, Instant};

    use super::super::timing::{ONION_DEGRADED_GRACE, ONION_PUBLISHING_GRACE};

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
            tracker.observe(
                OnionServiceHealth::Publishing,
                1,
                start + ONION_PUBLISHING_GRACE
            ),
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
            tracker.observe(
                OnionServiceHealth::Reachable,
                2,
                start + Duration::from_secs(30)
            ),
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
            tracker.observe(
                OnionServiceHealth::Degraded,
                1,
                start + ONION_DEGRADED_GRACE
            ),
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
