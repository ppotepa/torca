#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
pub(crate) enum Verdict {
    Pass,
    Fail,
    Inconclusive,
}

#[derive(Debug, serde::Serialize)]
pub(crate) struct Evaluation {
    pub verdict: Verdict,
    pub reasons: Vec<String>,
}

pub(crate) fn evaluate(
    cancelled: bool,
    requires_delivery: bool,
    requires_notifications: bool,
    delivered_messages: u64,
    expected_notifications: u64,
    observed_notifications: u64,
) -> Evaluation {
    let mut reasons = Vec::new();
    if cancelled {
        reasons.push("run cancelled by operator".to_owned());
        return Evaluation { verdict: Verdict::Inconclusive, reasons };
    }
    if requires_delivery && delivered_messages == 0 {
        reasons.push("no message was observed as delivered".to_owned());
    }
    if requires_notifications && expected_notifications == 0 {
        reasons.push("no Android-directed message was scheduled".to_owned());
    }
    if observed_notifications < expected_notifications {
        reasons.push(format!(
            "notification delivery incomplete: expected {expected_notifications}, observed {observed_notifications}"
        ));
    }
    Evaluation { verdict: if reasons.is_empty() { Verdict::Pass } else { Verdict::Fail }, reasons }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complete_run_passes() {
        assert_eq!(evaluate(false, true, true, 1, 1, 1).verdict, Verdict::Pass);
    }

    #[test]
    fn cancellation_is_inconclusive() {
        assert_eq!(evaluate(true, true, true, 10, 10, 0).verdict, Verdict::Inconclusive);
    }

    #[test]
    fn missing_notification_fails() {
        assert_eq!(evaluate(false, true, true, 1, 2, 1).verdict, Verdict::Fail);
    }

    #[test]
    fn idle_run_does_not_require_messages() {
        assert_eq!(evaluate(false, false, false, 0, 0, 0).verdict, Verdict::Pass);
    }

    #[test]
    fn active_run_without_android_message_fails() {
        assert_eq!(evaluate(false, true, true, 1, 0, 0).verdict, Verdict::Fail);
    }
}
