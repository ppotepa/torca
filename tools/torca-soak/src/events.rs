//! Stable event names shared by the SOAK runner and its cockpit.
//!
//! The timeline remains JSONL for easy inspection, but high-value assertions
//! must not rely on repeated ad-hoc string literals.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EventKind {
    NotificationAssertionPassed,
    NotificationAssertionFailed,
    RunVerdict,
}

impl EventKind {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::NotificationAssertionPassed => "notification_assertion_passed",
            Self::NotificationAssertionFailed => "notification_assertion_failed",
            Self::RunVerdict => "run_verdict",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::EventKind;

    #[test]
    fn assertion_event_names_are_stable() {
        assert_eq!(
            EventKind::NotificationAssertionPassed.as_str(),
            "notification_assertion_passed"
        );
        assert_eq!(EventKind::RunVerdict.as_str(), "run_verdict");
    }
}
