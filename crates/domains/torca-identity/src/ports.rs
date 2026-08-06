use crate::{GeneratedSigningKey, Identity, IdentityKeyProviderError, IdentityRepositoryError};

/// Persistence port owned by the identity domain.
pub trait IdentityRepository {
    /// Loads the single local identity.
    fn load(&self) -> Result<Option<Identity>, IdentityRepositoryError>;
    /// Inserts an identity only when none exists.
    fn insert(&mut self, identity: &Identity) -> Result<(), IdentityRepositoryError>;
    /// Replaces an identity when the stored generation matches the expected value.
    fn replace(
        &mut self,
        expected_generation: u32,
        identity: &Identity,
    ) -> Result<bool, IdentityRepositoryError>;
}

/// Private-key management port. Implementations retain all private material.
pub trait IdentityKeyProvider {
    /// Generates a signing key and returns only a handle plus public bytes.
    fn generate_signing_key(&mut self) -> Result<GeneratedSigningKey, IdentityKeyProviderError>;
}
