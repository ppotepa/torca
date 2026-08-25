//! Versioned external representation of a pairing invitation code.

use core::fmt::{self, Write};

const PREFIX_V2: &str = "torca://pair?v=2&code=";
const PREFIX_V3: &str = "torca://pair?v=3&code=";
const CODE_LEN: usize = 6;
const CROCKFORD_BASE32: &str = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const MAX_BOOTSTRAP_PAYLOAD_LEN: usize = 2048;
const MAX_BOOTSTRAP_PROVIDER_LEN: usize = 32;

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

/// Provider-specific, QR-safe bootstrap data used only until the pairing
/// session establishes its authenticated durable route. It is intentionally
/// separate from the encrypted pairing offer, which remains the source of
/// truth for the contact's final transport route.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairingBootstrapDescriptor {
    provider: String,
    payload: Vec<u8>,
}

impl PairingBootstrapDescriptor {
    pub fn new(provider: impl Into<String>, payload: Vec<u8>) -> Result<Self, InviteUriError> {
        let provider = provider.into();
        if provider.is_empty()
            || provider.len() > MAX_BOOTSTRAP_PROVIDER_LEN
            || !provider.bytes().all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_'))
        {
            return Err(InviteUriError::InvalidBootstrapProvider);
        }
        if payload.is_empty() || payload.len() > MAX_BOOTSTRAP_PAYLOAD_LEN {
            return Err(InviteUriError::InvalidBootstrapPayload);
        }
        Ok(Self { provider, payload })
    }

    pub fn provider(&self) -> &str {
        &self.provider
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
}

/// Encodes a validated pairing code without capability or key material.
pub fn encode_invite_uri(
    code: &str,
    ticket: Option<&PairingInviteTicket>,
) -> Result<String, InviteUriError> {
    validate_code(code)?;
    encode_invite_uri_with_bootstrap(code, ticket, None)
}

/// Encodes an invitation with optional provider-specific bootstrap data.
/// Invitations without it retain the v2 URI form for compatibility.
pub fn encode_invite_uri_with_bootstrap(
    code: &str,
    ticket: Option<&PairingInviteTicket>,
    bootstrap: Option<&PairingBootstrapDescriptor>,
) -> Result<String, InviteUriError> {
    validate_code(code)?;
    let mut uri = format!("{}{}", if bootstrap.is_some() { PREFIX_V3 } else { PREFIX_V2 }, code);
    if let Some(ticket) = ticket {
        uri.push_str("&ticket=");
        for byte in ticket.as_bytes() {
            let _ = write!(uri, "{byte:02x}");
        }
    }
    if let Some(bootstrap) = bootstrap {
        uri.push_str("&provider=");
        uri.push_str(bootstrap.provider());
        uri.push_str("&bootstrap=");
        for byte in bootstrap.payload() {
            let _ = write!(uri, "{byte:02x}");
        }
    }
    Ok(uri)
}

/// Parses exactly the currently supported pairing invitation URI.
pub fn decode_invite_uri(
    value: &str,
) -> Result<(PairingInviteCode, Option<PairingInviteTicket>), InviteUriError> {
    let (code, ticket, _) = decode_invite_uri_with_bootstrap(value)?;
    Ok((code, ticket))
}

/// Decodes v2 invitations and v3 invitations carrying a bootstrap descriptor.
pub fn decode_invite_uri_with_bootstrap(
    value: &str,
) -> Result<
    (PairingInviteCode, Option<PairingInviteTicket>, Option<PairingBootstrapDescriptor>),
    InviteUriError,
> {
    let (query, supports_bootstrap) = if let Some(query) = value.strip_prefix(PREFIX_V2) {
        (query, false)
    } else if let Some(query) = value.strip_prefix(PREFIX_V3) {
        (query, true)
    } else {
        return Err(InviteUriError::InvalidFormat);
    };
    let mut segments = query.split('&');
    let code = segments.next().ok_or(InviteUriError::InvalidFormat)?;
    validate_code(code)?;
    let mut ticket = None;
    let mut provider = None;
    let mut payload = None;
    for segment in segments {
        let (key, value) = segment.split_once('=').ok_or(InviteUriError::InvalidFormat)?;
        match key {
            "ticket" if ticket.is_none() => ticket = Some(parse_ticket(value)?),
            "provider" if supports_bootstrap && provider.is_none() => provider = Some(value),
            "bootstrap" if supports_bootstrap && payload.is_none() => {
                payload = Some(parse_hex_payload(value)?)
            }
            _ => return Err(InviteUriError::InvalidFormat),
        }
    }
    let bootstrap = match (provider, payload) {
        (None, None) => None,
        (Some(provider), Some(payload)) => {
            Some(PairingBootstrapDescriptor::new(provider, payload)?)
        }
        _ => return Err(InviteUriError::InvalidFormat),
    };
    Ok((PairingInviteCode(code.to_owned()), ticket, bootstrap))
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

fn parse_hex_payload(value: &str) -> Result<Vec<u8>, InviteUriError> {
    if value.is_empty() || value.len() % 2 != 0 || value.len() / 2 > MAX_BOOTSTRAP_PAYLOAD_LEN {
        return Err(InviteUriError::InvalidBootstrapPayload);
    }
    let mut payload = Vec::with_capacity(value.len() / 2);
    for chunk in value.as_bytes().chunks_exact(2) {
        let high =
            (chunk[0] as char).to_digit(16).ok_or(InviteUriError::InvalidBootstrapPayload)?;
        let low = (chunk[1] as char).to_digit(16).ok_or(InviteUriError::InvalidBootstrapPayload)?;
        payload.push(((high << 4) | low) as u8);
    }
    Ok(payload)
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
    InvalidBootstrapProvider,
    InvalidBootstrapPayload,
}

impl fmt::Display for InviteUriError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for InviteUriError {}

#[cfg(test)]
mod tests {
    use super::{
        InviteUriError, PairingBootstrapDescriptor, PairingInviteTicket, decode_invite_uri,
        decode_invite_uri_with_bootstrap, encode_invite_uri, encode_invite_uri_with_bootstrap,
    };

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

    #[test]
    fn v3_uri_round_trips_provider_bootstrap_without_changing_v2_api() {
        let bootstrap = PairingBootstrapDescriptor::new("iroh", vec![1, 2, 3]).expect("descriptor");
        let uri =
            encode_invite_uri_with_bootstrap("ABC123", None, Some(&bootstrap)).expect("encode");
        let (code, ticket, decoded) = decode_invite_uri_with_bootstrap(&uri).expect("decode v3");
        assert_eq!(code.as_str(), "ABC123");
        assert_eq!(ticket, None);
        assert_eq!(decoded, Some(bootstrap));
        assert_eq!(decode_invite_uri(&uri).expect("legacy decode").0.as_str(), "ABC123");
    }
}
