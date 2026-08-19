//! Application policy for bounded peer keepalive scheduling.
//!
//! The peer adapter owns authenticated I/O and the current health sample. This
//! module owns *when* a sample is requested. Keeping that distinction here
//! avoids hiding product retry policy in a SQLite/Tor adapter.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use torca_foundation::{OpaqueId, Timestamp};
use torca_runtime_policy::Freshness;

const HEALTHY_INTERVAL: Duration = Duration::from_secs(60);
const RETRY_INTERVAL: Duration = Duration::from_secs(10);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PeerProbeCandidate {
    pub peer_id: OpaqueId,
    pub ready: bool,
    pub eligible: bool,
    pub freshness: Freshness,
    pub reported_rtt_ms: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PeerProbeRequest {
    pub peer_id: OpaqueId,
    pub probe_id: OpaqueId,
    pub reported_rtt_ms: u64,
}

#[derive(Clone, Copy, Debug)]
struct PeerSchedule {
    next_at: Timestamp,
}

/// One single-flight lane for all P2P probes in a runtime. It prevents a large
/// contact list from opening many concurrent Tor streams while still letting
/// a leased ready contact receive an occasional health sample.
#[derive(Default)]
pub struct PeerProbeSupervisor {
    schedules: BTreeMap<OpaqueId, PeerSchedule>,
    in_flight: Option<OpaqueId>,
    sequence: u128,
}

impl PeerProbeSupervisor {
    /// Disable cosmetic probes while a restrictive battery profile is active.
    /// Clearing schedules prevents an already-due deadline from creating a
    /// zero-delay scheduler spin when the profile changes mid-probe.
    pub fn suspend(&mut self) {
        self.schedules.clear();
        self.in_flight = None;
    }

    pub fn reconcile(&mut self, candidates: &[PeerProbeCandidate], now: Timestamp) {
        let active = candidates.iter().map(|candidate| candidate.peer_id).collect::<BTreeSet<_>>();
        self.schedules.retain(|peer_id, _| active.contains(peer_id));
        if self.in_flight.is_some_and(|peer_id| !active.contains(&peer_id)) {
            self.in_flight = None;
        }
        for candidate in candidates {
            if candidate.ready
                && candidate.eligible
                && !matches!(candidate.freshness, Freshness::Live | Freshness::Recent)
            {
                self.schedules.entry(candidate.peer_id).or_insert(PeerSchedule { next_at: now });
            } else {
                self.schedules.remove(&candidate.peer_id);
                if self.in_flight == Some(candidate.peer_id) {
                    self.in_flight = None;
                }
            }
        }
    }

    pub fn next_due(
        &mut self,
        candidates: &[PeerProbeCandidate],
        now: Timestamp,
    ) -> Option<PeerProbeRequest> {
        if self.in_flight.is_some() {
            return None;
        }
        let candidate = candidates.iter().find(|candidate| {
            candidate.ready
                && candidate.eligible
                && !matches!(candidate.freshness, Freshness::Live | Freshness::Recent)
                && self
                    .schedules
                    .get(&candidate.peer_id)
                    .is_some_and(|schedule| now >= schedule.next_at)
        })?;
        self.sequence = self.sequence.saturating_add(1).max(1);
        self.in_flight = Some(candidate.peer_id);
        Some(PeerProbeRequest {
            peer_id: candidate.peer_id,
            probe_id: OpaqueId::from_u128(self.sequence),
            reported_rtt_ms: candidate.reported_rtt_ms.unwrap_or(u64::MAX),
        })
    }

    pub fn complete(&mut self, peer_id: OpaqueId, succeeded: bool, now: Timestamp) {
        if self.in_flight == Some(peer_id) {
            self.in_flight = None;
        }
        let delay = if succeeded { HEALTHY_INTERVAL } else { RETRY_INTERVAL };
        if let Some(schedule) = self.schedules.get_mut(&peer_id) {
            schedule.next_at = now.checked_add(delay).unwrap_or(now);
        }
    }

    /// Returns the next probe deadline owned by this single-flight lane.
    /// Callers can sleep until this timestamp instead of polling the lane at
    /// a fixed cadence.
    pub fn next_deadline(&self) -> Option<Timestamp> {
        self.schedules.values().map(|schedule| schedule.next_at).min()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(id: u128) -> PeerProbeCandidate {
        PeerProbeCandidate {
            peer_id: OpaqueId::from_u128(id),
            ready: true,
            eligible: true,
            freshness: Freshness::Unknown,
            reported_rtt_ms: Some(12),
        }
    }

    #[test]
    fn schedules_one_probe_and_uses_shorter_failure_backoff() {
        let now = Timestamp::UNIX_EPOCH;
        let candidates = [candidate(1), candidate(2)];
        let mut supervisor = PeerProbeSupervisor::default();
        supervisor.reconcile(&candidates, now);
        let first = supervisor.next_due(&candidates, now).expect("first probe");
        assert_eq!(first.peer_id, OpaqueId::from_u128(1));
        assert!(supervisor.next_due(&candidates, now).is_none());
        supervisor.complete(first.peer_id, false, now);
        let retry_at = now.checked_add(RETRY_INTERVAL).expect("retry time");
        assert_eq!(
            supervisor.next_due(&candidates, retry_at).expect("retry").peer_id,
            first.peer_id
        );
    }

    #[test]
    fn live_or_recent_evidence_suppresses_cosmetic_probe() {
        let now = Timestamp::UNIX_EPOCH;
        let mut candidate = candidate(1);
        candidate.freshness = Freshness::Live;
        let candidates = [candidate];
        let mut supervisor = PeerProbeSupervisor::default();
        supervisor.reconcile(&candidates, now);
        assert!(supervisor.next_due(&candidates, now).is_none());

        candidate.freshness = Freshness::Stale;
        supervisor.reconcile(&[candidate], now);
        assert!(supervisor.next_due(&[candidate], now).is_some());
    }
}
