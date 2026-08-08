use std::collections::BTreeMap;
use std::path::Path;

use rusqlite::{OptionalExtension, params};
use torca_contacts::ContactId;
use torca_conversations::ConversationId;
use torca_foundation::{OpaqueId, Timestamp};

use crate::{DatabaseKey, SqlCipherBackend, StorageKernel};

const CONTACT_STATES_SQL: &str = include_str!("../sql/queries/contact_security_states.sql");
const CONVERSATION_STATE_SQL: &str = include_str!("../sql/queries/conversation_security_state.sql");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContactSecurityState {
    Unverified,
    Verified,
    IdentityChanged,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContactSecuritySnapshot {
    pub state: ContactSecurityState,
    pub verified_at: Option<Timestamp>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SecurityProjectionError {
    Storage,
    InvalidStoredId,
    InvalidStoredState,
    NotFound,
}
impl core::fmt::Display for SecurityProjectionError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for SecurityProjectionError {}

pub struct SqlCipherSecurityProjection {
    backend: SqlCipherBackend,
}
impl SqlCipherSecurityProjection {
    pub fn open(path: impl AsRef<Path>, key: &DatabaseKey) -> Result<Self, SecurityProjectionError> {
        let backend = SqlCipherBackend::open(path, key).map_err(|_| SecurityProjectionError::Storage)?;
        let mut kernel = StorageKernel::new(backend);
        kernel.bootstrap().map_err(|_| SecurityProjectionError::Storage)?;
        Ok(Self { backend: kernel.into_backend() })
    }

    pub fn contact_states(
        &self,
    ) -> Result<BTreeMap<ContactId, ContactSecuritySnapshot>, SecurityProjectionError> {
        let mut statement = self.backend.connection().prepare(CONTACT_STATES_SQL)
            .map_err(|_| SecurityProjectionError::Storage)?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, Vec<u8>>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, Option<i64>>(2)?,
            ))
        }).map_err(|_| SecurityProjectionError::Storage)?;
        let mut result = BTreeMap::new();
        for row in rows {
            let (contact_id, state, verified_at_ms) = row.map_err(|_| SecurityProjectionError::Storage)?;
            let contact_id = ContactId::from_opaque(OpaqueId::from_bytes(fixed16(contact_id)?));
            let verified_at = verified_at_ms
                .map(Timestamp::from_unix_millis)
                .transpose()
                .map_err(|_| SecurityProjectionError::Storage)?;
            result.insert(
                contact_id,
                ContactSecuritySnapshot { state: decode_state(state)?, verified_at },
            );
        }
        Ok(result)
    }

    pub fn contact_state(
        &self,
        contact_id: ContactId,
    ) -> Result<ContactSecurityState, SecurityProjectionError> {
        self.contact_states()?
            .get(&contact_id)
            .map(|snapshot| snapshot.state)
            .ok_or(SecurityProjectionError::NotFound)
    }

    pub fn conversation_state(
        &self,
        conversation_id: ConversationId,
    ) -> Result<ContactSecurityState, SecurityProjectionError> {
        let conversation = conversation_id.to_opaque().into_bytes();
        let state = self.backend.connection().query_row(
            CONVERSATION_STATE_SQL,
            params![conversation.as_slice()],
            |row| row.get::<_, i64>(0),
        ).optional().map_err(|_| SecurityProjectionError::Storage)?
            .ok_or(SecurityProjectionError::NotFound)?;
        decode_state(state)
    }

    pub fn requires_reverification(
        &self,
        conversation_id: ConversationId,
    ) -> Result<bool, SecurityProjectionError> {
        self.conversation_state(conversation_id)
            .map(|state| state == ContactSecurityState::IdentityChanged)
    }
}

fn decode_state(value: i64) -> Result<ContactSecurityState, SecurityProjectionError> {
    match value {
        0 => Ok(ContactSecurityState::Unverified),
        1 => Ok(ContactSecurityState::Verified),
        2 => Ok(ContactSecurityState::IdentityChanged),
        _ => Err(SecurityProjectionError::InvalidStoredState),
    }
}

fn fixed16(value: Vec<u8>) -> Result<[u8; 16], SecurityProjectionError> {
    value.try_into().map_err(|_| SecurityProjectionError::InvalidStoredId)
}
