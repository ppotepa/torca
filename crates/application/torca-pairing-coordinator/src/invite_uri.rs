use torca_pairing::PairingCode;
use torca_pairing_protocol::{
    PairingBootstrapDescriptor, PairingInviteTicket,
    decode_invite_uri as decode_protocol_invite_uri,
    decode_invite_uri_with_bootstrap as decode_protocol_invite_uri_with_bootstrap,
    encode_invite_uri as encode_protocol_invite_uri,
    encode_invite_uri_with_bootstrap as encode_protocol_invite_uri_with_bootstrap,
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

pub fn encode_invite_uri_with_bootstrap(
    code: &PairingCode,
    ticket: Option<&PairingInviteTicket>,
    bootstrap: Option<&PairingBootstrapDescriptor>,
) -> Result<String, PairingCoordinatorError> {
    encode_protocol_invite_uri_with_bootstrap(code.as_str(), ticket, bootstrap)
        .map_err(|_| PairingCoordinatorError::Protocol)
}

/// Parses exactly the supported invitation URI shape.
pub fn decode_invite_uri(
    value: &str,
) -> Result<(PairingCode, Option<PairingInviteTicket>), PairingCoordinatorError> {
    let (code, ticket) =
        decode_protocol_invite_uri(value).map_err(|_| PairingCoordinatorError::Protocol)?;
    Ok((PairingCode::new(code.as_str()).map_err(|_| PairingCoordinatorError::Protocol)?, ticket))
}

pub fn decode_invite_uri_with_bootstrap(
    value: &str,
) -> Result<
    (PairingCode, Option<PairingInviteTicket>, Option<PairingBootstrapDescriptor>),
    PairingCoordinatorError,
> {
    let (code, ticket, bootstrap) = decode_protocol_invite_uri_with_bootstrap(value)
        .map_err(|_| PairingCoordinatorError::Protocol)?;
    Ok((
        PairingCode::new(code.as_str()).map_err(|_| PairingCoordinatorError::Protocol)?,
        ticket,
        bootstrap,
    ))
}
