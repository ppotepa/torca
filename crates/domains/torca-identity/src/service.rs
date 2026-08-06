use crate::{
    CreateIdentity, Identity, IdentityCreated, IdentityError, IdentityKey, IdentityKeyProvider,
    IdentityKeyRotated, IdentityRepository, ProfileUpdated, PublicIdentity, RotateIdentity,
    UpdateProfile,
};

/// Coordinates identity use cases through domain-owned ports.
pub struct IdentityService<R, K> {
    repository: R,
    key_provider: K,
}

impl<R, K> IdentityService<R, K>
where
    R: IdentityRepository,
    K: IdentityKeyProvider,
{
    /// Creates a service.
    pub const fn new(repository: R, key_provider: K) -> Self {
        Self { repository, key_provider }
    }
    /// Consumes the service and returns its ports.
    pub fn into_parts(self) -> (R, K) {
        (self.repository, self.key_provider)
    }
    /// Loads the local identity.
    pub fn load(&self) -> Result<Option<Identity>, IdentityError> {
        self.repository.load().map_err(Into::into)
    }

    /// Creates the local identity exactly once.
    pub fn create(
        &mut self,
        command: CreateIdentity,
    ) -> Result<(Identity, IdentityCreated), IdentityError> {
        if self.repository.load()?.is_some() {
            return Err(IdentityError::AlreadyExists);
        }
        let generated = self.key_provider.generate_signing_key()?;
        let key = IdentityKey::new(generated.key_id, generated.algorithm, generated.public_key)?;
        let public = PublicIdentity::new(command.identity_id, key, 0);
        let identity = Identity::new(public.clone(), command.profile, command.at);
        self.repository.insert(&identity)?;
        let event = IdentityCreated { public_identity: public, at: command.at };
        Ok((identity, event))
    }

    /// Updates the local profile using optimistic key-generation concurrency.
    pub fn update_profile(
        &mut self,
        command: UpdateProfile,
    ) -> Result<(Identity, ProfileUpdated), IdentityError> {
        let mut identity = self.repository.load()?.ok_or(IdentityError::NotFound)?;
        let generation = identity.public().generation();
        identity.update_profile(command.profile.clone(), command.at);
        if !self.repository.replace(generation, &identity)? {
            return Err(IdentityError::Conflict);
        }
        let event = ProfileUpdated {
            identity_id: identity.public().identity_id(),
            profile: command.profile,
            at: command.at,
        };
        Ok((identity, event))
    }

    /// Rotates the private signing key while preserving identity continuity.
    pub fn rotate(
        &mut self,
        command: RotateIdentity,
    ) -> Result<(Identity, IdentityKeyRotated), IdentityError> {
        let mut identity = self.repository.load()?.ok_or(IdentityError::NotFound)?;
        let expected_generation = identity.public().generation();
        let previous_key_id = identity.public().key().key_id();
        let generated = self.key_provider.generate_signing_key()?;
        let key = IdentityKey::new(generated.key_id, generated.algorithm, generated.public_key)?;
        identity.rotate_key(key, command.at);
        if !self.repository.replace(expected_generation, &identity)? {
            return Err(IdentityError::Conflict);
        }
        let event = IdentityKeyRotated {
            public_identity: identity.public().clone(),
            previous_key_id,
            at: command.at,
        };
        Ok((identity, event))
    }
}
