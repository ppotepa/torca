use hkdf::Hkdf;
use sha2::Sha256;
use torca_foundation::OpaqueId;

use crate::{Ciphertext, CryptoProvider, Nonce, SealingKey};

const RADIO_KEY_CONTEXT: &[u8] = b"torca-radio-media/v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RadioCipherError {
    Derivation,
    Encryption,
    Authentication,
}

impl core::fmt::Display for RadioCipherError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for RadioCipherError {}

/// Ephemeral directional media cipher. Pairwise secret bytes are used only
/// during construction; this object retains independently derived session
/// keys and zeroes them through `SealingKey::drop`.
pub struct RadioSessionCipher<C> {
    crypto: C,
    transmit: SealingKey,
    receive: SealingKey,
}

impl<C> core::fmt::Debug for RadioSessionCipher<C> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("RadioSessionCipher([REDACTED])")
    }
}

impl<C> RadioSessionCipher<C>
where
    C: CryptoProvider,
{
    pub(crate) fn derive(
        crypto: C,
        pairwise_key: &SealingKey,
        session_id: OpaqueId,
        media_token: &[u8; 32],
        local_identity: OpaqueId,
        remote_identity: OpaqueId,
    ) -> Result<Self, RadioCipherError> {
        if local_identity == remote_identity {
            return Err(RadioCipherError::Derivation);
        }
        let (lower, upper, local_is_lower) = if local_identity < remote_identity {
            (local_identity, remote_identity, true)
        } else {
            (remote_identity, local_identity, false)
        };
        let mut info = Vec::with_capacity(RADIO_KEY_CONTEXT.len() + OpaqueId::BYTE_LEN * 3);
        info.extend_from_slice(RADIO_KEY_CONTEXT);
        info.extend_from_slice(session_id.as_bytes());
        info.extend_from_slice(lower.as_bytes());
        info.extend_from_slice(upper.as_bytes());
        let hkdf = Hkdf::<Sha256>::new(Some(media_token), pairwise_key.expose());
        let mut directional = [0_u8; 64];
        hkdf.expand(&info, &mut directional).map_err(|_| RadioCipherError::Derivation)?;
        let mut lower_to_upper = [0_u8; 32];
        lower_to_upper.copy_from_slice(&directional[..32]);
        let mut upper_to_lower = [0_u8; 32];
        upper_to_lower.copy_from_slice(&directional[32..]);
        directional.fill(0);
        let (transmit, receive) = if local_is_lower {
            (lower_to_upper, upper_to_lower)
        } else {
            (upper_to_lower, lower_to_upper)
        };
        Ok(Self { crypto, transmit: SealingKey::new(transmit), receive: SealingKey::new(receive) })
    }

    pub fn seal(
        &self,
        nonce: Nonce,
        associated_data: &[u8],
        plaintext: &[u8],
    ) -> Result<Ciphertext, RadioCipherError> {
        self.crypto
            .seal(&self.transmit, nonce, associated_data, plaintext)
            .map_err(|_| RadioCipherError::Encryption)
    }

    pub fn open(
        &self,
        nonce: Nonce,
        associated_data: &[u8],
        ciphertext: &Ciphertext,
    ) -> Result<Vec<u8>, RadioCipherError> {
        self.crypto
            .open(&self.receive, nonce, associated_data, ciphertext)
            .map_err(|_| RadioCipherError::Authentication)
    }
}

#[cfg(test)]
mod tests {
    use torca_foundation::OpaqueId;
    use torca_identity::KeyId;

    use crate::{
        DeterministicTestCrypto, InMemoryProtectedSecretStore, ManagedPeerSecrets, Nonce,
        ProtectedSecretStore,
    };

    fn manager()
    -> (ManagedPeerSecrets<DeterministicTestCrypto, InMemoryProtectedSecretStore>, OpaqueId) {
        let handle = OpaqueId::from_u128(90);
        let mut store = InMemoryProtectedSecretStore::default();
        store.insert(KeyId::from_opaque(handle), &[7; 32]).expect("store secret");
        (ManagedPeerSecrets::new(DeterministicTestCrypto::default(), store), handle)
    }

    #[test]
    fn opposite_peers_derive_complementary_directional_keys() {
        let (lower_manager, handle) = manager();
        let (upper_manager, _) = manager();
        let lower = OpaqueId::from_u128(1);
        let upper = OpaqueId::from_u128(2);
        let session = OpaqueId::from_u128(3);
        let token = [4; 32];
        let lower_cipher = lower_manager
            .derive_radio_session_cipher(handle, session, &token, lower, upper)
            .expect("lower cipher");
        let upper_cipher = upper_manager
            .derive_radio_session_cipher(handle, session, &token, upper, lower)
            .expect("upper cipher");
        let nonce = Nonce([5; 24]);
        let sealed = lower_cipher.seal(nonce, b"radio-frame", b"voice").expect("seal");
        assert_eq!(upper_cipher.open(nonce, b"radio-frame", &sealed).expect("open"), b"voice");
    }

    #[test]
    fn session_changes_are_not_interoperable() {
        let (first_manager, handle) = manager();
        let (second_manager, _) = manager();
        let first = first_manager
            .derive_radio_session_cipher(
                handle,
                OpaqueId::from_u128(3),
                &[4; 32],
                OpaqueId::from_u128(1),
                OpaqueId::from_u128(2),
            )
            .expect("first");
        let second = second_manager
            .derive_radio_session_cipher(
                handle,
                OpaqueId::from_u128(4),
                &[4; 32],
                OpaqueId::from_u128(2),
                OpaqueId::from_u128(1),
            )
            .expect("second");
        let nonce = Nonce([5; 24]);
        let ciphertext = first.seal(nonce, b"frame", b"voice").expect("seal");
        assert!(second.open(nonce, b"frame", &ciphertext).is_err());
    }
}
