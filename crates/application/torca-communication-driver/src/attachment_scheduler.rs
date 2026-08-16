use std::time::Duration;
use torca_foundation::Timestamp;

/// Small application-owned scheduler for durable attachment jobs.  It does
/// not know about sockets or storage; it only prevents the runtime tick from
/// repeatedly entering the adapter while a previous Tor attempt is settling.
#[derive(Clone, Copy, Debug, Default)]
pub struct AttachmentJobScheduler {
    next_due_ms: i64,
    armed: bool,
}

impl AttachmentJobScheduler {
    pub const fn new() -> Self {
        Self { next_due_ms: 0, armed: false }
    }

    pub const fn due(self, now: Timestamp) -> bool {
        self.armed && now.to_unix_millis() >= self.next_due_ms
    }

    pub fn next_delay(self, now: Timestamp) -> Option<Duration> {
        if !self.armed {
            return None;
        }
        let remaining = self.next_due_ms.saturating_sub(now.to_unix_millis());
        if remaining <= 0 {
            Some(Duration::ZERO)
        } else {
            Some(Duration::from_millis(u64::try_from(remaining).unwrap_or(u64::MAX)))
        }
    }

    pub const fn wake(&mut self) {
        self.armed = true;
        self.next_due_ms = 0;
    }

    pub fn wake_after(&mut self, now: Timestamp, delay: Duration) {
        self.armed = true;
        let delay_ms = i64::try_from(delay.as_millis()).unwrap_or(i64::MAX);
        self.next_due_ms = now.to_unix_millis().saturating_add(delay_ms);
    }

    pub const fn disarm(&mut self) {
        self.armed = false;
        self.next_due_ms = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> Timestamp {
        Timestamp::from_unix_millis(1_000).expect("valid timestamp")
    }

    #[test]
    fn scheduler_is_disarmed_when_created() {
        let scheduler = AttachmentJobScheduler::new();
        assert!(!scheduler.due(now()));
        assert_eq!(scheduler.next_delay(now()), None);
    }

    #[test]
    fn wake_arms_scheduler_immediately() {
        let mut scheduler = AttachmentJobScheduler::new();
        scheduler.wake();
        assert!(scheduler.due(now()));
        assert_eq!(scheduler.next_delay(now()), Some(Duration::ZERO));
    }

    #[test]
    fn disarm_removes_future_deadline() {
        let mut scheduler = AttachmentJobScheduler::new();
        scheduler.wake();
        scheduler.disarm();
        assert!(!scheduler.due(now()));
        assert_eq!(scheduler.next_delay(now()), None);
    }

    #[test]
    fn wake_after_defers_retry_without_losing_work() {
        let mut scheduler = AttachmentJobScheduler::new();
        scheduler.wake_after(now(), Duration::from_secs(2));
        assert!(!scheduler.due(now()));
        assert_eq!(scheduler.next_delay(now()), Some(Duration::from_secs(2)));
        let later = Timestamp::from_unix_millis(3_000).expect("valid timestamp");
        assert!(scheduler.due(later));
    }
}
