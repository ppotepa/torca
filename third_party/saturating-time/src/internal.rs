use std::{sync::LazyLock, time::{Duration, Instant, SystemTime}};

static MAX_SYSTEM_TIME: LazyLock<SystemTime> = LazyLock::new(find_max);
static MIN_SYSTEM_TIME: LazyLock<SystemTime> = LazyLock::new(find_min);
static MAX_INSTANT: LazyLock<Instant> = LazyLock::new(find_max);
static MIN_INSTANT: LazyLock<Instant> = LazyLock::new(find_min);

pub trait SaturatingTime: Sized + Copy + PartialEq {
    fn anchor() -> Self;
    fn max_value() -> Self;
    fn min_value() -> Self;
    fn checked_add(&self, duration: Duration) -> Option<Self>;
    fn checked_sub(&self, duration: Duration) -> Option<Self>;
    fn checked_duration_since(&self, earlier: Self) -> Option<Duration>;
}

impl SaturatingTime for SystemTime {
    fn anchor() -> Self { Self::UNIX_EPOCH }
    fn max_value() -> Self { *MAX_SYSTEM_TIME }
    fn min_value() -> Self { *MIN_SYSTEM_TIME }
    fn checked_add(&self, duration: Duration) -> Option<Self> { Self::checked_add(self, duration) }
    fn checked_sub(&self, duration: Duration) -> Option<Self> { Self::checked_sub(self, duration) }
    fn checked_duration_since(&self, earlier: Self) -> Option<Duration> {
        Self::duration_since(self, earlier).ok()
    }
}

impl SaturatingTime for Instant {
    fn anchor() -> Self { Self::now() }
    fn max_value() -> Self { *MAX_INSTANT }
    fn min_value() -> Self { *MIN_INSTANT }
    fn checked_add(&self, duration: Duration) -> Option<Self> { Self::checked_add(self, duration) }
    fn checked_sub(&self, duration: Duration) -> Option<Self> { Self::checked_sub(self, duration) }
    fn checked_duration_since(&self, _earlier: Self) -> Option<Duration> { unreachable!() }
}

fn find_max<T: SaturatingTime>() -> T { find_limit(T::checked_add) }
fn find_min<T: SaturatingTime>() -> T { find_limit(T::checked_sub) }

/// Finds a platform limit without assuming that the OS represents nanoseconds.
/// Windows rounds a 1ns operation to zero.  Treating `Some(res)` as progress
/// in that case makes the original implementation spin forever.
fn find_limit<T, F>(f: F) -> T
where
    T: SaturatingTime,
    F: Fn(&T, Duration) -> Option<T>,
{
    const INITIAL_STEP: Duration = Duration::new(1_000_000_000_000_000_000, 0);
    const ONE_NS: Duration = Duration::new(0, 1);

    let mut step = INITIAL_STEP;
    let mut res = T::anchor();
    loop {
        match f(&res, step) {
            Some(next) if next != res => res = next,
            Some(_) | None if step == ONE_NS => return res,
            Some(_) | None => step = std::cmp::max(ONE_NS, step / 2),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_time_limit_initializes_without_spinning() {
        let min = <SystemTime as super::super::SaturatingTime>::min_value();
        let max = <SystemTime as super::super::SaturatingTime>::max_value();
        assert!(min <= SystemTime::UNIX_EPOCH);
        assert!(max >= SystemTime::UNIX_EPOCH);
    }

    #[test]
    fn no_progress_at_one_nanosecond_is_a_limit() {
        let value = find_limit::<SystemTime, _>(|current, _| Some(*current));
        assert_eq!(value, SystemTime::UNIX_EPOCH);
    }
}
