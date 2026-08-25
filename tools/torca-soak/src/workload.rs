use std::time::{Duration, Instant};

/// Deterministic per-participant scheduling. The host can replay a run from
/// its seed while each participant still has an independent sleep/wake clock.
pub(crate) fn initial_deadline(now: Instant, seed: u64, participant: usize) -> Instant {
    now + Duration::from_secs(1 + ((seed.wrapping_add(participant as u64 * 17)) % 9))
}

pub(crate) fn next_deadline(
    now: Instant,
    seed: u64,
    participant: usize,
    sequence: u64,
    cadence_seconds: u64,
) -> Instant {
    let jitter = seed.wrapping_add(participant as u64 * 31).wrapping_add(sequence * 7)
        % cadence_seconds.max(1);
    now + Duration::from_secs(cadence_seconds.max(1) + jitter)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn participants_have_independent_replayable_deadlines() {
        let now = Instant::now();
        assert_ne!(initial_deadline(now, 7, 0), initial_deadline(now, 7, 1));
        assert_eq!(initial_deadline(now, 7, 0), initial_deadline(now, 7, 0));
    }

    #[test]
    fn cadence_is_never_zero() {
        let now = Instant::now();
        assert!(next_deadline(now, 1, 0, 1, 0) > now);
    }
}
