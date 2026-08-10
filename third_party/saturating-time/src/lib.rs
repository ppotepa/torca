#![forbid(unsafe_code)]

use std::time::{Duration, Instant, SystemTime};

mod internal;

/// Saturating arithmetic for standard-library time values.
pub trait SaturatingTime: internal::SaturatingTime {
    /// Returns the largest representable value on this platform.
    fn max_value() -> Self {
        internal::SaturatingTime::max_value()
    }

    /// Returns the smallest representable value on this platform.
    fn min_value() -> Self {
        internal::SaturatingTime::min_value()
    }

    /// Adds a duration, saturating at the platform maximum.
    fn saturating_add(self, duration: Duration) -> Self {
        self.checked_add(duration)
            .unwrap_or(SaturatingTime::max_value())
    }

    /// Subtracts a duration, saturating at the platform minimum.
    fn saturating_sub(self, duration: Duration) -> Self {
        self.checked_sub(duration)
            .unwrap_or(SaturatingTime::min_value())
    }

    /// Returns a non-negative duration between two values.
    fn saturating_duration_since(&self, earlier: Self) -> Duration {
        self.checked_duration_since(earlier)
            .unwrap_or(Duration::ZERO)
    }
}

#[cfg(feature = "nightly")]
impl SaturatingTime for SystemTime {
    fn max_value() -> Self { Self::MAX }
    fn min_value() -> Self { Self::MIN }
    fn saturating_add(self, duration: Duration) -> Self { Self::saturating_add(&self, duration) }
    fn saturating_sub(self, duration: Duration) -> Self { Self::saturating_sub(&self, duration) }
    fn saturating_duration_since(&self, earlier: Self) -> Duration {
        Self::saturating_duration_since(self, earlier)
    }
}

#[cfg(not(feature = "nightly"))]
impl SaturatingTime for SystemTime {}

impl SaturatingTime for Instant {
    fn saturating_duration_since(&self, earlier: Self) -> Duration {
        Self::saturating_duration_since(self, earlier)
    }
}
