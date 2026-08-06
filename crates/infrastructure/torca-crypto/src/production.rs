use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use ed25519_dalek::{
    Signature as DalekSignature, Signer, SigningKey, Verifier, VerifyingKey,
};
use rand_core::{OsRng, RngCore};

use crate::{
    Ciphertext, CryptoError, CryptoProvider, Nonce, PublicKey, SealingKey, Signature,
    SigningSecretKey,
};

/// Production cryptographic provider backed by audited RustCrypto implementations.
///
/// Signing uses Ed25519. Authenticated sealing uses XChaCha20-Poly1305 with a
/// 192-bit nonce. Random bytes are obtained from the operating system CSPRNG.
#[derive(Clone, Copy, Debug, Default)]
pub struct RustCryptoProvider;

impl CryptoProvider for RustCryptoProvider {
    fn generate_signing_key(&mut self) -> Result<(SigningSecretKey, PublicKey), CryptoError> {
        let mut secret = [0_u8; 32];
        self.fill_random(&mut secret)?;
        let signing_key = SigningKey::from_bytes(&secret);
        let public_key = signing_key.verifying_key().to_bytes();
        Ok((SigningSecretKey::new(secret), PublicKey(public_key)))
    }

    fn generate_sealing_key(&mut self) -> Result<SealingKey, CryptoError> {
        let mut key = [0_u8; 32];
        self.fill_random(&mut key)?;
        Ok(SealingKey::new(key))
    }

    fn sign(
        &self,
        secret: &SigningSecretKey,
        message: &[u8],
    ) -> Result<Signature, CryptoError> {
        let signing_key = SigningKey::from_bytes(secret.expose());
        Ok(Signature(signing_key.sign(message).to_bytes()))
    }

    fn verify(
        &self,
        public: &PublicKey,
        message: &[u8],
        signature: &Signature,
    ) -> Result<(), CryptoError> {
        let verifying_key =
            VerifyingKey::from_bytes(&public.0).map_err(|_| CryptoError::InvalidKey)?;
        let signature = DalekSignature::from_bytes(&signature.0);
        verifying_key
            .verify_strict(message, &signature)
            .map_err(|_| CryptoError::InvalidSignature)
    }

    fn fill_random(&mut self, output: &mut [u8]) -> Result<(), CryptoError> {
        let mut rng = OsRng;
        rng.try_fill_bytes(output)
            .map_err(|_| CryptoError::RandomnessUnavailable)
    }

    fn seal(
        &self,
        key: &SealingKey,
        nonce: Nonce,
        associated_data: &[u8],
        plaintext: &[u8],
    ) -> Result<Ciphertext, CryptoError> {
        let cipher = XChaCha20Poly1305::new_from_slice(key.expose())
            .map_err(|_| CryptoError::InvalidKey)?;
        cipher
            .encrypt(
                XNonce::from_slice(&nonce.0),
                Payload {
                    msg: plaintext,
                    aad: associated_data,
                },
            )
            .map(Ciphertext)
            .map_err(|_| CryptoError::Internal)
    }

    fn open(
        &self,
        key: &SealingKey,
        nonce: Nonce,
        associated_data: &[u8],
        ciphertext: &Ciphertext,
    ) -> Result<Vec<u8>, CryptoError> {
        let cipher = XChaCha20Poly1305::new_from_slice(key.expose())
            .map_err(|_| CryptoError::InvalidKey)?;
        cipher
            .decrypt(
                XNonce::from_slice(&nonce.0),
                Payload {
                    msg: &ciphertext.0,
                    aad: associated_data,
                },
            )
            .map_err(|_| CryptoError::AuthenticationFailed)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        Ciphertext, CryptoProvider, Nonce, PublicKey, RustCryptoProvider, SealingKey, Signature,
        SigningSecretKey,
    };

    fn decode_hex(value: &str) -> Vec<u8> {
        assert_eq!(value.len() % 2, 0);
        value
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| {
                let high = (pair[0] as char).to_digit(16).expect("hex") as u8;
                let low = (pair[1] as char).to_digit(16).expect("hex") as u8;
                (high << 4) | low
            })
            .collect()
    }

    #[test]
    fn matches_rfc8032_ed25519_empty_message_vector() {
        let secret: [u8; 32] = decode_hex(
            "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60",
        )
        .try_into()
        .expect("secret length");
        let expected_public: [u8; 32] = decode_hex(
            "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a",
        )
        .try_into()
        .expect("public length");
        let expected_signature: [u8; 64] = decode_hex(concat!(
            "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e06522490155",
            "5fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b"
        ))
        .try_into()
        .expect("signature length");

        let provider = RustCryptoProvider;
        let signing_key = SigningSecretKey::new(secret);
        let signature = provider.sign(&signing_key, b"").expect("sign");

        assert_eq!(signature, Signature(expected_signature));
        provider
            .verify(&PublicKey(expected_public), b"", &signature)
            .expect("verify");
    }

    #[test]
    fn matches_xchacha20_poly1305_draft_vector() {
        let key: [u8; 32] = decode_hex(
            "808182838485868788898a8b8c8d8e8f909192939495969798999a9b9c9d9e9f",
        )
        .try_into()
        .expect("key length");
        let nonce: [u8; 24] = decode_hex(
            "404142434445464748494a4b4c4d4e4f5051525354555657",
        )
        .try_into()
        .expect("nonce length");
        let aad = decode_hex("50515253c0c1c2c3c4c5c6c7");
        let plaintext = b"Ladies and Gentlemen of the class of '99: If I could offer you only one tip for the future, sunscreen would be it.";
        let expected = decode_hex(concat!(
            "bd6d179d3e83d43b9576579493c0e939572a1700252bfaccbed2902c21396cbb",
            "731c7f1b0b4aa6440bf3a82f4eda7e39ae64c6708c54c216cb96b72e1213b452",
            "2f8c9ba40db5d945b11b69b982c1bb9e3f3fac2bc369488f76b2383565d3fff9",
            "21f9664c97637da9768812f615c68b13b52ec0875924c1c7987947deafd8780acf49"
        ));

        let provider = RustCryptoProvider;
        let sealing_key = SealingKey::new(key);
        let ciphertext = provider
            .seal(&sealing_key, Nonce(nonce), &aad, plaintext)
            .expect("seal");

        assert_eq!(ciphertext, Ciphertext(expected));
        assert_eq!(
            provider
                .open(&sealing_key, Nonce(nonce), &aad, &ciphertext)
                .expect("open"),
            plaintext
        );
    }

    #[test]
    fn rejects_modified_ciphertext() {
        let provider = RustCryptoProvider;
        let key = SealingKey::new([7; 32]);
        let nonce = Nonce([9; 24]);
        let mut ciphertext = provider
            .seal(&key, nonce, b"context", b"payload")
            .expect("seal");
        ciphertext.0[0] ^= 1;

        assert!(provider.open(&key, nonce, b"context", &ciphertext).is_err());
    }
}
