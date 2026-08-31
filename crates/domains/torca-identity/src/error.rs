use core::fmt;

/// Invalid profile or public identity input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProfileError {
    /// Display name was empty after trimming.
    EmptyName,
    /// Display name exceeded the allowed character count.
    NameTooLong { actual: usize },
    /// Text contained a control character.
    ControlCharacter,
    /// Avatar reference was empty.
    EmptyAvatarReference,
    /// Avatar reference exceeded the byte limit.
    AvatarReferenceTooLong { actual: usize },
    /// Public key bytes were empty.
    EmptyPublicKey,
    /// Country code is neither ISO alpha-2 nor UNKNOWN.
    InvalidCountryCode,
}

impl fmt::Display for ProfileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for ProfileError {}

/// Storage-port failure represented without infrastructure details.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdentityRepositoryError(pub String);
impl fmt::Display for IdentityRepositoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for IdentityRepositoryError {}

/// Key-provider failure represented without secret material.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdentityKeyProviderError(pub String);
impl fmt::Display for IdentityKeyProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for IdentityKeyProviderError {}

/// Identity workflow error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IdentityError {
    /// A local identity already exists.
    AlreadyExists,
    /// No local identity exists.
    NotFound,
    /// Optimistic generation check failed.
    Conflict,
    /// Profile validation failed.
    InvalidProfile(ProfileError),
    /// Repository port failed.
    Repository(IdentityRepositoryError),
    /// Key provider failed.
    KeyProvider(IdentityKeyProviderError),
}

impl fmt::Display for IdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for IdentityError {}
impl From<ProfileError> for IdentityError {
    fn from(value: ProfileError) -> Self {
        Self::InvalidProfile(value)
    }
}
impl From<IdentityRepositoryError> for IdentityError {
    fn from(value: IdentityRepositoryError) -> Self {
        Self::Repository(value)
    }
}
impl From<IdentityKeyProviderError> for IdentityError {
    fn from(value: IdentityKeyProviderError) -> Self {
        Self::KeyProvider(value)
    }
}
