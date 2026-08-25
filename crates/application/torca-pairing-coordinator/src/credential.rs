use torca_foundation::OpaqueId;
use torca_pairing::PairingSessionId;

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

    /// Saves encrypted transport material for an in-flight pairing. Implementations backed by
    /// protected storage override this; in-memory test adapters may leave it as a no-op.
    fn store_pairing_state(
        &mut self,
        _session_id: PairingSessionId,
        _state: &[u8],
    ) -> Result<(), PairingCredentialError> {
        Ok(())
    }

    /// Loads encrypted transport material for one in-flight pairing.
    fn load_pairing_state(
        &self,
        _session_id: PairingSessionId,
    ) -> Result<Option<Vec<u8>>, PairingCredentialError> {
        Ok(None)
    }

    /// Removes transport material after cancellation, expiry or mutual completion.
    fn delete_pairing_state(
        &mut self,
        _session_id: PairingSessionId,
    ) -> Result<bool, PairingCredentialError> {
        Ok(false)
    }
}

impl PairingPeerSecretStore for Box<dyn PairingPeerSecretStore + Send> {
    fn store_peer_secret(
        &mut self,
        secret: PairingDerivedSecret,
    ) -> Result<OpaqueId, PairingCredentialError> {
        (**self).store_peer_secret(secret)
    }

    fn delete_peer_secret(&mut self, handle: OpaqueId) -> Result<bool, PairingCredentialError> {
        (**self).delete_peer_secret(handle)
    }

    fn store_pairing_state(
        &mut self,
        session_id: PairingSessionId,
        state: &[u8],
    ) -> Result<(), PairingCredentialError> {
        (**self).store_pairing_state(session_id, state)
    }

    fn load_pairing_state(
        &self,
        session_id: PairingSessionId,
    ) -> Result<Option<Vec<u8>>, PairingCredentialError> {
        (**self).load_pairing_state(session_id)
    }

    fn delete_pairing_state(
        &mut self,
        session_id: PairingSessionId,
    ) -> Result<bool, PairingCredentialError> {
        (**self).delete_pairing_state(session_id)
    }
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
