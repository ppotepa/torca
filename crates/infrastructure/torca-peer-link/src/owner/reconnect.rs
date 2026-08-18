use std::time::Duration;

use torca_crypto::{CryptoProvider, RustCryptoProvider};
use torca_foundation::Timestamp;

use crate::PeerLinkError;

const RECONNECT_BASE_MS: u64 = 1_000;
const RECONNECT_MAX_MS: u64 = 60_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ReconnectEntry {
    pub(super) failures: u32,
    pub(super) next_attempt_at: Timestamp,
    pub(super) in_progress: bool,
}

pub(super) fn reconnect_delay(
    random_provider: &mut RustCryptoProvider,
    failures: u32,
) -> Result<Duration, PeerLinkError> {
    let exponent = failures.saturating_sub(1).min(16);
    let base = RECONNECT_BASE_MS.saturating_mul(1_u64 << exponent).min(RECONNECT_MAX_MS);
    let jitter_room = (base / 4).min(RECONNECT_MAX_MS.saturating_sub(base));
    let jitter = if jitter_room == 0 {
        0
    } else {
        let mut random = [0_u8; 8];
        random_provider.fill_random(&mut random).map_err(|_| PeerLinkError::Randomness)?;
        u64::from_le_bytes(random) % (jitter_room + 1)
    };
    Ok(Duration::from_millis(base + jitter))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconnect_backoff_is_bounded() {
        let mut random = RustCryptoProvider;
        for failures in [1, 2, 3, 8, 32] {
            let delay = reconnect_delay(&mut random, failures).expect("randomness available");
            assert!(delay >= Duration::from_secs(1));
            assert!(delay <= Duration::from_secs(60));
        }
    }
}
