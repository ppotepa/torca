//! Semantic cryptographic contracts and redaction-safe value types.
//!
//! The deterministic provider exported here is for tests only and is not cryptographically secure.
//! A reviewed production provider must be supplied before a distributable 0.1 build.

mod ports;
mod test_support;
mod types;

pub use ports::CryptoProvider;
pub use test_support::DeterministicTestCrypto;
pub use types::{
    Ciphertext, CryptoError, Nonce, PublicKey, SealingKey, Signature, SigningSecretKey,
};
