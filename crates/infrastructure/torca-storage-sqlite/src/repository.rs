use core::fmt;
use std::path::Path;

use rusqlite::{OptionalExtension, params};
use torca_foundation::{OpaqueId, Timestamp};
use torca_identity::{
    AvatarReference, Identity, IdentityId, IdentityKey, IdentityRepository,
    IdentityRepositoryError, KeyAlgorithm, KeyId, Profile, ProfileName, PublicIdentity,
};

use crate::{
    DatabaseKey, MigrationError, SqlCipherBackend, StorageBackendError, StorageKernel,
    identity_sql,
};

/// Failure while opening and migrating a concrete encrypted store.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SqlCipherStoreOpenError {
    /// SQLCipher connection or key verification failed.
    Backend(StorageBackendError),
    /// Embedded schema migration failed.
    Migration(MigrationError),
}

impl fmt::Display for SqlCipherStoreOpenError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for SqlCipherStoreOpenError {}

impl From<StorageBackendError> for SqlCipherStoreOpenError {
    fn from(value: StorageBackendError) -> Self {
        Self::Backend(value)
    }
}

impl From<MigrationError> for SqlCipherStoreOpenError {
    fn from(value: MigrationError) -> Self {
        Self::Migration(value)
    }
}

/// Concrete SQLCipher store implementing domain-owned repository ports.
pub struct SqlCipherStore {
    backend: SqlCipherBackend,
}

impl SqlCipherStore {
    /// Opens, keys and migrates an encrypted store.
    pub fn open(
        path: impl AsRef<Path>,
        key: &DatabaseKey,
    ) -> Result<Self, SqlCipherStoreOpenError> {
        let backend = SqlCipherBackend::open(path, key)?;
        Self::bootstrap(backend)
    }

    /// Opens and migrates an encrypted in-memory store for integration tests.
    pub fn open_in_memory(key: &DatabaseKey) -> Result<Self, SqlCipherStoreOpenError> {
        let backend = SqlCipherBackend::open_in_memory(key)?;
        Self::bootstrap(backend)
    }

    fn bootstrap(backend: SqlCipherBackend) -> Result<Self, SqlCipherStoreOpenError> {
        let mut kernel = StorageKernel::new(backend);
        kernel.bootstrap()?;
        Ok(Self {
            backend: kernel.into_backend(),
        })
    }

    /// Returns the active SQLCipher version.
    pub fn cipher_version(&self) -> &str {
        self.backend.cipher_version()
    }
}

impl IdentityRepository for SqlCipherStore {
    fn load(&self) -> Result<Option<Identity>, IdentityRepositoryError> {
        let mut statement = self
            .backend
            .connection()
            .prepare(identity_sql::SELECT.sql)
            .map_err(repository_error)?;

        let row = statement
            .query_row([], |row| {
                Ok(IdentityRow {
                    identity_id: row.get(0)?,
                    key_id: row.get(1)?,
                    key_algorithm: row.get(2)?,
                    public_key: row.get(3)?,
                    key_generation: row.get(4)?,
                    display_name: row.get(5)?,
                    avatar_reference: row.get(6)?,
                    created_at_ms: row.get(7)?,
                    updated_at_ms: row.get(8)?,
                })
            })
            .optional()
            .map_err(repository_error)?;

        row.map(IdentityRow::into_identity).transpose()
    }

    fn insert(&mut self, identity: &Identity) -> Result<(), IdentityRepositoryError> {
        let identity_id = identity.public().identity_id().to_opaque().into_bytes();
        let key_id = identity.public().key().key_id().to_opaque().into_bytes();
        let avatar = identity.profile().avatar().map(AvatarReference::as_str);

        self.backend
            .connection()
            .execute(
                identity_sql::INSERT.sql,
                params![
                    identity_id.as_slice(),
                    key_id.as_slice(),
                    encode_algorithm(identity.public().key().algorithm()),
                    identity.public().key().public_key(),
                    i64::from(identity.public().generation()),
                    identity.profile().display_name().as_str(),
                    avatar,
                    identity.created_at().to_unix_millis(),
                    identity.updated_at().to_unix_millis(),
                ],
            )
            .map_err(repository_error)?;
        Ok(())
    }

    fn replace(
        &mut self,
        expected_generation: u32,
        identity: &Identity,
    ) -> Result<bool, IdentityRepositoryError> {
        let key_id = identity.public().key().key_id().to_opaque().into_bytes();
        let avatar = identity.profile().avatar().map(AvatarReference::as_str);
        let changed = self
            .backend
            .connection()
            .execute(
                identity_sql::UPDATE.sql,
                params![
                    key_id.as_slice(),
                    encode_algorithm(identity.public().key().algorithm()),
                    identity.public().key().public_key(),
                    i64::from(identity.public().generation()),
                    identity.profile().display_name().as_str(),
                    avatar,
                    identity.updated_at().to_unix_millis(),
                    i64::from(expected_generation),
                ],
            )
            .map_err(repository_error)?;
        Ok(changed == 1)
    }
}

struct IdentityRow {
    identity_id: Vec<u8>,
    key_id: Vec<u8>,
    key_algorithm: i64,
    public_key: Vec<u8>,
    key_generation: i64,
    display_name: String,
    avatar_reference: Option<String>,
    created_at_ms: i64,
    updated_at_ms: i64,
}

impl IdentityRow {
    fn into_identity(self) -> Result<Identity, IdentityRepositoryError> {
        let identity_id = IdentityId::from_opaque(OpaqueId::from_bytes(fixed_16(
            self.identity_id,
            "identity_id",
        )?));
        let key_id = KeyId::from_opaque(OpaqueId::from_bytes(fixed_16(self.key_id, "key_id")?));
        let algorithm = decode_algorithm(self.key_algorithm)?;
        let generation = u32::try_from(self.key_generation)
            .map_err(|_| data_error("key_generation is outside u32 range"))?;
        let key = IdentityKey::new(key_id, algorithm, self.public_key)
            .map_err(|error| data_error(&format!("invalid public key: {error}")))?;
        let public = PublicIdentity::new(identity_id, key, generation);
        let name = ProfileName::new(self.display_name)
            .map_err(|error| data_error(&format!("invalid display name: {error}")))?;
        let avatar = self
            .avatar_reference
            .map(AvatarReference::new)
            .transpose()
            .map_err(|error| data_error(&format!("invalid avatar reference: {error}")))?;
        let profile = Profile::new(name, avatar);
        let created_at = Timestamp::from_unix_millis(self.created_at_ms)
            .map_err(|error| data_error(&format!("invalid created_at: {error}")))?;
        let updated_at = Timestamp::from_unix_millis(self.updated_at_ms)
            .map_err(|error| data_error(&format!("invalid updated_at: {error}")))?;

        let mut identity = Identity::new(public, profile.clone(), created_at);
        if updated_at != created_at {
            identity.update_profile(profile, updated_at);
        }
        Ok(identity)
    }
}

fn fixed_16(value: Vec<u8>, field: &str) -> Result<[u8; 16], IdentityRepositoryError> {
    value
        .try_into()
        .map_err(|_| data_error(&format!("{field} must contain 16 bytes")))
}

const fn encode_algorithm(value: KeyAlgorithm) -> i64 {
    match value {
        KeyAlgorithm::Ed25519 => 1,
    }
}

fn decode_algorithm(value: i64) -> Result<KeyAlgorithm, IdentityRepositoryError> {
    match value {
        1 => Ok(KeyAlgorithm::Ed25519),
        _ => Err(data_error("unsupported key algorithm in database")),
    }
}

fn repository_error(error: rusqlite::Error) -> IdentityRepositoryError {
    let code = error
        .sqlite_error_code()
        .map_or_else(|| "unknown".to_owned(), |value| format!("{value:?}"));
    IdentityRepositoryError(format!("identity repository operation failed ({code})"))
}

fn data_error(message: &str) -> IdentityRepositoryError {
    IdentityRepositoryError(format!("identity repository contains invalid data: {message}"))
}

#[cfg(test)]
mod tests {
    use torca_foundation::Timestamp;
    use torca_identity::{
        Identity, IdentityId, IdentityKey, IdentityRepository, KeyAlgorithm, KeyId, Profile,
        ProfileName, PublicIdentity,
    };

    use crate::{DatabaseKey, SqlCipherStore};

    #[test]
    fn identity_round_trips_through_sqlcipher() {
        let key = DatabaseKey::new([0x24; 32]);
        let mut store = SqlCipherStore::open_in_memory(&key).expect("open store");
        let profile = Profile::new(ProfileName::new("Orca").expect("name"), None);
        let public_key = IdentityKey::new(
            KeyId::from_u128(2),
            KeyAlgorithm::Ed25519,
            vec![7; 32],
        )
        .expect("key");
        let public = PublicIdentity::new(IdentityId::from_u128(1), public_key, 0);
        let identity = Identity::new(public, profile, Timestamp::UNIX_EPOCH);

        store.insert(&identity).expect("insert");
        assert_eq!(store.load().expect("load"), Some(identity));
    }
}
