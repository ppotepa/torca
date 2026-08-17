#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct RecoveryEpoch(u64);

impl RecoveryEpoch {
    pub(super) fn advance(&mut self) -> Self {
        self.0 = self.0.saturating_add(1);
        *self
    }

    pub(super) const fn matches(self, other: Self) -> bool {
        self.0 == other.0
    }
}

#[cfg(test)]
mod tests {
    use super::RecoveryEpoch;

    #[test]
    fn advancing_invalidates_results_from_an_older_recovery() {
        let mut current = RecoveryEpoch::default();
        let first = current.advance();
        assert!(current.matches(first));

        let second = current.advance();
        assert!(!second.matches(first));
        assert!(current.matches(second));
    }

    #[test]
    fn shutdown_then_restart_cannot_accept_the_shutdown_generation() {
        let mut current = RecoveryEpoch::default();
        let bootstrap = current.advance();
        let shutdown = current.advance();
        let restarted = current.advance();

        assert!(!current.matches(bootstrap));
        assert!(!current.matches(shutdown));
        assert!(current.matches(restarted));
    }

    #[test]
    fn newer_recovery_supersedes_an_in_flight_worker_result() {
        let mut current = RecoveryEpoch::default();
        let slow_worker = current.advance();
        let replacement_worker = current.advance();

        assert!(!current.matches(slow_worker));
        assert!(current.matches(replacement_worker));
    }
}
