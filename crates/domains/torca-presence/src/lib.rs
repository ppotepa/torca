//! Domain policy for deriving availability from independently observed facts.

use std::time::Duration;
use torca_foundation::Timestamp;

/// Raw facts reported by peer and endpoint adapters.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PresenceObservation {
    pub peer_connected: bool,
    pub endpoint_available: bool,
    pub conversation_active: bool,
    pub last_activity_at: Option<Timestamp>,
}

/// User-facing availability derived from, but not replacing, raw observations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PresenceState {
    Offline,
    Available,
    Active,
    Stale,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PresenceQuality {
    Unknown,
    Excellent,
    Good,
    Fair,
    Poor,
}

/// Freshness boundary supplied by application policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FreshnessPolicy {
    pub max_activity_age_ms: i64,
}

impl FreshnessPolicy {
    pub const fn new(max_activity_age_ms: i64) -> Self {
        Self { max_activity_age_ms }
    }
}

/// Derives a label while preserving all source facts in the caller's projection.
pub fn derive_presence(
    observation: PresenceObservation,
    now: Timestamp,
    freshness: FreshnessPolicy,
) -> PresenceState {
    if observation.conversation_active && observation.peer_connected {
        return PresenceState::Active;
    }
    if observation.peer_connected || observation.endpoint_available {
        return PresenceState::Available;
    }
    if observation.last_activity_at.is_some_and(|at| {
        now.to_unix_millis().saturating_sub(at.to_unix_millis()) <= freshness.max_activity_age_ms
    }) {
        PresenceState::Stale
    } else {
        PresenceState::Offline
    }
}

/// Shared health classification used by application adapters. Keeping this
/// rule in the domain prevents runtime and host layers from drifting.
pub fn classify_health(
    rtt_ms: Option<u64>,
    failures: u32,
    age: Option<Duration>,
) -> PresenceQuality {
    if failures >= 2 || age.is_some_and(|value| value > Duration::from_secs(90)) {
        return PresenceQuality::Poor;
    }
    if failures == 1 {
        return PresenceQuality::Fair;
    }
    match rtt_ms {
        Some(0..=500) => PresenceQuality::Excellent,
        Some(501..=1000) => PresenceQuality::Good,
        Some(1001..=2000) => PresenceQuality::Fair,
        Some(_) => PresenceQuality::Poor,
        None => PresenceQuality::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::{FreshnessPolicy, PresenceObservation, PresenceState, derive_presence};
    use torca_foundation::Timestamp;

    #[test]
    fn active_requires_an_active_connected_conversation() {
        let now = Timestamp::from_unix_millis(10_000).expect("timestamp");
        assert_eq!(
            derive_presence(
                PresenceObservation {
                    peer_connected: true,
                    endpoint_available: true,
                    conversation_active: true,
                    last_activity_at: None,
                },
                now,
                FreshnessPolicy::new(1_000),
            ),
            PresenceState::Active
        );
    }

    #[test]
    fn stale_activity_expires_predictably() {
        let now = Timestamp::from_unix_millis(10_000).expect("timestamp");
        let observation = PresenceObservation {
            peer_connected: false,
            endpoint_available: false,
            conversation_active: false,
            last_activity_at: Some(Timestamp::from_unix_millis(9_500).expect("timestamp")),
        };
        assert_eq!(
            derive_presence(observation, now, FreshnessPolicy::new(600)),
            PresenceState::Stale
        );
        assert_eq!(
            derive_presence(observation, now, FreshnessPolicy::new(400)),
            PresenceState::Offline
        );
    }
}
