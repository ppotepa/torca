use crate::{
    GeneratedSigningKey, Identity, IdentityKeyProvider, IdentityKeyProviderError,
    IdentityRepository, IdentityRepositoryError, KeyAlgorithm, KeyId,
};

/// In-memory repository for domain and integration tests.
#[derive(Clone, Debug, Default)]
pub struct InMemoryIdentityRepository {
    identity: Option<Identity>,
}

impl IdentityRepository for InMemoryIdentityRepository {
    fn load(&self) -> Result<Option<Identity>, IdentityRepositoryError> {
        Ok(self.identity.clone())
    }
    fn insert(&mut self, identity: &Identity) -> Result<(), IdentityRepositoryError> {
        if self.identity.is_some() {
            return Err(IdentityRepositoryError("identity already exists".into()));
        }
        self.identity = Some(identity.clone());
        Ok(())
    }
    fn replace(
        &mut self,
        expected_generation: u32,
        identity: &Identity,
    ) -> Result<bool, IdentityRepositoryError> {
        let matches = self
            .identity
            .as_ref()
            .is_some_and(|stored| stored.public().generation() == expected_generation);
        if matches {
            self.identity = Some(identity.clone());
        }
        Ok(matches)
    }
}

/// Deterministic non-production key provider for tests only.
#[derive(Clone, Debug)]
pub struct DeterministicKeyProvider {
    next: u128,
}
impl Default for DeterministicKeyProvider {
    fn default() -> Self {
        Self { next: 1 }
    }
}

impl IdentityKeyProvider for DeterministicKeyProvider {
    fn generate_signing_key(&mut self) -> Result<GeneratedSigningKey, IdentityKeyProviderError> {
        let value = self.next;
        self.next = self
            .next
            .checked_add(1)
            .ok_or_else(|| IdentityKeyProviderError("test key counter exhausted".into()))?;
        let mut public_key = vec![0_u8; 32];
        public_key[16..].copy_from_slice(&value.to_be_bytes());
        Ok(GeneratedSigningKey {
            key_id: KeyId::from_u128(value),
            algorithm: KeyAlgorithm::Ed25519,
            public_key,
        })
    }
}

#[cfg(test)]
mod tests {
    use torca_foundation::Timestamp;

    use crate::{CreateIdentity, IdentityId, IdentityService, Profile, ProfileName};

    use super::{DeterministicKeyProvider, InMemoryIdentityRepository};

    #[test]
    fn identity_is_created_once_and_contains_no_private_key_material() {
        let mut service = IdentityService::new(
            InMemoryIdentityRepository::default(),
            DeterministicKeyProvider::default(),
        );
        let profile = Profile::new(ProfileName::new("Alice").expect("valid name"), None);
        let (identity, event) = service
            .create(CreateIdentity {
                identity_id: IdentityId::from_u128(7),
                profile,
                at: Timestamp::UNIX_EPOCH,
            })
            .expect("creation succeeds");
        assert_eq!(identity.public(), &event.public_identity);
        assert!(
            service
                .create(CreateIdentity {
                    identity_id: IdentityId::from_u128(8),
                    profile: identity.profile().clone(),
                    at: Timestamp::UNIX_EPOCH
                })
                .is_err()
        );
    }
}
