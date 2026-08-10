//! Versioned external representation of a pairing invitation code.

use core::fmt::{self, Write};

const PREFIX: &str = "torca://pair?v=2&code=";
const CODE_LEN: usize = 6;
const CROCKFORD_BASE32: &str = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// QR-safe invitation code parsed from the versioned URI representation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairingInviteCode(String);

impl PairingInviteCode {
    /// Returns the validated code text for conversion to a domain pairing code.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PairingInviteTicket([u8; 16]);

impl PairingInviteTicket {
    pub const fn new(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
    pub fn as_hex(&self) -> String {
        self.0.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}

/// Encodes a validated pairing code without capability or key material.
pub fn encode_invite_uri(
    code: &str,
    ticket: Option<&PairingInviteTicket>,
) -> Result<String, InviteUriError> {
    validate_code(code)?;
    let mut uri = format!("{PREFIX}{code}");
    if let Some(ticket) = ticket {
        uri.push_str("&ticket=");
        for byte in ticket.as_bytes() {
            let _ = write!(uri, "{byte:02x}");
        }
    }
    Ok(uri)
}

/// Parses exactly the currently supported pairing invitation URI.
pub fn decode_invite_uri(
    value: &str,
) -> Result<(PairingInviteCode, Option<PairingInviteTicket>), InviteUriError> {
    let query = value.strip_prefix(PREFIX).ok_or(InviteUriError::InvalidFormat)?;
    let (code, ticket) = match query.split_once("&ticket=") {
        Some((code, hex)) => (code, Some(parse_ticket(hex)?)),
        None => (query, None),
    };
    validate_code(code)?;
    Ok((PairingInviteCode(code.to_owned()), ticket))
}

fn parse_ticket(value: &str) -> Result<PairingInviteTicket, InviteUriError> {
    if value.len() != 32 {
        return Err(InviteUriError::InvalidTicket);
    }
    let mut bytes = [0_u8; 16];
    for (index, chunk) in value.as_bytes().chunks_exact(2).enumerate() {
        let high = (chunk[0] as char).to_digit(16).ok_or(InviteUriError::InvalidTicket)?;
        let low = (chunk[1] as char).to_digit(16).ok_or(InviteUriError::InvalidTicket)?;
        bytes[index] = ((high << 4) | low) as u8;
    }
    Ok(PairingInviteTicket(bytes))
}

fn validate_code(code: &str) -> Result<(), InviteUriError> {
    if code.len() != CODE_LEN || !code.chars().all(|character| CROCKFORD_BASE32.contains(character))
    {
        return Err(InviteUriError::InvalidCode);
    }
    Ok(())
}

/// Safe error returned when an external pairing URI is unsupported or malformed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InviteUriError {
    InvalidFormat,
    InvalidCode,
    InvalidTicket,
}

impl fmt::Display for InviteUriError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for InviteUriError {}

#[cfg(test)]
mod tests {
    use super::{InviteUriError, PairingInviteTicket, decode_invite_uri, encode_invite_uri};

    #[test]
    fn invitation_uri_round_trips() {
        let ticket = PairingInviteTicket::new([7; 16]);
        let uri = encode_invite_uri("ABC123", Some(&ticket)).expect("encode");
        let (code, parsed) = decode_invite_uri(&uri).expect("decode");
        assert_eq!(code.as_str(), "ABC123");
        assert_eq!(parsed, Some(ticket));
    }

    #[test]
    fn invitation_uri_rejects_unknown_shapes() {
        assert_eq!(
            decode_invite_uri("torca://pair?code=ABC123"),
            Err(InviteUriError::InvalidFormat)
        );
        assert_eq!(
            decode_invite_uri("torca://pair?v=2&code=bad!"),
            Err(InviteUriError::InvalidCode)
        );
    }
}
