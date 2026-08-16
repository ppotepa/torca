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
}
