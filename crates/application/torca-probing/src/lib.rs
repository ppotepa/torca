//! Reusable health probes for local components, relays and peers.
//!
//! Probes are deliberately independent from bootstrap and presentation. The same
//! probe can run once during startup or repeatedly under [`ProbeSupervisor`].

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use torca_foundation::Timestamp;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProbeKind {
    Readiness,
    Liveness,
    Connectivity,
    RoundTrip,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProbeTarget {
    NativeBridge,
    SecureStorage,
    Database,
    Engine,
    /// Provider-neutral communication runtime.
    Communication,
    /// Provider-neutral inbound reachability.
    IncomingReachability,
    /// Provider-owned short-lived service used to exchange pairing state.
    /// It may be a rendezvous relay, discovery endpoint or signaling service.
    PairingService,
    /// Pairing-service relay target.
    Relay,
    Peer,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProbeStatus {
    Unknown,
    Checking,
    Healthy,
    Degraded,
    Unreachable,
    Failed,
    Disabled,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProbeResult {
    pub target: ProbeTarget,
    pub kind: ProbeKind,
    pub status: ProbeStatus,
    pub diagnostic_code: String,
    pub latency_ms: Option<u64>,
    pub measured_at: Timestamp,
}

impl ProbeResult {
    #[must_use]
    pub fn healthy(target: ProbeTarget, kind: ProbeKind, at: Timestamp, latency: Duration) -> Self {
        Self {
            target,
            kind,
            status: ProbeStatus::Healthy,
            diagnostic_code: "OK".into(),
            latency_ms: Some(
                u64::try_from(latency.as_millis().min(u128::from(u64::MAX))).unwrap_or(u64::MAX),
            ),
            measured_at: at,
        }
    }

    #[must_use]
    pub fn failed(
        target: ProbeTarget,
        kind: ProbeKind,
        at: Timestamp,
        diagnostic_code: impl Into<String>,
    ) -> Self {
        Self {
            target,
            kind,
            status: ProbeStatus::Failed,
            diagnostic_code: diagnostic_code.into(),
            latency_ms: None,
            measured_at: at,
        }
    }
}

pub trait Probe: Send {
    fn target(&self) -> ProbeTarget;
    fn kind(&self) -> ProbeKind;
    fn interval(&self) -> Duration;
    fn run(&mut self, now: Timestamp) -> ProbeResult;
}

#[derive(Default)]
pub struct ProbeSupervisor {
    probes: BTreeMap<(ProbeTarget, ProbeKind), Box<dyn Probe>>,
    latest: BTreeMap<(ProbeTarget, ProbeKind), ProbeResult>,
    next_run: BTreeMap<(ProbeTarget, ProbeKind), Instant>,
}

impl ProbeSupervisor {
    pub fn register(&mut self, probe: Box<dyn Probe>) {
        let key = (probe.target(), probe.kind());
        self.next_run.insert(key, Instant::now());
        self.probes.insert(key, probe);
    }

    pub fn run_due(&mut self, now: Timestamp) -> Vec<ProbeResult> {
        let wall_now = Instant::now();
        let due = self
            .probes
            .keys()
            .copied()
            .filter(|key| self.next_run.get(key).is_some_and(|at| *at <= wall_now))
            .collect::<Vec<_>>();
        let mut results = Vec::with_capacity(due.len());
        for key in due {
            let Some(probe) = self.probes.get_mut(&key) else { continue };
            let result = probe.run(now);
            self.next_run.insert(key, wall_now + probe.interval());
            self.latest.insert(key, result.clone());
            results.push(result);
        }
        results
    }

    /// Records an observation produced by an adapter-owned health loop.
    ///
    /// This keeps probes useful when the underlying runtime already performs its own
    /// maintenance (for example peer reconnects) while preserving one reusable result ledger.
    pub fn record(&mut self, result: ProbeResult) {
        self.latest.insert((result.target, result.kind), result);
    }

    #[must_use]
    pub fn latest(&self) -> Vec<ProbeResult> {
        self.latest.values().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{ProbeKind, ProbeResult, ProbeStatus, ProbeSupervisor, ProbeTarget};
    use torca_foundation::Timestamp;

    #[test]
    fn adapter_observations_are_kept_in_the_shared_ledger() {
        let mut supervisor = ProbeSupervisor::default();
        supervisor.record(ProbeResult {
            target: ProbeTarget::PairingService,
            kind: ProbeKind::Connectivity,
            status: ProbeStatus::Healthy,
            diagnostic_code: "PAIRING_SERVICE_READY".into(),
            latency_ms: Some(12),
            measured_at: Timestamp::UNIX_EPOCH,
        });

        let latest = supervisor.latest();
        assert_eq!(latest.len(), 1);
        assert_eq!(latest[0].diagnostic_code, "PAIRING_SERVICE_READY");
    }
}
