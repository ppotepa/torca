use std::collections::BTreeMap;
use std::path::Path;

use rusqlite::{OptionalExtension, params};
use torca_contacts::ContactId;
use torca_conversations::ConversationId;
use torca_foundation::{OpaqueId, Timestamp};

use crate::{DatabaseKey, SqlCipherBackend, StorageBackend, StorageKernel};

const UPSERT_NAME_SQL: &str = include_str!("../sql/commands/contact_metadata_upsert.sql");
const LIST_NAMES_SQL: &str = include_str!("../sql/queries/contact_metadata_list.sql");
const UPDATE_STATUS_SQL: &str = include_str!("../sql/commands/contact_status_update.sql");
const CONTACT_CONTEXT_SQL: &str = include_str!("../sql/queries/contact_admin_context.sql");
const CONVERSATION_CONTACT_SQL: &str = include_str!("../sql/queries/conversation_contact_id.sql");
const ATTACHMENTS_FOR_CONVERSATION_SQL: &str =
    include_str!("../sql/queries/attachments_for_conversation.sql");
const DELETE_CONTROL_SQL: &str = include_str!("../sql/commands/contact_control_delete.sql");
const DELETE_MESSAGES_SQL: &str = include_str!("../sql/commands/conversation_messages_delete.sql");
const DELETE_CONTACT_SQL: &str = include_str!("../sql/commands/contact_delete.sql");
const VERIFY_CONTACT_SQL: &str = include_str!("../sql/commands/contact_verification_upsert.sql");
const RESET_VERIFICATION_SQL: &str =
    include_str!("../sql/commands/contact_verification_delete.sql");
const LIST_VERIFICATIONS_SQL: &str = include_str!("../sql/queries/contact_verification_list.sql");

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RelationshipAdminError {
    Storage,
    NotFound,
    InvalidTransition,
    InvalidDisplayName,
    InvalidStoredId,
}
impl core::fmt::Display for RelationshipAdminError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for RelationshipAdminError {}

#[must_use]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RelationshipCleanup {
    pub attachment_ids: Vec<OpaqueId>,
    pub peer_secret_handle: Option<OpaqueId>,
}

pub struct SqlCipherRelationshipAdmin {
    backend: SqlCipherBackend,
}
impl SqlCipherRelationshipAdmin {
    pub fn open(path: impl AsRef<Path>, key: &DatabaseKey) -> Result<Self, RelationshipAdminError> {
        let backend =
            SqlCipherBackend::open(path, key).map_err(|_| RelationshipAdminError::Storage)?;
        let mut kernel = StorageKernel::new(backend);
        kernel.bootstrap().map_err(|_| RelationshipAdminError::Storage)?;
        Ok(Self { backend: kernel.into_backend() })
    }

    pub fn contact_names(&self) -> Result<BTreeMap<ContactId, String>, RelationshipAdminError> {
        let mut statement = self
            .backend
            .connection()
            .prepare(LIST_NAMES_SQL)
            .map_err(|_| RelationshipAdminError::Storage)?;
        let rows = statement
            .query_map([], |row| Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, Option<String>>(1)?)))
            .map_err(|_| RelationshipAdminError::Storage)?;
        let mut values = BTreeMap::new();
        for row in rows {
            let (id, name) = row.map_err(|_| RelationshipAdminError::Storage)?;
            let id = ContactId::from_opaque(OpaqueId::from_bytes(fixed16(id)?));
            if let Some(name) = name {
                values.insert(id, name);
            }
        }
        Ok(values)
    }

    pub fn contact_verifications(
        &self,
    ) -> Result<BTreeMap<ContactId, (bool, Option<Timestamp>)>, RelationshipAdminError> {
        let mut statement = self
            .backend
            .connection()
            .prepare(LIST_VERIFICATIONS_SQL)
            .map_err(|_| RelationshipAdminError::Storage)?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                ))
            })
            .map_err(|_| RelationshipAdminError::Storage)?;
        let mut values = BTreeMap::new();
        for row in rows {
            let (id, verified, verified_at_ms) =
                row.map_err(|_| RelationshipAdminError::Storage)?;
            let id = ContactId::from_opaque(OpaqueId::from_bytes(fixed16(id)?));
            let verified_at = verified_at_ms
                .map(Timestamp::from_unix_millis)
                .transpose()
                .map_err(|_| RelationshipAdminError::Storage)?;
            values.insert(id, (verified != 0, verified_at));
        }
        Ok(values)
    }

    pub fn verify_contact(
        &mut self,
        contact_id: ContactId,
        at: Timestamp,
    ) -> Result<(), RelationshipAdminError> {
        let id = contact_id.to_opaque().into_bytes();
        let changed = self
            .backend
            .connection()
            .execute(VERIFY_CONTACT_SQL, params![id.as_slice(), at.to_unix_millis()])
            .map_err(|_| RelationshipAdminError::Storage)?;
        if changed == 1 { Ok(()) } else { Err(RelationshipAdminError::NotFound) }
    }

    pub fn reset_contact_verification(
        &mut self,
        contact_id: ContactId,
    ) -> Result<(), RelationshipAdminError> {
        let id = contact_id.to_opaque().into_bytes();
        self.backend
            .connection()
            .execute(RESET_VERIFICATION_SQL, params![id.as_slice()])
            .map_err(|_| RelationshipAdminError::Storage)?;
        Ok(())
    }

    pub fn rename_contact(
        &mut self,
        contact_id: ContactId,
        display_name: String,
        at: Timestamp,
    ) -> Result<(), RelationshipAdminError> {
        let display_name = validate_display_name(display_name)?;
        let id = contact_id.to_opaque().into_bytes();
        let changed = self
            .backend
            .connection()
            .execute(UPSERT_NAME_SQL, params![id.as_slice(), display_name, at.to_unix_millis()])
            .map_err(|_| RelationshipAdminError::Storage)?;
        if changed == 0 {
            return Err(RelationshipAdminError::NotFound);
        }
        Ok(())
    }

    pub fn block_contact(
        &mut self,
        contact_id: ContactId,
        at: Timestamp,
    ) -> Result<(), RelationshipAdminError> {
        self.transition_status(contact_id, 0, 1, at)
    }

    pub fn unblock_contact(
        &mut self,
        contact_id: ContactId,
        at: Timestamp,
    ) -> Result<(), RelationshipAdminError> {
        self.transition_status(contact_id, 1, 0, at)
    }

    pub fn clear_conversation_history(
        &mut self,
        conversation_id: ConversationId,
    ) -> Result<RelationshipCleanup, RelationshipAdminError> {
        let conversation = conversation_id.to_opaque().into_bytes();
        let contact: Option<Vec<u8>> = self
            .backend
            .connection()
            .query_row(CONVERSATION_CONTACT_SQL, params![conversation.as_slice()], |row| row.get(0))
            .optional()
            .map_err(|_| RelationshipAdminError::Storage)?;
        let contact = contact.ok_or(RelationshipAdminError::NotFound)?;
        let contact_id = OpaqueId::from_bytes(fixed16(contact)?);
        let attachments = self.attachments_for_conversation(conversation_id)?;
        self.backend.begin().map_err(|_| RelationshipAdminError::Storage)?;
        let result = (|| {
            self.backend
                .connection()
                .execute(DELETE_CONTROL_SQL, params![contact_id.as_bytes().as_slice()])
                .map_err(|_| RelationshipAdminError::Storage)?;
            self.backend
                .connection()
                .execute(DELETE_MESSAGES_SQL, params![conversation.as_slice()])
                .map_err(|_| RelationshipAdminError::Storage)?;
            Ok::<(), RelationshipAdminError>(())
        })();
        finish_transaction(&mut self.backend, result)?;
        Ok(RelationshipCleanup { attachment_ids: attachments, peer_secret_handle: None })
    }

    pub fn remove_contact(
        &mut self,
        contact_id: ContactId,
    ) -> Result<RelationshipCleanup, RelationshipAdminError> {
        let contact = contact_id.to_opaque().into_bytes();
        let context = self
            .backend
            .connection()
            .query_row(CONTACT_CONTEXT_SQL, params![contact.as_slice()], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<Vec<u8>>>(1)?,
                    row.get::<_, Option<Vec<u8>>>(2)?,
                ))
            })
            .optional()
            .map_err(|_| RelationshipAdminError::Storage)?
            .ok_or(RelationshipAdminError::NotFound)?;
        let conversation_id = context
            .1
            .map(fixed16)
            .transpose()?
            .map(OpaqueId::from_bytes)
            .map(ConversationId::from_opaque);
        let secret_handle = context.2.map(fixed16).transpose()?.map(OpaqueId::from_bytes);
        let attachments = match conversation_id {
            Some(id) => self.attachments_for_conversation(id)?,
            None => Vec::new(),
        };

        self.backend.begin().map_err(|_| RelationshipAdminError::Storage)?;
        let result = (|| {
            self.backend
                .connection()
                .execute(DELETE_CONTROL_SQL, params![contact.as_slice()])
                .map_err(|_| RelationshipAdminError::Storage)?;
            if let Some(conversation_id) = conversation_id {
                let conversation = conversation_id.to_opaque().into_bytes();
                self.backend
                    .connection()
                    .execute(DELETE_MESSAGES_SQL, params![conversation.as_slice()])
                    .map_err(|_| RelationshipAdminError::Storage)?;
            }
            let deleted = self
                .backend
                .connection()
                .execute(DELETE_CONTACT_SQL, params![contact.as_slice()])
                .map_err(|_| RelationshipAdminError::Storage)?;
            if deleted != 1 {
                return Err(RelationshipAdminError::NotFound);
            }
            Ok::<(), RelationshipAdminError>(())
        })();
        finish_transaction(&mut self.backend, result)?;
        Ok(RelationshipCleanup { attachment_ids: attachments, peer_secret_handle: secret_handle })
    }

    fn transition_status(
        &mut self,
        contact_id: ContactId,
        expected: i64,
        next: i64,
        at: Timestamp,
    ) -> Result<(), RelationshipAdminError> {
        let id = contact_id.to_opaque().into_bytes();
        let changed = self
            .backend
            .connection()
            .execute(UPDATE_STATUS_SQL, params![id.as_slice(), next, at.to_unix_millis(), expected])
            .map_err(|_| RelationshipAdminError::Storage)?;
        if changed == 1 { Ok(()) } else { Err(RelationshipAdminError::InvalidTransition) }
    }

    fn attachments_for_conversation(
        &self,
        conversation_id: ConversationId,
    ) -> Result<Vec<OpaqueId>, RelationshipAdminError> {
        let conversation = conversation_id.to_opaque().into_bytes();
        let mut statement = self
            .backend
            .connection()
            .prepare(ATTACHMENTS_FOR_CONVERSATION_SQL)
            .map_err(|_| RelationshipAdminError::Storage)?;
        let rows = statement
            .query_map(params![conversation.as_slice()], |row| row.get::<_, Vec<u8>>(0))
            .map_err(|_| RelationshipAdminError::Storage)?;
        rows.map(|row| {
            let value = row.map_err(|_| RelationshipAdminError::Storage)?;
            Ok(OpaqueId::from_bytes(fixed16(value)?))
        })
        .collect()
    }
}

fn validate_display_name(value: String) -> Result<String, RelationshipAdminError> {
    let trimmed = value.trim();
    let count = trimmed.chars().count();
    if count == 0 || count > 64 || trimmed.chars().any(char::is_control) {
        return Err(RelationshipAdminError::InvalidDisplayName);
    }
    Ok(trimmed.to_owned())
}

fn fixed16(value: Vec<u8>) -> Result<[u8; 16], RelationshipAdminError> {
    value.try_into().map_err(|_| RelationshipAdminError::InvalidStoredId)
}

fn finish_transaction(
    backend: &mut SqlCipherBackend,
    result: Result<(), RelationshipAdminError>,
) -> Result<(), RelationshipAdminError> {
    match result {
        Ok(()) => backend.commit().map_err(|_| RelationshipAdminError::Storage),
        Err(error) => {
            let _ = backend.rollback();
            Err(error)
        }
    }
}
