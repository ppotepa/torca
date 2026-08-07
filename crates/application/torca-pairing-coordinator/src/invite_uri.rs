use torca_pairing::PairingCode;

use crate::PairingCoordinatorError;

const PREFIX: &str = "torca://pair?v=1&code=";

/// Encodes a QR-safe invitation URI containing no capability or key material.
pub fn encode_invite_uri(code: &PairingCode) -> String {
    format!("{PREFIX}{}", code.as_str())
}

/// Parses exactly the supported invitation URI shape.
pub fn decode_invite_uri(value: &str) -> Result<PairingCode, PairingCoordinatorError> {
    if value.len() > PREFIX.len() + 16 {
        return Err(PairingCoordinatorError::Protocol);
    }
    let code = value.strip_prefix(PREFIX).ok_or(PairingCoordinatorError::Protocol)?;
    PairingCode::new(code).map_err(|_| PairingCoordinatorError::Protocol)
}
