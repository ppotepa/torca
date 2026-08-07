use core::fmt;
use std::collections::BTreeMap;

use hkdf::Hkdf;
use rand_core::OsRng;
use sha2::Sha256;
use torca_foundation::OpaqueId;
use torca_pairing_coordinator::{
    PairingCoordinatorError, PairingCryptoHandle, PairingCryptoPort, PairingEphemeralKey,
};
use x25519_dalek::{PublicKey as X25519PublicKey, ReusableSecret};

use crate::{Ciphertext, CryptoError, CryptoProvider, Nonce, RustCryptoProvider, SealingKey};

const PAIRING_KDF_LABEL: &[u8] = b"TORCA-PAIRING-SEAL-V1";

pub struct RustPairingCrypto {
    crypto: RustCryptoProvider,
    keys: BTreeMap<PairingCryptoHandle, ReusableSecret>,
}

impl Default for RustPairingCrypto {
    fn default() -> Self {
        Self { crypto: RustCryptoProvider, keys: BTreeMap::new() }
    }
}

impl RustPairingCrypto {
    pub const fn new() -> Self {
        Self { crypto: RustCryptoProvider, keys: BTreeMap::new() }
    }

    pub fn generate_key(&mut self) -> Result<PairingEphemeralKey, PairingKeyError> {
        let handle = self.new_handle()?;
        let mut rng = OsRng;
        let secret = ReusableSecret::random_from_rng(&mut rng);
        let public_key = X25519PublicKey::from(&secret).to_bytes();
        self.keys.insert(handle, secret);
        Ok(PairingEphemeralKey { handle, public_key })
    }

    pub fn release_key(&mut self, handle: PairingCryptoHandle) -> Result<(), PairingKeyError> {
        self.keys.remove(&handle).map(|_| ()).ok_or(PairingKeyError::NotFound)
    }

    pub fn fill_random(&mut self, output: &mut [u8]) -> Result<(), PairingKeyError> {
        self.crypto.fill_random(output).map_err(PairingKeyError::Crypto)
    }

    pub fn seal_pairing(
        &self,
        handle: PairingCryptoHandle,
        remote_public_key: [u8; 32],
        nonce: [u8; 24],
        associated_data: &[u8],
        plaintext: &[u8],
    ) -> Result<Vec<u8>, PairingKeyError> {
        let key = self.derive_key(handle, remote_public_key, associated_data, PAIRING_KDF_LABEL)?;
        self.crypto
            .seal(&key, Nonce(nonce), associated_data, plaintext)
            .map(|ciphertext| ciphertext.0)
            .map_err(PairingKeyError::Crypto)
    }

    pub fn open_pairing(
        &self,
        handle: PairingCryptoHandle,
        remote_public_key: [u8; 32],
        nonce: [u8; 24],
        associated_data: &[u8],
        ciphertext: &[u8],
    ) -> Result<Vec<u8>, PairingKeyError> {
        let key = self.derive_key(handle, remote_public_key, associated_data, PAIRING_KDF_LABEL)?;
        self.crypto
            .open(&key, Nonce(nonce), associated_data, &Ciphertext(ciphertext.to_vec()))
            .map_err(PairingKeyError::Crypto)
    }

    pub(crate) fn derive_key(
        &self,
        handle: PairingCryptoHandle,
        remote_public_key: [u8; 32],
        salt: &[u8],
        label: &[u8],
    ) -> Result<SealingKey, PairingKeyError> {
        let secret = self.keys.get(&handle).ok_or(PairingKeyError::NotFound)?;
        let remote = X25519PublicKey::from(remote_public_key);
        let shared = secret.diffie_hellman(&remote);
        if !shared.was_contributory() {
            return Err(PairingKeyError::NonContributoryKey);
        }
        let local_public = X25519PublicKey::from(secret).to_bytes();
        let (first, second) = if local_public <= remote_public_key {
            (local_public, remote_public_key)
        } else {
            (remote_public_key, local_public)
        };
        let mut info = Vec::with_capacity(label.len() + 64);
        info.extend_from_slice(label);
        info.extend_from_slice(&first);
        info.extend_from_slice(&second);
        let hkdf = Hkdf::<Sha256>::new(Some(salt), shared.as_bytes());
        let mut output = [0_u8; 32];
        hkdf.expand(&info, &mut output).map_err(|_| PairingKeyError::Kdf)?;
        Ok(SealingKey::new(output))
    }

    fn new_handle(&mut self) -> Result<PairingCryptoHandle, PairingKeyError> {
        for _ in 0..8 {
            let mut bytes = [0_u8; 16];
            self.crypto.fill_random(&mut bytes).map_err(PairingKeyError::Crypto)?;
            let opaque = OpaqueId::from_bytes(bytes);
            let handle = PairingCryptoHandle(opaque);
            if !opaque.is_nil() && !self.keys.contains_key(&handle) {
                return Ok(handle);
            }
        }
        Err(PairingKeyError::IdentifierUnavailable)
    }
}

impl PairingCryptoPort for RustPairingCrypto {
    fn generate_ephemeral_key(&mut self) -> Result<PairingEphemeralKey, PairingCoordinatorError> {
        self.generate_key().map_err(|_| PairingCoordinatorError::Crypto)
    }

    fn release_ephemeral_key(
        &mut self,
        handle: PairingCryptoHandle,
    ) -> Result<(), PairingCoordinatorError> {
        self.release_key(handle).map_err(|_| PairingCoordinatorError::Crypto)
    }

    fn fill_random(&mut self, output: &mut [u8]) -> Result<(), PairingCoordinatorError> {
        RustPairingCrypto::fill_random(self, output).map_err(|_| PairingCoordinatorError::Crypto)
    }

    fn seal_for_peer(
        &self,
        local_key: PairingCryptoHandle,
        remote_public_key: [u8; 32],
        nonce: [u8; 24],
        associated_data: &[u8],
        plaintext: &[u8],
    ) -> Result<Vec<u8>, PairingCoordinatorError> {
        self.seal_pairing(
            local_key,
            remote_public_key,
            nonce,
            associated_data,
            plaintext,
        )
        .map_err(|_| PairingCoordinatorError::Crypto)
    }

    fn open_from_peer(
        &self,
        local_key: PairingCryptoHandle,
        remote_public_key: [u8; 32],
        nonce: [u8; 24],
        associated_data: &[u8],
        ciphertext: &[u8],
    ) -> Result<Vec<u8>, PairingCoordinatorError> {
        self.open_pairing(
            local_key,
            remote_public_key,
            nonce,
            associated_data,
            ciphertext,
        )
        .map_err(|_| PairingCoordinatorError::Crypto)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PairingKeyError {
    NotFound,
    IdentifierUnavailable,
    NonContributoryKey,
    Kdf,
    Crypto(CryptoError),
}
impl fmt::Display for PairingKeyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for PairingKeyError {}
