use crate::{
    Ciphertext, CryptoError, Nonce, PublicKey, SealingKey, Signature, SigningSecretKey,
};

/// Semantic crypto provider used by higher layers.
pub trait CryptoProvider {
    /// Generates a signing key pair.
    fn generate_signing_key(&mut self) -> Result<(SigningSecretKey, PublicKey), CryptoError>;
    /// Generates an independent symmetric sealing key.
    fn generate_sealing_key(&mut self) -> Result<SealingKey, CryptoError>;
    /// Signs a message.
    fn sign(&self, secret: &SigningSecretKey, message: &[u8]) -> Result<Signature, CryptoError>;
    /// Verifies a signature.
    fn verify(&self, public: &PublicKey, message: &[u8], signature: &Signature) -> Result<(), CryptoError>;
    /// Fills caller-owned random bytes.
    fn fill_random(&mut self, output: &mut [u8]) -> Result<(), CryptoError>;
    /// Seals plaintext with associated data.
    fn seal(&self, key: &SealingKey, nonce: Nonce, associated_data: &[u8], plaintext: &[u8]) -> Result<Ciphertext, CryptoError>;
    /// Opens authenticated ciphertext.
    fn open(&self, key: &SealingKey, nonce: Nonce, associated_data: &[u8], ciphertext: &Ciphertext) -> Result<Vec<u8>, CryptoError>;
}
