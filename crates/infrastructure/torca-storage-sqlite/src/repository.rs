use core::fmt;
use std::path::Path;

use rusqlite::{OptionalExtension, params};
use torca_client_engine::{AvatarGenomeRecord, EngineError, RelationshipRepository};
use torca_contacts::{
    Contact, ContactError, ContactId, ContactRepository, ContactRoute, ContactStatus,
    PeerCredential, PeerCredentialRepository,
};
use torca_conversations::{
    ConversationError, ConversationId, ConversationRepository, ConversationStatus,
    DirectConversation,
};
use torca_foundation::{OpaqueId, ProviderId, Timestamp};
use torca_identity::{
    AvatarReference, Identity, IdentityId, IdentityKey, IdentityRepository,
    IdentityRepositoryError, KeyAlgorithm, KeyId, Profile, ProfileName, PublicIdentity,
};

use crate::{
    DatabaseKey, MigrationError, SqlCipherBackend, StorageBackend, StorageBackendError,
    StorageKernel, contact_sql, conversation_sql, identity_sql, peer_credential_sql,
};

const DELETE_CONTACT_SQL: &str = include_str!("../sql/commands/contact_delete.sql");

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SqlCipherStoreOpenError {
    Backend(StorageBackendError),
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

pub struct SqlCipherStore {
    backend: SqlCipherBackend,
}

impl SqlCipherStore {
    pub fn open(
        path: impl AsRef<Path>,
        key: &DatabaseKey,
    ) -> Result<Self, SqlCipherStoreOpenError> {
        let backend = SqlCipherBackend::open(path, key)?;
        Self::bootstrap(backend)
    }

    pub fn open_in_memory(key: &DatabaseKey) -> Result<Self, SqlCipherStoreOpenError> {
        let backend = SqlCipherBackend::open_in_memory(key)?;
        Self::bootstrap(backend)
    }

    fn bootstrap(backend: SqlCipherBackend) -> Result<Self, SqlCipherStoreOpenError> {
        let mut kernel = StorageKernel::new(backend);
        kernel.bootstrap()?;
        Ok(Self { backend: kernel.into_backend() })
    }

    pub fn cipher_version(&self) -> &str {
        self.backend.cipher_version()
    }

    pub fn upsert_avatar_genome(
        &mut self,
        record: &AvatarGenomeRecord,
        at: Timestamp,
    ) -> Result<(), EngineError> {
        if record.compressed_genome.is_empty() || record.compressed_genome.len() > 32 * 1024 {
            return Err(EngineError::InvalidState);
        }
        self.backend
            .connection()
            .execute(
                include_str!("../sql/commands/avatar_genome_upsert.sql"),
                params![
                    record.genome_hash.as_slice(),
                    i64::from(record.schema_version),
                    record.generator_version,
                    record.catalog_version,
                    record.compressed_genome,
                    at.to_unix_millis(),
                ],
            )
            .map_err(|_| EngineError::Repository)?;
        Ok(())
    }

    pub fn avatar_genome(
        &self,
        genome_hash: [u8; 32],
    ) -> Result<Option<AvatarGenomeRecord>, EngineError> {
        self.backend
            .connection()
            .query_row(
                include_str!("../sql/queries/avatar_genome_select.sql"),
                params![genome_hash.as_slice()],
                |row| {
                    let payload: Vec<u8> = row.get(3)?;
                    let schema_version = u8::try_from(row.get::<_, i64>(0)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?;
                    Ok(AvatarGenomeRecord {
                        genome_hash,
                        schema_version,
                        generator_version: row.get(1)?,
                        catalog_version: row.get(2)?,
                        compressed_genome: payload,
                    })
                },
            )
            .optional()
            .map_err(|_| EngineError::Repository)
    }

    pub fn local_avatar_genome(&self) -> Result<Option<AvatarGenomeRecord>, EngineError> {
        self.backend
            .connection()
            .query_row(include_str!("../sql/queries/avatar_genome_latest.sql"), [], |row| {
                let hash: Vec<u8> = row.get(0)?;
                let genome_hash: [u8; 32] =
                    hash.try_into().map_err(|_| rusqlite::Error::InvalidQuery)?;
                let schema_version = u8::try_from(row.get::<_, i64>(1)?)
                    .map_err(|_| rusqlite::Error::InvalidQuery)?;
                Ok(AvatarGenomeRecord {
                    genome_hash,
                    schema_version,
                    generator_version: row.get(2)?,
                    catalog_version: row.get(3)?,
                    compressed_genome: row.get(4)?,
                })
            })
            .optional()
            .map_err(|_| EngineError::Repository)
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
                    country_code: row.get(7)?,
                    created_at_ms: row.get(8)?,
                    updated_at_ms: row.get(9)?,
                })
            })
            .optional()
            .map_err(repository_error)?;
        row.map(IdentityRow::into_identity).transpose()
    }

    fn insert(&mut self, identity: &Identity) -> Result<(), IdentityRepositoryError> {
        let identity_id = identity.public().identity_id().to_opaque().into_bytes();
        let key_id = identity.public().key().key_id().to_opaque().into_bytes();
        let avatar =
            identity.profile().and_then(|profile| profile.avatar().map(AvatarReference::as_str));
        let display_name = identity.profile().map(|profile| profile.display_name().as_str());
        let country_code = identity.profile().and_then(|profile| profile.country_code());
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
                    display_name,
                    avatar,
                    country_code,
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
        let avatar =
            identity.profile().and_then(|profile| profile.avatar().map(AvatarReference::as_str));
        let display_name = identity.profile().map(|profile| profile.display_name().as_str());
        let country_code = identity.profile().and_then(|profile| profile.country_code());
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
                    display_name,
                    avatar,
                    country_code,
                    identity.updated_at().to_unix_millis(),
                    i64::from(expected_generation),
                ],
            )
            .map_err(repository_error)?;
        Ok(changed == 1)
    }
}

impl ContactRepository for SqlCipherStore {
    fn insert(&mut self, contact: Contact) -> Result<(), ContactError> {
        if ContactRepository::get(self, contact.id())?.is_some() {
            return Err(ContactError::AlreadyExists);
        }
        execute_contact(&self.backend, contact_sql::INSERT.sql, &contact)
    }

    fn get(&self, id: ContactId) -> Result<Option<Contact>, ContactError> {
        let id_bytes = id.to_opaque().into_bytes();
        let row = self
            .backend
            .connection()
            .query_row(contact_sql::SELECT_BY_ID.sql, params![id_bytes.as_slice()], |row| {
                Ok(ContactRow {
                    contact_id: id_bytes.to_vec(),
                    remote_identity_id: row.get(0)?,
                    remote_key_id: row.get(1)?,
                    remote_key_algorithm: row.get(2)?,
                    remote_public_key: row.get(3)?,
                    remote_key_generation: row.get(4)?,
                    capability_id: row.get(5)?,
                    status: row.get(6)?,
                    created_at_ms: row.get(7)?,
                    updated_at_ms: row.get(8)?,
                    transport_endpoints_json: row.get(9)?,
                    country_code: row.get(10)?,
                })
            })
            .optional()
            .map_err(|_| ContactError::RepositoryFailure)?;
        row.map(ContactRow::into_contact).transpose()
    }

    fn update(&mut self, contact: Contact) -> Result<(), ContactError> {
        if ContactRepository::get(self, contact.id())?.is_none() {
            return Err(ContactError::NotFound);
        }
        execute_contact(&self.backend, contact_sql::UPDATE.sql, &contact)
    }

    fn list(&self) -> Result<Vec<Contact>, ContactError> {
        let mut statement = self
            .backend
            .connection()
            .prepare(contact_sql::LIST.sql)
            .map_err(|_| ContactError::RepositoryFailure)?;
        let rows = statement
            .query_map([], |row| {
                Ok(ContactRow {
                    contact_id: row.get(0)?,
                    remote_identity_id: row.get(1)?,
                    remote_key_id: row.get(2)?,
                    remote_key_algorithm: row.get(3)?,
                    remote_public_key: row.get(4)?,
                    remote_key_generation: row.get(5)?,
                    capability_id: row.get(6)?,
                    status: row.get(7)?,
                    created_at_ms: row.get(8)?,
                    updated_at_ms: row.get(9)?,
                    transport_endpoints_json: row.get(10)?,
                    country_code: row.get(11)?,
                })
            })
            .map_err(|_| ContactError::RepositoryFailure)?;
        rows.map(|row| row.map_err(|_| ContactError::RepositoryFailure)?.into_contact()).collect()
    }
}

impl PeerCredentialRepository for SqlCipherStore {
    fn insert_credential(&mut self, credential: PeerCredential) -> Result<(), ContactError> {
        if PeerCredentialRepository::credential_for_contact(self, credential.contact_id())?
            .is_some()
        {
            return Err(ContactError::AlreadyExists);
        }
        insert_peer_credential(&self.backend, credential)
    }

    fn credential_for_contact(
        &self,
        contact_id: ContactId,
    ) -> Result<Option<PeerCredential>, ContactError> {
        let contact = contact_id.to_opaque().into_bytes();
        let row = self
            .backend
            .connection()
            .query_row(
                peer_credential_sql::SELECT_BY_CONTACT.sql,
                params![contact.as_slice()],
                |row| Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, Vec<u8>>(1)?)),
            )
            .optional()
            .map_err(|_| ContactError::RepositoryFailure)?;
        row.map(|(local_capability, secret_handle)| {
            PeerCredential::new(
                contact_id,
                OpaqueId::from_bytes(fixed_16_contact(local_capability)?),
                OpaqueId::from_bytes(fixed_16_contact(secret_handle)?),
            )
            .map_err(|_| ContactError::RepositoryFailure)
        })
        .transpose()
    }
}

impl ConversationRepository for SqlCipherStore {
    fn insert(&mut self, conversation: DirectConversation) -> Result<(), ConversationError> {
        if ConversationRepository::get(self, conversation.id())?.is_some() {
            return Err(ConversationError::AlreadyExists);
        }
        if ConversationRepository::for_contact(self, conversation.contact_id())?.is_some() {
            return Err(ConversationError::ContactAlreadyHasConversation);
        }
        insert_conversation(&self.backend, &conversation)
    }

    fn get(&self, id: ConversationId) -> Result<Option<DirectConversation>, ConversationError> {
        let id_bytes = id.to_opaque().into_bytes();
        let row = self
            .backend
            .connection()
            .query_row(conversation_sql::SELECT_BY_ID.sql, params![id_bytes.as_slice()], |row| {
                Ok(ConversationRow {
                    conversation_id: id_bytes.to_vec(),
                    contact_id: row.get(0)?,
                    status: row.get(1)?,
                    created_at_ms: row.get(2)?,
                    updated_at_ms: row.get(3)?,
                })
            })
            .optional()
            .map_err(|_| ConversationError::RepositoryFailure)?;
        row.map(ConversationRow::into_conversation).transpose()
    }

    fn for_contact(
        &self,
        contact_id: ContactId,
    ) -> Result<Option<DirectConversation>, ConversationError> {
        let contact_bytes = contact_id.to_opaque().into_bytes();
        let row = self
            .backend
            .connection()
            .query_row(
                conversation_sql::SELECT_BY_CONTACT.sql,
                params![contact_bytes.as_slice()],
                |row| {
                    Ok(ConversationRow {
                        conversation_id: row.get(0)?,
                        contact_id: contact_bytes.to_vec(),
                        status: row.get(1)?,
                        created_at_ms: row.get(2)?,
                        updated_at_ms: row.get(3)?,
                    })
                },
            )
            .optional()
            .map_err(|_| ConversationError::RepositoryFailure)?;
        row.map(ConversationRow::into_conversation).transpose()
    }

    fn list(&self) -> Result<Vec<DirectConversation>, ConversationError> {
        let mut statement = self
            .backend
            .connection()
            .prepare(conversation_sql::LIST.sql)
            .map_err(|_| ConversationError::RepositoryFailure)?;
        let rows = statement
            .query_map([], |row| {
                Ok(ConversationRow {
                    conversation_id: row.get(0)?,
                    contact_id: row.get(1)?,
                    status: row.get(2)?,
                    created_at_ms: row.get(3)?,
                    updated_at_ms: row.get(4)?,
                })
            })
            .map_err(|_| ConversationError::RepositoryFailure)?;
        rows.map(|row| row.map_err(|_| ConversationError::RepositoryFailure)?.into_conversation())
            .collect()
    }

    fn update(&mut self, conversation: DirectConversation) -> Result<(), ConversationError> {
        let id = conversation.id().to_opaque().into_bytes();
        let contact_id = conversation.contact_id().to_opaque().into_bytes();
        let changed = self
            .backend
            .connection()
            .execute(
                conversation_sql::UPDATE.sql,
                params![
                    id.as_slice(),
                    contact_id.as_slice(),
                    encode_conversation_status(conversation.status()),
                    conversation.updated_at().to_unix_millis(),
                ],
            )
            .map_err(|_| ConversationError::RepositoryFailure)?;
        if changed == 0 {
            return Err(ConversationError::NotFound);
        }
        Ok(())
    }
}

impl RelationshipRepository for SqlCipherStore {
    fn upsert_avatar_genome(
        &mut self,
        record: AvatarGenomeRecord,
        at: Timestamp,
    ) -> Result<(), EngineError> {
        SqlCipherStore::upsert_avatar_genome(self, &record, at)?;
        self.backend
            .connection()
            .execute(
                include_str!("../sql/commands/local_avatar_bind.sql"),
                params![record.genome_hash.as_slice()],
            )
            .map_err(|_| EngineError::Repository)?;
        Ok(())
    }

    fn avatar_genome(&self, hash: [u8; 32]) -> Result<Option<AvatarGenomeRecord>, EngineError> {
        SqlCipherStore::avatar_genome(self, hash)
    }

    fn avatar_genome_for_identity(
        &self,
        identity_id: IdentityId,
    ) -> Result<Option<AvatarGenomeRecord>, EngineError> {
        let identity = identity_id.to_opaque().into_bytes();
        self.backend
            .connection()
            .query_row(
                include_str!("../sql/queries/avatar_genome_for_identity.sql"),
                params![identity.as_slice()],
                |row| {
                    let hash: Vec<u8> = row.get(0)?;
                    let genome_hash: [u8; 32] =
                        hash.try_into().map_err(|_| rusqlite::Error::InvalidQuery)?;
                    let schema_version = u8::try_from(row.get::<_, i64>(1)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?;
                    Ok(AvatarGenomeRecord {
                        genome_hash,
                        schema_version,
                        generator_version: row.get(2)?,
                        catalog_version: row.get(3)?,
                        compressed_genome: row.get(4)?,
                    })
                },
            )
            .optional()
            .map_err(|_| EngineError::Repository)
    }

    fn local_avatar_genome(&self) -> Result<Option<AvatarGenomeRecord>, EngineError> {
        SqlCipherStore::local_avatar_genome(self)
    }

    fn insert_pairing_result(
        &mut self,
        contact: Contact,
        conversation: DirectConversation,
        display_name: &str,
        credential: PeerCredential,
        avatar: Option<AvatarGenomeRecord>,
        at: Timestamp,
    ) -> Result<(), EngineError> {
        if contact.id() != conversation.contact_id() || contact.id() != credential.contact_id() {
            return Err(EngineError::InvalidState);
        }
        if ContactRepository::get(self, contact.id()).map_err(relationship_error)?.is_some()
            || ConversationRepository::get(self, conversation.id())
                .map_err(relationship_error)?
                .is_some()
            || ConversationRepository::for_contact(self, contact.id())
                .map_err(relationship_error)?
                .is_some()
            || PeerCredentialRepository::credential_for_contact(self, contact.id())
                .map_err(relationship_error)?
                .is_some()
        {
            return Err(EngineError::Conflict);
        }

        self.backend.begin().map_err(|_| relationship_failure())?;
        let result = (|| {
            execute_contact(&self.backend, contact_sql::INSERT.sql, &contact)
                .map_err(relationship_error)?;
            insert_conversation(&self.backend, &conversation).map_err(relationship_error)?;
            insert_peer_credential(&self.backend, credential).map_err(relationship_error)?;
            if let Some(avatar) = avatar.as_ref() {
                SqlCipherStore::upsert_avatar_genome(self, avatar, at)?;
                self.backend
                    .connection()
                    .execute(
                        include_str!("../sql/commands/contact_avatar_bind.sql"),
                        params![
                            contact.id().to_opaque().as_bytes().as_slice(),
                            avatar.genome_hash.as_slice()
                        ],
                    )
                    .map_err(|_| relationship_failure())?;
            }
            self.backend
                .connection()
                .execute(
                    include_str!("../sql/commands/contact_metadata_upsert.sql"),
                    rusqlite::params![
                        contact.id().to_opaque().as_bytes().as_slice(),
                        display_name,
                        at.to_unix_millis()
                    ],
                )
                .map_err(|_| relationship_failure())?;
            Ok::<(), EngineError>(())
        })();

        match result {
            Ok(()) => self.backend.commit().map_err(|_| relationship_failure()),
            Err(error) => {
                let _ = self.backend.rollback();
                Err(error)
            }
        }
    }

    fn remove_relationship(&mut self, contact_id: ContactId) -> Result<(), EngineError> {
        let contact = contact_id.to_opaque().into_bytes();
        let changed = self
            .backend
            .connection()
            .execute(DELETE_CONTACT_SQL, params![contact.as_slice()])
            .map_err(|_| relationship_failure())?;
        if changed == 1 { Ok(()) } else { Err(EngineError::NotFound) }
    }
}

struct IdentityRow {
    identity_id: Vec<u8>,
    key_id: Vec<u8>,
    key_algorithm: i64,
    public_key: Vec<u8>,
    key_generation: i64,
    display_name: Option<String>,
    avatar_reference: Option<String>,
    country_code: Option<String>,
    created_at_ms: i64,
    updated_at_ms: i64,
}

impl IdentityRow {
    fn into_identity(self) -> Result<Identity, IdentityRepositoryError> {
        let identity_id = IdentityId::from_opaque(OpaqueId::from_bytes(fixed_16_identity(
            self.identity_id,
            "identity_id",
        )?));
        let key_id =
            KeyId::from_opaque(OpaqueId::from_bytes(fixed_16_identity(self.key_id, "key_id")?));
        let algorithm = decode_algorithm(self.key_algorithm)?;
        let generation = u32::try_from(self.key_generation)
            .map_err(|_| data_error("key_generation is outside u32 range"))?;
        let key = IdentityKey::new(key_id, algorithm, self.public_key)
            .map_err(|error| data_error(&format!("invalid public key: {error}")))?;
        let public = PublicIdentity::new(identity_id, key, generation);
        let profile = self
            .display_name
            .map(ProfileName::new)
            .transpose()
            .map_err(|error| data_error(&format!("invalid display name: {error}")))?
            .map(|name| {
                let avatar =
                    self.avatar_reference.clone().map(AvatarReference::new).transpose().map_err(
                        |error| data_error(&format!("invalid avatar reference: {error}")),
                    )?;
                Ok::<Profile, IdentityRepositoryError>(
                    Profile::with_country(name, avatar, self.country_code.clone())
                        .map_err(|error| data_error(&format!("invalid country code: {error}")))?,
                )
            })
            .transpose()?;
        let created_at = Timestamp::from_unix_millis(self.created_at_ms)
            .map_err(|error| data_error(&format!("invalid created_at: {error}")))?;
        let updated_at = Timestamp::from_unix_millis(self.updated_at_ms)
            .map_err(|error| data_error(&format!("invalid updated_at: {error}")))?;
        let mut identity = Identity::new(public, profile.clone(), created_at);
        if updated_at != created_at {
            if let Some(profile) = profile {
                identity.update_profile(profile, updated_at);
            }
        }
        Ok(identity)
    }
}

struct ContactRow {
    contact_id: Vec<u8>,
    remote_identity_id: Vec<u8>,
    remote_key_id: Vec<u8>,
    remote_key_algorithm: i64,
    remote_public_key: Vec<u8>,
    remote_key_generation: i64,
    capability_id: Vec<u8>,
    status: i64,
    created_at_ms: i64,
    updated_at_ms: i64,
    transport_endpoints_json: String,
    country_code: Option<String>,
}

impl ContactRow {
    fn into_contact(self) -> Result<Contact, ContactError> {
        let contact_id =
            ContactId::from_opaque(OpaqueId::from_bytes(fixed_16_contact(self.contact_id)?));
        let identity_id = IdentityId::from_opaque(OpaqueId::from_bytes(fixed_16_contact(
            self.remote_identity_id,
        )?));
        let key_id =
            KeyId::from_opaque(OpaqueId::from_bytes(fixed_16_contact(self.remote_key_id)?));
        let algorithm = match self.remote_key_algorithm {
            1 => KeyAlgorithm::Ed25519,
            _ => return Err(ContactError::RepositoryFailure),
        };
        let generation = u32::try_from(self.remote_key_generation)
            .map_err(|_| ContactError::RepositoryFailure)?;
        let key = IdentityKey::new(key_id, algorithm, self.remote_public_key)
            .map_err(|_| ContactError::RepositoryFailure)?;
        let remote_identity = PublicIdentity::new(identity_id, key, generation);
        let capability_id = OpaqueId::from_bytes(fixed_16_contact(self.capability_id)?);
        let endpoints = serde_json::from_str::<std::collections::BTreeMap<String, Vec<u8>>>(
            &self.transport_endpoints_json,
        )
        .map_err(|_| ContactError::RepositoryFailure)?;
        let endpoints = endpoints
            .into_iter()
            .map(|(provider, endpoint)| {
                ProviderId::new(provider)
                    .map(|provider| (provider, endpoint))
                    .map_err(|_| ContactError::RepositoryFailure)
            })
            .collect::<Result<std::collections::BTreeMap<_, _>, _>>()?;
        let route = ContactRoute::from_provider_endpoints(capability_id, endpoints)
            .map_err(|_| ContactError::RepositoryFailure)?;
        let status = match self.status {
            0 => ContactStatus::Active,
            1 => ContactStatus::Blocked,
            2 => ContactStatus::Removed,
            _ => return Err(ContactError::RepositoryFailure),
        };
        let created_at = Timestamp::from_unix_millis(self.created_at_ms)
            .map_err(|_| ContactError::RepositoryFailure)?;
        let updated_at = Timestamp::from_unix_millis(self.updated_at_ms)
            .map_err(|_| ContactError::RepositoryFailure)?;
        let mut contact =
            Contact::restore(contact_id, remote_identity, route, status, created_at, updated_at);
        contact.set_country_code(self.country_code);
        Ok(contact)
    }
}

struct ConversationRow {
    conversation_id: Vec<u8>,
    contact_id: Vec<u8>,
    status: i64,
    created_at_ms: i64,
    updated_at_ms: i64,
}

impl ConversationRow {
    fn into_conversation(self) -> Result<DirectConversation, ConversationError> {
        let id = ConversationId::from_opaque(OpaqueId::from_bytes(fixed_16_conversation(
            self.conversation_id,
        )?));
        let contact_id =
            ContactId::from_opaque(OpaqueId::from_bytes(fixed_16_conversation(self.contact_id)?));
        let status = match self.status {
            0 => ConversationStatus::Active,
            1 => ConversationStatus::Archived,
            _ => return Err(ConversationError::RepositoryFailure),
        };
        let created_at = Timestamp::from_unix_millis(self.created_at_ms)
            .map_err(|_| ConversationError::RepositoryFailure)?;
        let updated_at = Timestamp::from_unix_millis(self.updated_at_ms)
            .map_err(|_| ConversationError::RepositoryFailure)?;
        Ok(DirectConversation::from_persisted(id, contact_id, status, created_at, updated_at))
    }
}

fn execute_contact(
    backend: &SqlCipherBackend,
    sql: &str,
    contact: &Contact,
) -> Result<(), ContactError> {
    let contact_id = contact.id().to_opaque().into_bytes();
    let remote_identity_id = contact.remote_identity().identity_id().to_opaque().into_bytes();
    let remote_key_id = contact.remote_identity().key().key_id().to_opaque().into_bytes();
    let capability_id = contact.route().capability_id().into_bytes();
    let serialized_endpoints = contact
        .route()
        .provider_endpoints()
        .iter()
        .map(|(provider, endpoint)| (provider.as_str(), endpoint))
        .collect::<std::collections::BTreeMap<_, _>>();
    let transport_endpoints_json = serde_json::to_string(&serialized_endpoints)
        .map_err(|_| ContactError::RepositoryFailure)?;
    backend
        .connection()
        .execute(
            sql,
            params![
                contact_id.as_slice(),
                remote_identity_id.as_slice(),
                remote_key_id.as_slice(),
                encode_algorithm(contact.remote_identity().key().algorithm()),
                contact.remote_identity().key().public_key(),
                i64::from(contact.remote_identity().generation()),
                capability_id.as_slice(),
                encode_contact_status(contact.status()),
                contact.created_at().to_unix_millis(),
                contact.updated_at().to_unix_millis(),
                transport_endpoints_json,
                contact.country_code(),
            ],
        )
        .map_err(|_| ContactError::RepositoryFailure)?;
    Ok(())
}

fn insert_conversation(
    backend: &SqlCipherBackend,
    conversation: &DirectConversation,
) -> Result<(), ConversationError> {
    let id = conversation.id().to_opaque().into_bytes();
    let contact_id = conversation.contact_id().to_opaque().into_bytes();
    backend
        .connection()
        .execute(
            conversation_sql::INSERT.sql,
            params![
                id.as_slice(),
                contact_id.as_slice(),
                encode_conversation_status(conversation.status()),
                conversation.created_at().to_unix_millis(),
                conversation.updated_at().to_unix_millis(),
            ],
        )
        .map_err(|_| ConversationError::RepositoryFailure)?;
    Ok(())
}

fn insert_peer_credential(
    backend: &SqlCipherBackend,
    credential: PeerCredential,
) -> Result<(), ContactError> {
    let contact_id = credential.contact_id().to_opaque().into_bytes();
    let local_capability = credential.local_capability_id().into_bytes();
    let secret_handle = credential.secret_handle().into_bytes();
    backend
        .connection()
        .execute(
            peer_credential_sql::INSERT.sql,
            params![contact_id.as_slice(), local_capability.as_slice(), secret_handle.as_slice(),],
        )
        .map_err(|_| ContactError::RepositoryFailure)?;
    Ok(())
}

fn fixed_16_identity(value: Vec<u8>, field: &str) -> Result<[u8; 16], IdentityRepositoryError> {
    value.try_into().map_err(|_| data_error(&format!("{field} must contain 16 bytes")))
}

fn fixed_16_contact(value: Vec<u8>) -> Result<[u8; 16], ContactError> {
    value.try_into().map_err(|_| ContactError::RepositoryFailure)
}

fn fixed_16_conversation(value: Vec<u8>) -> Result<[u8; 16], ConversationError> {
    value.try_into().map_err(|_| ConversationError::RepositoryFailure)
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

const fn encode_contact_status(value: ContactStatus) -> i64 {
    match value {
        ContactStatus::Active => 0,
        ContactStatus::Blocked => 1,
        ContactStatus::Removed => 2,
    }
}

const fn encode_conversation_status(value: ConversationStatus) -> i64 {
    match value {
        ConversationStatus::Active => 0,
        ConversationStatus::Archived => 1,
    }
}

fn relationship_error(error: impl fmt::Display) -> EngineError {
    let _ = error;
    relationship_failure()
}

fn relationship_failure() -> EngineError {
    EngineError::Repository
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
    use rusqlite::params;
    use torca_client_engine::RelationshipRepository;
    use torca_contacts::{
        Contact, ContactId, ContactRepository, ContactRoute, PeerCredential,
        PeerCredentialRepository,
    };
    use torca_conversations::{ConversationId, ConversationRepository, DirectConversation};
    use torca_foundation::{OpaqueId, Timestamp};
    use torca_identity::{
        Identity, IdentityId, IdentityKey, IdentityRepository, KeyAlgorithm, KeyId, Profile,
        ProfileName, PublicIdentity,
    };

    use crate::{AvatarGenomeRecord, DatabaseKey, SqlCipherStore};

    fn remote_identity() -> PublicIdentity {
        let key = IdentityKey::new(KeyId::from_u128(12), KeyAlgorithm::Ed25519, vec![9; 32])
            .expect("key");
        PublicIdentity::new(IdentityId::from_u128(11), key, 0)
    }

    #[test]
    fn identity_round_trips_through_sqlcipher() {
        let key = DatabaseKey::new([0x24; 32]);
        let mut store = SqlCipherStore::open_in_memory(&key).expect("open store");
        let profile = Profile::new(ProfileName::new("Orca").expect("name"), None);
        let public_key =
            IdentityKey::new(KeyId::from_u128(2), KeyAlgorithm::Ed25519, vec![7; 32]).expect("key");
        let public = PublicIdentity::new(IdentityId::from_u128(1), public_key, 0);
        let identity = Identity::new(public, Some(profile), Timestamp::UNIX_EPOCH);
        IdentityRepository::insert(&mut store, &identity).expect("insert");
        assert_eq!(IdentityRepository::load(&store).expect("load"), Some(identity));
    }

    #[test]
    fn avatar_genome_is_content_addressed_and_survives_lookup() {
        let key = DatabaseKey::new([0x26; 32]);
        let mut store = SqlCipherStore::open_in_memory(&key).expect("open store");
        let record = AvatarGenomeRecord {
            genome_hash: [4; 32],
            schema_version: 1,
            generator_version: "4.7.0".into(),
            catalog_version: "4.4".into(),
            compressed_genome: vec![1, 2, 3],
        };
        RelationshipRepository::upsert_avatar_genome(
            &mut store,
            record.clone(),
            Timestamp::UNIX_EPOCH,
        )
        .expect("save");
        assert_eq!(store.avatar_genome(record.genome_hash).expect("lookup"), Some(record));
    }

    #[test]
    fn contact_conversation_and_credential_round_trip_through_sqlcipher() {
        let key = DatabaseKey::new([0x25; 32]);
        let mut store = SqlCipherStore::open_in_memory(&key).expect("open store");
        let contact = Contact::new(
            ContactId::from_u128(21),
            remote_identity(),
            ContactRoute::for_provider_endpoint(OpaqueId::from_u128(22), "tor", vec![1, 2, 3, 4])
                .expect("route"),
            Timestamp::UNIX_EPOCH,
        );
        let conversation = DirectConversation::new(
            ConversationId::from_u128(23),
            contact.id(),
            Timestamp::UNIX_EPOCH,
        );
        let credential =
            PeerCredential::new(contact.id(), OpaqueId::from_u128(24), OpaqueId::from_u128(25))
                .expect("credential");
        let avatar = AvatarGenomeRecord {
            genome_hash: [8; 32],
            schema_version: 1,
            generator_version: "4.7.0".into(),
            catalog_version: "4.4".into(),
            compressed_genome: vec![7, 8, 9],
        };
        let local_avatar = AvatarGenomeRecord {
            genome_hash: [6; 32],
            schema_version: 1,
            generator_version: "4.7.0".into(),
            catalog_version: "4.4".into(),
            compressed_genome: vec![4, 5, 6],
        };
        RelationshipRepository::upsert_avatar_genome(
            &mut store,
            local_avatar.clone(),
            Timestamp::UNIX_EPOCH,
        )
        .expect("local avatar");
        store
            .insert_pairing_result(
                contact.clone(),
                conversation.clone(),
                "Peer name",
                credential,
                Some(avatar.clone()),
                Timestamp::from_unix_millis(100).expect("timestamp"),
            )
            .expect("insert relationship");
        assert_eq!(
            ContactRepository::get(&store, contact.id()).expect("get contact"),
            Some(contact.clone())
        );
        assert_eq!(
            ConversationRepository::get(&store, conversation.id()).expect("get conversation"),
            Some(conversation)
        );
        assert_eq!(
            PeerCredentialRepository::credential_for_contact(&store, credential.contact_id())
                .expect("get credential"),
            Some(credential)
        );
        let stored_name: String = store
            .backend
            .connection()
            .query_row(
                include_str!("../sql/queries/contact_metadata_name.sql"),
                params![contact.id().to_opaque().as_bytes().as_slice()],
                |row| row.get(0),
            )
            .expect("pairing display name");
        assert_eq!(stored_name, "Peer name");
        assert_eq!(
            RelationshipRepository::avatar_genome_for_identity(
                &store,
                contact.remote_identity().identity_id(),
            )
            .expect("contact avatar"),
            Some(avatar),
        );
        assert_eq!(
            RelationshipRepository::local_avatar_genome(&store).expect("local avatar lookup"),
            Some(local_avatar),
        );
    }
}
