use torca_pairing::PairingCode;
use torca_pairing_protocol::{
    PairingInviteTicket, decode_invite_uri as decode_protocol_invite_uri,
    encode_invite_uri as encode_protocol_invite_uri,
};

use crate::PairingCoordinatorError;

/// Encodes a QR-safe invitation URI containing no capability or key material.
///
/// # Panics
///
/// Panics only if the pairing domain accepts a code that the matching protocol rejects.
pub fn encode_invite_uri(code: &PairingCode, ticket: Option<&PairingInviteTicket>) -> String {
    encode_protocol_invite_uri(code.as_str(), ticket)
        .expect("domain pairing code is valid protocol input")
}

/// Parses exactly the supported invitation URI shape.
pub fn decode_invite_uri(
    value: &str,
) -> Result<(PairingCode, Option<PairingInviteTicket>), PairingCoordinatorError> {
    let (code, ticket) =
        decode_protocol_invite_uri(value).map_err(|_| PairingCoordinatorError::Protocol)?;
    Ok((PairingCode::new(code.as_str()).map_err(|_| PairingCoordinatorError::Protocol)?, ticket))
}
