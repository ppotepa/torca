//! Semantic cryptographic contracts, production RustCrypto adapters and redaction-safe value types.
//!
//! [`RustCryptoProvider`] is the production algorithm implementation. The deterministic provider
//! remains available only for tests and simulations. Platform-protected persistence of private
//! keys is intentionally a separate integration boundary.

mod ports;
mod production;
mod test_support;
mod types;

pub use ports::CryptoProvider;
pub use production::RustCryptoProvider;
pub use test_support::DeterministicTestCrypto;
pub use types::{
    Ciphertext, CryptoError, Nonce, PublicKey, SealingKey, Signature, SigningSecretKey,
};
