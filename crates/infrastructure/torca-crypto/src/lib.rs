//! Semantic cryptographic contracts, production RustCrypto adapters and redaction-safe value types.
//!
//! [`RustCryptoProvider`] is the production algorithm implementation. Platform-protected
//! persistence is composed through [`ProtectedSecretStore`] and [`ManagedIdentityKeys`].
//! The deterministic algorithm and in-memory secret store remain test-only.

mod key_management;
mod pairing;
mod pairing_approval;
mod peer_handshake;
mod peer_secrets;
mod ports;
mod production;
mod radio;
mod test_support;
mod types;

pub use key_management::{
    InMemoryProtectedSecretStore, ManagedIdentityKeys, ManagedKeyError, ProtectedSecretStore,
    ProtectedSecretStoreError,
};
pub use pairing::{PairingKeyError, RustPairingCrypto};
pub use peer_handshake::{Ed25519HandshakeVerifier, ManagedHandshakeSigner, OwnedHandshakeSigner};
pub use peer_secrets::{ManagedPeerSecrets, PeerSecretError};
pub use ports::CryptoProvider;
pub use production::RustCryptoProvider;
pub use radio::{RadioCipherError, RadioSessionCipher};
pub use test_support::DeterministicTestCrypto;
pub use types::{
    Ciphertext, CryptoError, Nonce, PublicKey, SealingKey, Signature, SigningSecretKey,
};
