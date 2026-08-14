use std::time::Duration;
use torca_foundation::Timestamp;

/// Small application-owned scheduler for durable attachment jobs.  It does
/// not know about sockets or storage; it only prevents the runtime tick from
/// repeatedly entering the adapter while a previous Tor attempt is settling.
#[derive(Clone, Copy, Debug, Default)]
pub struct AttachmentJobScheduler {
    next_due_ms: i64,
}

impl AttachmentJobScheduler {
    const MIN_INTERVAL_MS: i64 = 250;

    pub const fn new() -> Self {
        Self { next_due_ms: 0 }
    }

    pub const fn due(self, now: Timestamp) -> bool {
        now.to_unix_millis() >= self.next_due_ms
    }

    pub fn record_attempt(&mut self, now: Timestamp) {
        self.next_due_ms = now.to_unix_millis().saturating_add(Self::MIN_INTERVAL_MS);
    }

    pub fn next_delay(self, now: Timestamp) -> Option<Duration> {
        if self.next_due_ms == 0 {
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
        self.next_due_ms = 0;
    }
}
