use torca_foundation::OpaqueId;

use crate::PairingDerivedSecret;

/// Protected storage boundary for a long-lived pairwise peer secret derived during pairing.
pub trait PairingPeerSecretStore {
    /// Stores secret material under a fresh opaque handle and returns only that handle.
    fn store_peer_secret(
        &mut self,
        secret: PairingDerivedSecret,
    ) -> Result<OpaqueId, PairingCredentialError>;

    /// Removes a secret created during a failed or rolled-back pairing finalization.
    fn delete_peer_secret(&mut self, handle: OpaqueId) -> Result<bool, PairingCredentialError>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PairingCredentialError {
    Storage,
    RandomIdentifierUnavailable,
}
impl core::fmt::Display for PairingCredentialError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for PairingCredentialError {}
