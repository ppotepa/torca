use core::fmt;

/// Redaction-safe cryptographic failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CryptoError {
    /// Key material is invalid for the requested operation.
    InvalidKey,
    /// Signature verification failed.
    InvalidSignature,
    /// Authenticated decryption failed.
    AuthenticationFailed,
    /// Nonce data is invalid.
    InvalidNonce,
    /// Secure randomness was unavailable.
    RandomnessUnavailable,
    /// Requested algorithm is unsupported.
    UnsupportedAlgorithm,
    /// Provider failed without a safely exposable cause.
    Internal,
}
impl fmt::Display for CryptoError { fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result { write!(formatter, "{self:?}") } }
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

/// Private signing key bytes with redacted diagnostics and best-effort zeroing.
#[must_use]
#[derive(Eq, PartialEq)]
pub struct SigningSecretKey([u8; 32]);
impl SigningSecretKey {
    /// Creates signing-key bytes for a provider implementation.
    pub const fn new(bytes: [u8; 32]) -> Self { Self(bytes) }
    pub(crate) const fn expose(&self) -> &[u8; 32] { &self.0 }
}
impl fmt::Debug for SigningSecretKey { fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result { formatter.write_str("SigningSecretKey([REDACTED])") } }
impl Drop for SigningSecretKey { fn drop(&mut self) { self.0.fill(0); } }

/// Symmetric authenticated-encryption key with redacted diagnostics and best-effort zeroing.
#[must_use]
#[derive(Eq, PartialEq)]
pub struct SealingKey([u8; 32]);
impl SealingKey {
    /// Creates sealing-key bytes for a provider implementation.
    pub const fn new(bytes: [u8; 32]) -> Self { Self(bytes) }
    pub(crate) const fn expose(&self) -> &[u8; 32] { &self.0 }
}
impl fmt::Debug for SealingKey { fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result { formatter.write_str("SealingKey([REDACTED])") } }
impl Drop for SealingKey { fn drop(&mut self) { self.0.fill(0); } }
