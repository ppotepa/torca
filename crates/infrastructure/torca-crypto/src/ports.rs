use crate::{Ciphertext, CryptoError, Nonce, PublicKey, SecretKey, Signature};

/// Semantic crypto provider used by higher layers.
pub trait CryptoProvider {
    /// Generates a signing key pair.
    fn generate_signing_key(&mut self) -> Result<(SecretKey, PublicKey), CryptoError>;
    /// Signs a message.
    fn sign(&self, secret: &SecretKey, message: &[u8]) -> Result<Signature, CryptoError>;
    /// Verifies a signature.
    fn verify(&self, public: &PublicKey, message: &[u8], signature: &Signature) -> Result<(), CryptoError>;
    /// Fills caller-owned random bytes.
    fn fill_random(&mut self, output: &mut [u8]) -> Result<(), CryptoError>;
    /// Seals plaintext with associated data.
    fn seal(&self, key: &SecretKey, nonce: Nonce, associated_data: &[u8], plaintext: &[u8]) -> Result<Ciphertext, CryptoError>;
    /// Opens authenticated ciphertext.
    fn open(&self, key: &SecretKey, nonce: Nonce, associated_data: &[u8], ciphertext: &Ciphertext) -> Result<Vec<u8>, CryptoError>;
}
