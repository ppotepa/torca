use core::fmt;
use std::collections::BTreeMap;

use torca_foundation::OpaqueId;
use torca_identity::{
    GeneratedSigningKey, IdentityKeyProvider, IdentityKeyProviderError, KeyAlgorithm, KeyId,
};

use crate::{CryptoError, CryptoProvider, Signature, SigningSecretKey};

/// Redaction-safe protected-secret-store failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtectedSecretStoreError(pub String);
impl fmt::Display for ProtectedSecretStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}
impl std::error::Error for ProtectedSecretStoreError {}

/// Platform boundary for storing secret bytes under an opaque key handle.
///
/// Production implementations must use Windows DPAPI/CNG or Android Keystore-backed wrapping.
/// Implementations must never include secret bytes in errors or diagnostics.
pub trait ProtectedSecretStore: Send {
    /// Stores one secret only when the handle is unused.
    fn insert(&mut self, key_id: KeyId, secret: &[u8]) -> Result<(), ProtectedSecretStoreError>;
    /// Loads one secret into caller-owned memory for the shortest practical duration.
    fn load(&self, key_id: KeyId) -> Result<Option<Vec<u8>>, ProtectedSecretStoreError>;
    /// Deletes one secret.
    fn delete(&mut self, key_id: KeyId) -> Result<bool, ProtectedSecretStoreError>;
}

impl<T: ProtectedSecretStore + ?Sized> ProtectedSecretStore for Box<T> {
    fn insert(&mut self, key_id: KeyId, secret: &[u8]) -> Result<(), ProtectedSecretStoreError> {
        (**self).insert(key_id, secret)
    }

    fn load(&self, key_id: KeyId) -> Result<Option<Vec<u8>>, ProtectedSecretStoreError> {
        (**self).load(key_id)
    }

    fn delete(&mut self, key_id: KeyId) -> Result<bool, ProtectedSecretStoreError> {
        (**self).delete(key_id)
    }
}

/// Identity key provider that combines production algorithms with a protected secret store.
pub struct ManagedIdentityKeys<C, S> {
    crypto: C,
    store: S,
}

impl<C, S> ManagedIdentityKeys<C, S> {
    /// Creates the key manager.
    pub const fn new(crypto: C, store: S) -> Self {
        Self { crypto, store }
    }
    /// Returns the protected store for platform composition and diagnostics.
    pub const fn store(&self) -> &S {
        &self.store
    }
    /// Consumes the manager.
    pub fn into_parts(self) -> (C, S) {
        (self.crypto, self.store)
    }
}

impl<C: CryptoProvider, S: ProtectedSecretStore> ManagedIdentityKeys<C, S> {
    /// Signs using a previously generated opaque key handle.
    pub fn sign(&self, key_id: KeyId, message: &[u8]) -> Result<Signature, ManagedKeyError> {
        let mut bytes = self.store.load(key_id)?.ok_or(ManagedKeyError::NotFound)?;
        if bytes.len() != 32 {
            bytes.fill(0);
            return Err(ManagedKeyError::InvalidStoredKey);
        }
        let mut secret_bytes = [0_u8; 32];
        secret_bytes.copy_from_slice(&bytes);
        bytes.fill(0);
        let secret = SigningSecretKey::new(secret_bytes);
        self.crypto.sign(&secret, message).map_err(ManagedKeyError::Crypto)
    }

    fn new_key_id(&mut self) -> Result<KeyId, ManagedKeyError> {
        for _ in 0..8 {
            let mut bytes = [0_u8; 16];
            self.crypto.fill_random(&mut bytes)?;
            let opaque = OpaqueId::from_bytes(bytes);
            if !opaque.is_nil() {
                return Ok(KeyId::from_opaque(opaque));
            }
        }
        Err(ManagedKeyError::RandomIdentifierUnavailable)
    }
}

impl<C: CryptoProvider, S: ProtectedSecretStore> IdentityKeyProvider for ManagedIdentityKeys<C, S> {
    fn generate_signing_key(&mut self) -> Result<GeneratedSigningKey, IdentityKeyProviderError> {
        let result = (|| {
            let key_id = self.new_key_id()?;
            let (secret, public) = self.crypto.generate_signing_key()?;
            self.store.insert(key_id, secret.expose())?;
            Ok::<_, ManagedKeyError>(GeneratedSigningKey {
                key_id,
                algorithm: KeyAlgorithm::Ed25519,
                public_key: public.0.to_vec(),
            })
        })();
        result.map_err(|error| IdentityKeyProviderError(error.to_string()))
    }
}

/// Key-management failure without secret material.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ManagedKeyError {
    NotFound,
    InvalidStoredKey,
    RandomIdentifierUnavailable,
    Crypto(CryptoError),
    Store(ProtectedSecretStoreError),
}
impl fmt::Display for ManagedKeyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for ManagedKeyError {}
impl From<CryptoError> for ManagedKeyError {
    fn from(value: CryptoError) -> Self {
        Self::Crypto(value)
    }
}
impl From<ProtectedSecretStoreError> for ManagedKeyError {
    fn from(value: ProtectedSecretStoreError) -> Self {
        Self::Store(value)
    }
}

/// Deliberately unprotected in-memory store for tests only.
#[derive(Clone, Debug, Default)]
pub struct InMemoryProtectedSecretStore {
    secrets: BTreeMap<KeyId, Vec<u8>>,
}

impl Drop for InMemoryProtectedSecretStore {
    fn drop(&mut self) {
        for secret in self.secrets.values_mut() {
            secret.fill(0);
        }
    }
}

impl ProtectedSecretStore for InMemoryProtectedSecretStore {
    fn insert(&mut self, key_id: KeyId, secret: &[u8]) -> Result<(), ProtectedSecretStoreError> {
        if self.secrets.contains_key(&key_id) {
            return Err(ProtectedSecretStoreError("key handle already exists".into()));
        }
        self.secrets.insert(key_id, secret.to_vec());
        Ok(())
    }
    fn load(&self, key_id: KeyId) -> Result<Option<Vec<u8>>, ProtectedSecretStoreError> {
        Ok(self.secrets.get(&key_id).cloned())
    }
    fn delete(&mut self, key_id: KeyId) -> Result<bool, ProtectedSecretStoreError> {
        if let Some(mut secret) = self.secrets.remove(&key_id) {
            secret.fill(0);
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use torca_identity::IdentityKeyProvider;

    use crate::{
        CryptoProvider, DeterministicTestCrypto, InMemoryProtectedSecretStore, ManagedIdentityKeys,
    };

    #[test]
    fn identity_provider_returns_a_handle_and_can_sign_without_exposing_domain_secrets() {
        let crypto = DeterministicTestCrypto::default();
        let store = InMemoryProtectedSecretStore::default();
        let mut keys = ManagedIdentityKeys::new(crypto, store);
        let generated = keys.generate_signing_key().expect("generate");
        let signature = keys.sign(generated.key_id, b"message").expect("sign");
        let public: [u8; 32] = generated.public_key.try_into().expect("public key");
        keys.crypto.verify(&crate::PublicKey(public), b"message", &signature).expect("verify");
    }
}
