use core::{fmt, str::FromStr};

/// Validated, provider-neutral identifier used at plugin and route boundaries.
///
/// Provider identifiers contain 1–32 lowercase ASCII letters, digits, `-` or
/// `_`. The owned representation keeps persisted routes independent of a
/// compile-time registry.
#[must_use]
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProviderId(String);

impl ProviderId {
    pub const MAX_LEN: usize = 32;

    pub fn new(value: impl Into<String>) -> Result<Self, InvalidProviderId> {
        let value = value.into();
        if value.is_empty()
            || value.len() > Self::MAX_LEN
            || !value.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"-_".contains(&byte)
            })
        {
            return Err(InvalidProviderId);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl AsRef<str> for ProviderId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for ProviderId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for ProviderId {
    type Err = InvalidProviderId;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidProviderId;

impl fmt::Display for InvalidProviderId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("invalid provider identifier")
    }
}

impl std::error::Error for InvalidProviderId {}

#[cfg(test)]
mod tests {
    use super::ProviderId;

    #[test]
    fn accepts_the_stable_provider_alphabet() {
        assert_eq!(ProviderId::new("iroh").expect("valid").as_str(), "iroh");
        assert!(ProviderId::new("provider_2-alpha").is_ok());
    }

    #[test]
    fn rejects_empty_long_uppercase_and_punctuation() {
        for invalid in ["", "Tor", "with.dot", "with space", "abcdefghijklmnopqrstuvwxyz1234567"] {
            assert!(ProviderId::new(invalid).is_err(), "accepted {invalid:?}");
        }
    }
}
