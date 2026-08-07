use torca_foundation::OpaqueId;
use torca_identity::KeyId;
use torca_pairing_coordinator::{
    PairingCredentialError, PairingDerivedSecret, PairingPeerSecretStore,
};

use crate::{CryptoProvider, ProtectedSecretStore};

/// Protected pairwise-secret manager. It stores only under opaque random handles and never returns
/// secret bytes to callers after provisioning.
pub struct ManagedPeerSecrets<C, S> {
    crypto: C,
    store: S,
}

impl<C, S> ManagedPeerSecrets<C, S> {
    pub const fn new(crypto: C, store: S) -> Self {
        Self { crypto, store }
    }

    pub fn into_parts(self) -> (C, S) {
        (self.crypto, self.store)
    }
}

impl<C, S> PairingPeerSecretStore for ManagedPeerSecrets<C, S>
where
    C: CryptoProvider,
    S: ProtectedSecretStore,
{
    fn store_peer_secret(
        &mut self,
        secret: PairingDerivedSecret,
    ) -> Result<OpaqueId, PairingCredentialError> {
        let handle = self.new_handle()?;
        self.store
            .insert(
                KeyId::from_opaque(handle),
                secret.expose_for_protected_storage(),
            )
            .map_err(|_| PairingCredentialError::Storage)?;
        Ok(handle)
    }

    fn delete_peer_secret(&mut self, handle: OpaqueId) -> Result<bool, PairingCredentialError> {
        self.store
            .delete(KeyId::from_opaque(handle))
            .map_err(|_| PairingCredentialError::Storage)
    }
}

impl<C, S> ManagedPeerSecrets<C, S>
where
    C: CryptoProvider,
{
    fn new_handle(&mut self) -> Result<OpaqueId, PairingCredentialError> {
        for _ in 0..8 {
            let mut bytes = [0_u8; 16];
            self.crypto
                .fill_random(&mut bytes)
                .map_err(|_| PairingCredentialError::Storage)?;
            let handle = OpaqueId::from_bytes(bytes);
            if !handle.is_nil() {
                return Ok(handle);
            }
        }
        Err(PairingCredentialError::RandomIdentifierUnavailable)
    }
}
