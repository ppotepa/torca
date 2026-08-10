use torca_foundation::OpaqueId;
use torca_identity::KeyId;
use torca_pairing::PairingSessionId;
use torca_pairing_coordinator::{
    PairingCredentialError, PairingDerivedSecret, PairingPeerSecretStore,
};

use crate::{Ciphertext, CryptoProvider, Nonce, ProtectedSecretStore, SealingKey};

/// Protected pairwise-secret manager. Secret bytes are loaded only for the duration of one
/// authenticated-encryption operation and are zeroed before returning.
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
            .insert(KeyId::from_opaque(handle), secret.expose_for_protected_storage())
            .map_err(|_| PairingCredentialError::Storage)?;
        Ok(handle)
    }

    fn delete_peer_secret(&mut self, handle: OpaqueId) -> Result<bool, PairingCredentialError> {
        self.store.delete(KeyId::from_opaque(handle)).map_err(|_| PairingCredentialError::Storage)
    }

    fn store_pairing_state(
        &mut self,
        session_id: PairingSessionId,
        state: &[u8],
    ) -> Result<(), PairingCredentialError> {
        let key = pairing_state_key(session_id);
        if self.store.load(key).map_err(|_| PairingCredentialError::Storage)?.is_some() {
            self.store.delete(key).map_err(|_| PairingCredentialError::Storage)?;
        }
        self.store.insert(key, state).map_err(|_| PairingCredentialError::Storage)
    }

    fn load_pairing_state(
        &self,
        session_id: PairingSessionId,
    ) -> Result<Option<Vec<u8>>, PairingCredentialError> {
        self.store.load(pairing_state_key(session_id)).map_err(|_| PairingCredentialError::Storage)
    }

    fn delete_pairing_state(
        &mut self,
        session_id: PairingSessionId,
    ) -> Result<bool, PairingCredentialError> {
        self.store
            .delete(pairing_state_key(session_id))
            .map_err(|_| PairingCredentialError::Storage)
    }
}

fn pairing_state_key(session_id: PairingSessionId) -> KeyId {
    let mut bytes = session_id.to_opaque().into_bytes();
    for (byte, domain) in bytes.iter_mut().zip(*b"torca-pair-state") {
        *byte ^= domain;
    }
    KeyId::from_opaque(OpaqueId::from_bytes(bytes))
}

impl<C, S> ManagedPeerSecrets<C, S>
where
    C: CryptoProvider,
    S: ProtectedSecretStore,
{
    /// Generates an AEAD nonce without exposing the protected key.
    pub fn peer_nonce(&mut self) -> Result<Nonce, PeerSecretError> {
        let mut bytes = [0_u8; 24];
        self.crypto.fill_random(&mut bytes).map_err(|_| PeerSecretError::Crypto)?;
        Ok(Nonce(bytes))
    }

    /// Authenticates and encrypts one peer payload using the secret referenced by `handle`.
    pub fn seal_peer_payload(
        &self,
        handle: OpaqueId,
        nonce: Nonce,
        associated_data: &[u8],
        plaintext: &[u8],
    ) -> Result<Ciphertext, PeerSecretError> {
        let key = self.load_key(handle)?;
        self.crypto
            .seal(&key, nonce, associated_data, plaintext)
            .map_err(|_| PeerSecretError::Crypto)
    }

    /// Authenticates and decrypts one peer payload using the secret referenced by `handle`.
    pub fn open_peer_payload(
        &self,
        handle: OpaqueId,
        nonce: Nonce,
        associated_data: &[u8],
        ciphertext: &Ciphertext,
    ) -> Result<Vec<u8>, PeerSecretError> {
        let key = self.load_key(handle)?;
        self.crypto
            .open(&key, nonce, associated_data, ciphertext)
            .map_err(|_| PeerSecretError::Authentication)
    }

    fn load_key(&self, handle: OpaqueId) -> Result<SealingKey, PeerSecretError> {
        let mut stored = self
            .store
            .load(KeyId::from_opaque(handle))
            .map_err(|_| PeerSecretError::Storage)?
            .ok_or(PeerSecretError::NotFound)?;
        if stored.len() != 32 {
            stored.fill(0);
            return Err(PeerSecretError::InvalidStoredSecret);
        }
        let mut bytes = [0_u8; 32];
        bytes.copy_from_slice(&stored);
        stored.fill(0);
        Ok(SealingKey::new(bytes))
    }
}

impl<C, S> ManagedPeerSecrets<C, S>
where
    C: CryptoProvider,
{
    fn new_handle(&mut self) -> Result<OpaqueId, PairingCredentialError> {
        for _ in 0..8 {
            let mut bytes = [0_u8; 16];
            self.crypto.fill_random(&mut bytes).map_err(|_| PairingCredentialError::Storage)?;
            let handle = OpaqueId::from_bytes(bytes);
            if !handle.is_nil() {
                return Ok(handle);
            }
        }
        Err(PairingCredentialError::RandomIdentifierUnavailable)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PeerSecretError {
    NotFound,
    InvalidStoredSecret,
    Storage,
    Crypto,
    Authentication,
}

impl core::fmt::Display for PeerSecretError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for PeerSecretError {}

#[cfg(test)]
mod tests {
    use torca_foundation::OpaqueId;
    use torca_pairing::PairingSessionId;
    use torca_pairing_coordinator::PairingPeerSecretStore;

    use crate::{DeterministicTestCrypto, InMemoryProtectedSecretStore, ManagedPeerSecrets};

    #[test]
    fn pairing_restart_state_replaces_the_previous_protected_record() {
        let mut secrets = ManagedPeerSecrets::new(
            DeterministicTestCrypto::default(),
            InMemoryProtectedSecretStore::default(),
        );
        let id = PairingSessionId::from_opaque(OpaqueId::from_u128(42));
        secrets.store_pairing_state(id, b"first").expect("first state");
        secrets.store_pairing_state(id, b"second").expect("replacement state");
        assert_eq!(secrets.load_pairing_state(id).expect("load state"), Some(b"second".to_vec()));
    }
}
