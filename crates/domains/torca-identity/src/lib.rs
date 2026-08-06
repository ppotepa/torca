//! Local installation identity domain for Torca.

mod error;
mod memory;
mod model;
mod ports;
mod service;

pub use error::{IdentityError, IdentityKeyProviderError, IdentityRepositoryError, ProfileError};
pub use memory::{DeterministicKeyProvider, InMemoryIdentityRepository};
pub use model::{
    AvatarReference, CreateIdentity, GeneratedSigningKey, Identity, IdentityCreated,
    IdentityId, IdentityKey, IdentityKeyRotated, KeyAlgorithm, KeyId, Profile,
    ProfileName, ProfileUpdated, PublicIdentity, RotateIdentity, UpdateProfile,
};
pub use ports::{IdentityKeyProvider, IdentityRepository};
pub use service::IdentityService;
