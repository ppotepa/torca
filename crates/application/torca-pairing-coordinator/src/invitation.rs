use torca_pairing::PairingCode;
use torca_pairing_protocol::PairingInviteTicket;

use crate::{
    PairingCoordinator, PairingCoordinatorError, PairingCryptoPort, PairingSessionServicePort,
};

/// Crockford Base32 avoids visual ambiguity in a code that users may type.
const CODE_ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const GENERATED_CODE_LEN: usize = 6;

impl<R, C> PairingCoordinator<R, C>
where
    R: PairingSessionServicePort,
    C: PairingCryptoPort,
{
    /// Generates a short invitation code without modulo bias. The UI never supplies creator
    /// codes, and all 32 symbols are selected directly from five random bits.
    pub fn generate_pairing_code(&mut self) -> Result<PairingCode, PairingCoordinatorError> {
        let mut output = String::with_capacity(GENERATED_CODE_LEN);
        while output.len() < GENERATED_CODE_LEN {
            let mut byte = [0_u8; 1];
            self.crypto.fill_random(&mut byte)?;
            let index = usize::from(byte[0] & 0b0001_1111);
            output.push(char::from(CODE_ALPHABET[index]));
        }
        PairingCode::new(output).map_err(|_| PairingCoordinatorError::Crypto)
    }

    pub fn generate_pairing_ticket(
        &mut self,
    ) -> Result<PairingInviteTicket, PairingCoordinatorError> {
        let mut bytes = [0_u8; 16];
        self.crypto.fill_random(&mut bytes)?;
        Ok(PairingInviteTicket::new(bytes))
    }
}
