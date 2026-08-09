use torca_pairing::PairingCode;

use crate::{
    PairingCoordinator, PairingCoordinatorError, PairingCryptoPort, PairingRendezvousPort,
};

const CODE_ALPHABET: &[u8; 36] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
const GENERATED_CODE_LEN: usize = 8;
const UNBIASED_BYTE_LIMIT: u8 = 252;

impl<R, C> PairingCoordinator<R, C>
where
    R: PairingRendezvousPort,
    C: PairingCryptoPort,
{
    /// Generates a short invitation code with rejection sampling so modulo reduction does not
    /// introduce alphabet bias. The UI never supplies creator codes.
    pub fn generate_pairing_code(&mut self) -> Result<PairingCode, PairingCoordinatorError> {
        let mut output = String::with_capacity(GENERATED_CODE_LEN);
        while output.len() < GENERATED_CODE_LEN {
            let mut byte = [0_u8; 1];
            self.crypto.fill_random(&mut byte)?;
            if byte[0] >= UNBIASED_BYTE_LIMIT {
                continue;
            }
            let index = usize::from(byte[0] % 36);
            output.push(char::from(CODE_ALPHABET[index]));
        }
        PairingCode::new(output).map_err(|_| PairingCoordinatorError::Crypto)
    }
}
