use core::fmt;

use torca_foundation::{OpaqueId, Timestamp};

use crate::ProfileError;

macro_rules! typed_id {
    ($name:ident, $docs:literal) => {
        #[doc = $docs]
        #[must_use]
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(OpaqueId);

        impl $name {
            /// Creates an identifier from a shared opaque value.
            pub const fn from_opaque(value: OpaqueId) -> Self {
                Self(value)
            }
            /// Creates an identifier from an integer, primarily for deterministic composition and tests.
            pub const fn from_u128(value: u128) -> Self {
                Self(OpaqueId::from_u128(value))
            }
            /// Returns the shared opaque representation.
            pub const fn to_opaque(self) -> OpaqueId {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, formatter)
            }
        }
    };
}

typed_id!(IdentityId, "Stable identifier of one local installation identity.");
typed_id!(KeyId, "Stable identifier of a private signing key managed outside the domain.");

/// Supported semantic signing algorithm identifier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyAlgorithm {
    /// Ed25519 signing keys.
    Ed25519,
}

/// Validated display name.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfileName(String);

impl ProfileName {
    /// Maximum number of Unicode scalar values.
    pub const MAX_CHARS: usize = 64;

    /// Validates and creates a profile name.
    pub fn new(value: impl Into<String>) -> Result<Self, ProfileError> {
        let value = value.into();
        let trimmed = value.trim();
        let length = trimmed.chars().count();
        if length == 0 {
            return Err(ProfileError::EmptyName);
        }
        if length > Self::MAX_CHARS {
            return Err(ProfileError::NameTooLong { actual: length });
        }
        if trimmed.chars().any(char::is_control) {
            return Err(ProfileError::ControlCharacter);
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// Returns the validated name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Validated opaque avatar reference. The domain does not interpret storage paths or URLs.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AvatarReference(String);

impl AvatarReference {
    /// Maximum encoded reference length.
    pub const MAX_BYTES: usize = 256;

    /// Validates and creates an avatar reference.
    pub fn new(value: impl Into<String>) -> Result<Self, ProfileError> {
        let value = value.into();
        if value.is_empty() {
            return Err(ProfileError::EmptyAvatarReference);
        }
        if value.len() > Self::MAX_BYTES {
            return Err(ProfileError::AvatarReferenceTooLong { actual: value.len() });
        }
        if value.chars().any(char::is_control) {
            return Err(ProfileError::ControlCharacter);
        }
        Ok(Self(value))
    }

    /// Returns the opaque reference.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// User-editable local profile.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Profile {
    display_name: ProfileName,
    avatar: Option<AvatarReference>,
}

impl Profile {
    /// Creates a profile.
    pub const fn new(display_name: ProfileName, avatar: Option<AvatarReference>) -> Self {
        Self { display_name, avatar }
    }
    /// Returns the display name.
    pub const fn display_name(&self) -> &ProfileName {
        &self.display_name
    }
    /// Returns the optional avatar reference.
    pub const fn avatar(&self) -> Option<&AvatarReference> {
        self.avatar.as_ref()
    }
}

/// Public key reference stored by the domain without private material.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdentityKey {
    key_id: KeyId,
    algorithm: KeyAlgorithm,
    public_key: Vec<u8>,
}

impl IdentityKey {
    /// Creates a validated key reference.
    pub fn new(
        key_id: KeyId,
        algorithm: KeyAlgorithm,
        public_key: Vec<u8>,
    ) -> Result<Self, ProfileError> {
        if public_key.is_empty() {
            return Err(ProfileError::EmptyPublicKey);
        }
        Ok(Self { key_id, algorithm, public_key })
    }
    /// Returns the key identifier.
    pub const fn key_id(&self) -> KeyId {
        self.key_id
    }
    /// Returns the algorithm identifier.
    pub const fn algorithm(&self) -> KeyAlgorithm {
        self.algorithm
    }
    /// Returns public key bytes.
    pub fn public_key(&self) -> &[u8] {
        &self.public_key
    }
}

/// Public representation safe to share with peers.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicIdentity {
    identity_id: IdentityId,
    key: IdentityKey,
    generation: u32,
}

impl PublicIdentity {
    /// Creates a public identity.
    pub const fn new(identity_id: IdentityId, key: IdentityKey, generation: u32) -> Self {
        Self { identity_id, key, generation }
    }
    /// Returns the identity identifier.
    pub const fn identity_id(&self) -> IdentityId {
        self.identity_id
    }
    /// Returns the public key reference.
    pub const fn key(&self) -> &IdentityKey {
        &self.key
    }
    /// Returns the monotonic key generation.
    pub const fn generation(&self) -> u32 {
        self.generation
    }
}

/// Complete local domain identity without private key bytes.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Identity {
    public: PublicIdentity,
    profile: Profile,
    created_at: Timestamp,
    updated_at: Timestamp,
}

impl Identity {
    /// Creates a local identity.
    pub const fn new(public: PublicIdentity, profile: Profile, created_at: Timestamp) -> Self {
        Self { public, profile, created_at, updated_at: created_at }
    }
    /// Returns the public representation.
    pub const fn public(&self) -> &PublicIdentity {
        &self.public
    }
    /// Returns the local profile.
    pub const fn profile(&self) -> &Profile {
        &self.profile
    }
    /// Returns the creation timestamp.
    pub const fn created_at(&self) -> Timestamp {
        self.created_at
    }
    /// Returns the last mutation timestamp.
    pub const fn updated_at(&self) -> Timestamp {
        self.updated_at
    }
    /// Updates the profile.
    pub fn update_profile(&mut self, profile: Profile, at: Timestamp) {
        self.profile = profile;
        self.updated_at = at;
    }
    /// Replaces the public key and increments generation.
    pub fn rotate_key(&mut self, key: IdentityKey, at: Timestamp) {
        let generation = self.public.generation.saturating_add(1);
        self.public = PublicIdentity::new(self.public.identity_id, key, generation);
        self.updated_at = at;
    }
}

/// Key material generated by an external key provider. No private bytes cross the port.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedSigningKey {
    /// Key identifier managed by the provider.
    pub key_id: KeyId,
    /// Semantic algorithm.
    pub algorithm: KeyAlgorithm,
    /// Public key bytes.
    pub public_key: Vec<u8>,
}

/// Input for identity creation.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateIdentity {
    pub identity_id: IdentityId,
    pub profile: Profile,
    pub at: Timestamp,
}
/// Input for profile mutation.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateProfile {
    pub profile: Profile,
    pub at: Timestamp,
}
/// Input for key rotation.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RotateIdentity {
    pub at: Timestamp,
}

/// Event emitted after identity creation.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdentityCreated {
    pub public_identity: PublicIdentity,
    pub at: Timestamp,
}
/// Event emitted after profile mutation.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfileUpdated {
    pub identity_id: IdentityId,
    pub profile: Profile,
    pub at: Timestamp,
}
/// Event emitted after key rotation.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdentityKeyRotated {
    pub public_identity: PublicIdentity,
    pub previous_key_id: KeyId,
    pub at: Timestamp,
}
