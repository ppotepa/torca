use std::time::Duration;

pub(super) const RESTART_BACKOFF: [Duration; 3] =
    [Duration::from_secs(5), Duration::from_secs(15), Duration::from_secs(30)];
pub(super) const MAX_BOOTSTRAP_ATTEMPTS: u32 = 3;
pub(super) const ONION_SERVICE_TIMEOUT: Duration = Duration::from_secs(8);
pub(super) const ONION_DEGRADED_GRACE: Duration = Duration::from_secs(300);
pub(super) const ONION_PUBLISHING_GRACE: Duration = Duration::from_secs(600);
pub(super) const MAX_ONION_PUBLICATION_ATTEMPTS: u32 = 2;
pub(super) const ONION_HEALTH_INTERVAL_REACHABLE: Duration = Duration::from_secs(15);
pub(super) const ONION_HEALTH_INTERVAL_TRANSITIONING: Duration = Duration::from_secs(3);
pub(super) const RESTART_BOOTSTRAP_TIMEOUT: Duration = Duration::from_secs(60);
