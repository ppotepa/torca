use core::fmt;
use std::time::Duration;

/// Millisecond-resolution UTC timestamp bounded to the range from Unix epoch through year 9999.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Timestamp(i64);

impl Timestamp {
    /// Earliest accepted Unix timestamp in milliseconds.
    pub const MIN_UNIX_MILLIS: i64 = 0;

    /// Latest accepted Unix timestamp in milliseconds: 9999-12-31T23:59:59.999Z.
    pub const MAX_UNIX_MILLIS: i64 = 253_402_300_799_999;

    /// Unix epoch.
    pub const UNIX_EPOCH: Self = Self(Self::MIN_UNIX_MILLIS);

    /// Creates a bounded timestamp from Unix milliseconds.
    ///
    /// # Errors
    ///
    /// Returns [`TimestampError::OutOfRange`] when the value is before Unix epoch or after year 9999.
    pub const fn from_unix_millis(value: i64) -> Result<Self, TimestampError> {
        if value < Self::MIN_UNIX_MILLIS || value > Self::MAX_UNIX_MILLIS {
            Err(TimestampError::OutOfRange { value })
        } else {
            Ok(Self(value))
        }
    }

    /// Returns Unix milliseconds.
    pub const fn as_unix_millis(self) -> i64 {
        self.0
    }

    /// Adds a non-negative duration while preserving timestamp bounds.
    pub fn checked_add(self, duration: Duration) -> Option<Self> {
        let milliseconds = i64::try_from(duration.as_millis()).ok()?;
        let value = self.0.checked_add(milliseconds)?;
        Self::from_unix_millis(value).ok()
    }

    /// Subtracts a non-negative duration while preserving timestamp bounds.
    pub fn checked_sub(self, duration: Duration) -> Option<Self> {
        let milliseconds = i64::try_from(duration.as_millis()).ok()?;
        let value = self.0.checked_sub(milliseconds)?;
        Self::from_unix_millis(value).ok()
    }

    /// Returns the elapsed duration when `earlier` does not occur after this timestamp.
    pub fn duration_since(self, earlier: Self) -> Option<Duration> {
        let milliseconds = self.0.checked_sub(earlier.0)?;
        let milliseconds = u64::try_from(milliseconds).ok()?;
        Some(Duration::from_millis(milliseconds))
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, formatter)
    }
}

impl TryFrom<i64> for Timestamp {
    type Error = TimestampError;

    fn try_from(value: i64) -> Result<Self, Self::Error> {
        Self::from_unix_millis(value)
    }
}

impl From<Timestamp> for i64 {
    fn from(value: Timestamp) -> Self {
        value.0
    }
}

/// Error returned when timestamp data is outside the supported range.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimestampError {
    /// The supplied Unix millisecond value is outside the supported range.
    OutOfRange {
        /// Rejected Unix millisecond value.
        value: i64,
    },
}

impl fmt::Display for TimestampError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfRange { value } => write!(
                formatter,
                "timestamp {value} is outside the supported range {}..={}",
                Timestamp::MIN_UNIX_MILLIS,
                Timestamp::MAX_UNIX_MILLIS
            ),
        }
    }
}

impl std::error::Error for TimestampError {}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{Timestamp, TimestampError};

    #[test]
    fn timestamp_rejects_values_outside_the_supported_range() {
        assert_eq!(
            Timestamp::from_unix_millis(-1),
            Err(TimestampError::OutOfRange { value: -1 })
        );
        assert!(Timestamp::from_unix_millis(Timestamp::MAX_UNIX_MILLIS).is_ok());
    }

    #[test]
    fn timestamp_arithmetic_preserves_bounds() {
        let start = Timestamp::from_unix_millis(1_000).expect("timestamp is valid");
        let end = start
            .checked_add(Duration::from_millis(250))
            .expect("addition remains in range");

        assert_eq!(end.as_unix_millis(), 1_250);
        assert_eq!(end.duration_since(start), Some(Duration::from_millis(250)));
        assert_eq!(Timestamp::UNIX_EPOCH.checked_sub(Duration::from_millis(1)), None);
    }
}
