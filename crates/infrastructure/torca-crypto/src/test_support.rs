use crate::{Ciphertext, CryptoError, CryptoProvider, Nonce, PublicKey, SecretKey, Signature};

/// Deterministic, deliberately insecure provider for tests and simulations only.
#[derive(Clone, Debug)]
pub struct DeterministicTestCrypto { state: u64 }
impl Default for DeterministicTestCrypto { fn default() -> Self { Self { state: 0x544f_5243_4154_4553 } } }

impl DeterministicTestCrypto {
    fn digest(parts: &[&[u8]], output: &mut [u8]) {
        let mut state = 0xcbf2_9ce4_8422_2325_u64;
        for part in parts {
            for byte in *part { state ^= u64::from(*byte); state = state.wrapping_mul(0x0000_0100_0000_01b3); }
        }
        for (index, byte) in output.iter_mut().enumerate() {
            state ^= (index as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
            state = state.rotate_left(13).wrapping_mul(0xff51_afd7_ed55_8ccd);
            *byte = state as u8;
        }
    }
}

impl CryptoProvider for DeterministicTestCrypto {
    fn generate_signing_key(&mut self) -> Result<(SecretKey, PublicKey), CryptoError> {
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let mut secret = [0_u8; 32];
        Self::digest(&[&self.state.to_be_bytes()], &mut secret);
        let mut public = [0_u8; 32];
        Self::digest(&[&secret], &mut public);
        Ok((SecretKey::new(secret), PublicKey(public)))
    }
    fn sign(&self, secret: &SecretKey, message: &[u8]) -> Result<Signature, CryptoError> {
        let mut public = [0_u8; 32];
        Self::digest(&[secret.expose()], &mut public);
        let mut signature = [0_u8; 64];
        Self::digest(&[&public, message], &mut signature);
        Ok(Signature(signature))
    }
    fn verify(&self, public: &PublicKey, message: &[u8], signature: &Signature) -> Result<(), CryptoError> {
        let mut expected = [0_u8; 64];
        Self::digest(&[&public.0, message], &mut expected);
        if expected == signature.0 { Ok(()) } else { Err(CryptoError::InvalidSignature) }
    }
    fn fill_random(&mut self, output: &mut [u8]) -> Result<(), CryptoError> {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        Self::digest(&[&self.state.to_be_bytes()], output);
        Ok(())
    }
    fn seal(&self, key: &SecretKey, nonce: Nonce, associated_data: &[u8], plaintext: &[u8]) -> Result<Ciphertext, CryptoError> {
        let mut stream = vec![0_u8; plaintext.len()];
        Self::digest(&[key.expose(), &nonce.0, associated_data], &mut stream);
        let mut output: Vec<u8> = plaintext.iter().zip(stream).map(|(plain, mask)| plain ^ mask).collect();
        let mut tag = [0_u8; 16];
        Self::digest(&[key.expose(), &nonce.0, associated_data, &output], &mut tag);
        output.extend_from_slice(&tag);
        Ok(Ciphertext(output))
    }
    fn open(&self, key: &SecretKey, nonce: Nonce, associated_data: &[u8], ciphertext: &Ciphertext) -> Result<Vec<u8>, CryptoError> {
        if ciphertext.0.len() < 16 { return Err(CryptoError::AuthenticationFailed); }
        let split = ciphertext.0.len() - 16;
        let (body, tag) = ciphertext.0.split_at(split);
        let mut expected = [0_u8; 16];
        Self::digest(&[key.expose(), &nonce.0, associated_data, body], &mut expected);
        if expected != tag { return Err(CryptoError::AuthenticationFailed); }
        let mut stream = vec![0_u8; body.len()];
        Self::digest(&[key.expose(), &nonce.0, associated_data], &mut stream);
        Ok(body.iter().zip(stream).map(|(byte, mask)| byte ^ mask).collect())
    }
}

#[cfg(test)]
mod tests {
    use crate::{CryptoProvider, DeterministicTestCrypto, Nonce};
    #[test]
    fn test_provider_round_trips_and_authenticates() {
        let mut provider = DeterministicTestCrypto::default();
        let (secret, public) = provider.generate_signing_key().expect("key generation");
        let signature = provider.sign(&secret, b"message").expect("sign");
        provider.verify(&public, b"message", &signature).expect("verify");
        let ciphertext = provider.seal(&secret, Nonce([1; 24]), b"ad", b"hello").expect("seal");
        assert_eq!(provider.open(&secret, Nonce([1; 24]), b"ad", &ciphertext).expect("open"), b"hello");
    }
}
