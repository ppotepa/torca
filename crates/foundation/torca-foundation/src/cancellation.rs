use core::fmt;

/// Stable reason for cooperative cancellation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CancellationReason {
    /// Caller explicitly requested cancellation.
    Requested,
    /// Application or engine is shutting down.
    Shutdown,
    /// Newer work superseded the current operation.
    Superseded,
    /// Operation deadline elapsed.
    DeadlineExceeded,
}

/// Error returned when cooperative cancellation has been observed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Cancelled {
    reason: CancellationReason,
}

impl Cancelled {
    /// Creates a cancellation error.
    pub const fn new(reason: CancellationReason) -> Self {
        Self { reason }
    }

    /// Returns the reason cancellation was requested.
    pub const fn reason(self) -> CancellationReason {
        self.reason
    }
}

impl fmt::Display for Cancelled {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "operation cancelled: {:?}", self.reason)
    }
}

impl std::error::Error for Cancelled {}

/// Read-only cooperative cancellation contract independent from a specific async runtime.
pub trait CancellationProbe: Send + Sync {
    /// Returns the current cancellation reason, or `None` while work may continue.
    fn cancellation_reason(&self) -> Option<CancellationReason>;

    /// Returns whether cancellation has been requested.
    fn is_cancelled(&self) -> bool {
        self.cancellation_reason().is_some()
    }

    /// Fails when cancellation has been requested.
    ///
    /// # Errors
    ///
    /// Returns [`Cancelled`] with the current reason when cancellation is active.
    fn check(&self) -> Result<(), Cancelled> {
        self.cancellation_reason().map_or(Ok(()), |reason| Err(Cancelled::new(reason)))
    }
}

/// Cancellation probe that never cancels, useful for synchronous paths and deterministic tests.
#[derive(Clone, Copy, Debug, Default)]
pub struct NeverCancelled;

impl CancellationProbe for NeverCancelled {
    fn cancellation_reason(&self) -> Option<CancellationReason> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{CancellationProbe, NeverCancelled};

    #[test]
    fn never_cancelled_probe_allows_work_to_continue() {
        let probe = NeverCancelled;

        assert!(!probe.is_cancelled());
        assert_eq!(probe.check(), Ok(()));
    }
}
