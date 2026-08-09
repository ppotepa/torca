use std::time::Duration;

use torca_foundation::Timestamp;

use crate::PairingCoordinatorError;

/// Product TTL for a newly created invitation.
pub const PAIRING_INVITATION_TTL: Duration = Duration::from_secs(5 * 60);

/// Computes the fixed product expiry deadline for a creator invitation.
pub fn invitation_expires_at(now: Timestamp) -> Result<Timestamp, PairingCoordinatorError> {
    now.checked_add(PAIRING_INVITATION_TTL).ok_or(PairingCoordinatorError::Protocol)
}
