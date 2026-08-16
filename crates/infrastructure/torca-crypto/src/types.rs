use core::fmt;
use torca_foundation::SecretBytes;

/// Redaction-safe cryptographic failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CryptoError {
    InvalidKey,
    InvalidSignature,
    AuthenticationFailed,
    InvalidNonce,
    RandomnessUnavailable,
    UnsupportedAlgorithm,
    Internal,
}
impl fmt::Display for CryptoError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for CryptoError {}

#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PublicKey(pub [u8; 32]);
#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Signature(pub [u8; 64]);
#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Nonce(pub [u8; 24]);
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Ciphertext(pub Vec<u8>);

/// Private signing key bytes with redacted diagnostics and wipe-on-drop.
#[must_use]
#[derive(Eq, PartialEq)]
pub struct SigningSecretKey(SecretBytes<32>);
impl SigningSecretKey {
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(SecretBytes::new(bytes))
    }
    pub(crate) const fn expose(&self) -> &[u8; 32] {
        self.0.expose()
    }
}
impl fmt::Debug for SigningSecretKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SigningSecretKey([REDACTED])")
    }
}

/// Symmetric authenticated-encryption key with redacted diagnostics and wipe-on-drop.
#[must_use]
#[derive(Eq, PartialEq)]
pub struct SealingKey(SecretBytes<32>);
impl SealingKey {
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(SecretBytes::new(bytes))
    }
    pub(crate) const fn expose(&self) -> &[u8; 32] {
        self.0.expose()
    }
}
impl fmt::Debug for SealingKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SealingKey([REDACTED])")
    }
}
