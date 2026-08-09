use torca_identity::KeyId;
use torca_peer_protocol::{HandshakeSigner, HandshakeSigningError, HandshakeVerifier};

use crate::{CryptoProvider, ManagedIdentityKeys, ProtectedSecretStore, PublicKey, Signature};

/// Handshake signer backed by a borrowed protected identity-key manager.
pub struct ManagedHandshakeSigner<'a, C, S> {
    keys: &'a ManagedIdentityKeys<C, S>,
    key_id: KeyId,
}
impl<'a, C, S> ManagedHandshakeSigner<'a, C, S> {
    pub const fn new(keys: &'a ManagedIdentityKeys<C, S>, key_id: KeyId) -> Self {
        Self { keys, key_id }
    }
}
impl<C, S> HandshakeSigner for ManagedHandshakeSigner<'_, C, S>
where
    C: CryptoProvider,
    S: ProtectedSecretStore,
{
    fn sign(&self, canonical: &[u8]) -> Result<Vec<u8>, HandshakeSigningError> {
        self.keys
            .sign(self.key_id, canonical)
            .map(|signature| signature.0.to_vec())
            .map_err(|error| HandshakeSigningError(error.to_string()))
    }
}

/// Process-owned handshake signer. It keeps only the opaque key handle and protected-store owner;
/// private signing bytes are loaded and zeroed by `ManagedIdentityKeys::sign` per operation.
pub struct OwnedHandshakeSigner<C, S> {
    keys: ManagedIdentityKeys<C, S>,
    key_id: KeyId,
}
impl<C, S> OwnedHandshakeSigner<C, S> {
    pub const fn new(keys: ManagedIdentityKeys<C, S>, key_id: KeyId) -> Self {
        Self { keys, key_id }
    }
}
impl<C, S> HandshakeSigner for OwnedHandshakeSigner<C, S>
where
    C: CryptoProvider,
    S: ProtectedSecretStore,
{
    fn sign(&self, canonical: &[u8]) -> Result<Vec<u8>, HandshakeSigningError> {
        self.keys
            .sign(self.key_id, canonical)
            .map(|signature| signature.0.to_vec())
            .map_err(|error| HandshakeSigningError(error.to_string()))
    }
}

/// Ed25519 peer-handshake verifier bound to the public key stored on a verified contact.
pub struct Ed25519HandshakeVerifier<C = crate::RustCryptoProvider> {
    crypto: C,
    public_key: PublicKey,
}
impl<C> Ed25519HandshakeVerifier<C> {
    pub const fn new(crypto: C, public_key: [u8; 32]) -> Self {
        Self { crypto, public_key: PublicKey(public_key) }
    }
}
impl Ed25519HandshakeVerifier<crate::RustCryptoProvider> {
    pub const fn from_bytes(public_key: [u8; 32]) -> Self {
        Self::new(crate::RustCryptoProvider, public_key)
    }
}
impl<C: CryptoProvider> HandshakeVerifier for Ed25519HandshakeVerifier<C> {
    fn verify(&self, canonical: &[u8], proof: &[u8]) -> bool {
        let Ok(bytes) = <[u8; 64]>::try_from(proof) else {
            return false;
        };
        self.crypto.verify(&self.public_key, canonical, &Signature(bytes)).is_ok()
    }
}
