use core::fmt;

/// Redaction-safe cryptographic failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CryptoError { InvalidKey, InvalidSignature, AuthenticationFailed, InvalidNonce, RandomnessUnavailable, UnsupportedAlgorithm, Internal }
impl fmt::Display for CryptoError { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{self:?}") } }
impl std::error::Error for CryptoError {}

/// Public signing key bytes.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PublicKey(pub [u8; 32]);
/// Signature bytes.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Signature(pub [u8; 64]);
/// AEAD nonce bytes.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Nonce(pub [u8; 24]);
/// Authenticated ciphertext bytes.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Ciphertext(pub Vec<u8>);

/// Private key bytes with redacted diagnostics and best-effort zeroing on drop.
#[must_use]
#[derive(Eq, PartialEq)]
pub struct SecretKey([u8; 32]);
impl SecretKey {
    /// Creates private key bytes.
    pub const fn new(bytes: [u8; 32]) -> Self { Self(bytes) }
    /// Exposes bytes only to a provider implementation.
    pub(crate) const fn expose(&self) -> &[u8; 32] { &self.0 }
}
impl fmt::Debug for SecretKey { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str("SecretKey([REDACTED])") } }
impl Drop for SecretKey { fn drop(&mut self) { self.0.fill(0); } }
