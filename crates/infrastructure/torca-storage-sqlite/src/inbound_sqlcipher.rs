use std::path::Path;

use rusqlite::{ErrorCode, params};
use torca_delivery::{DurableDeliveryError, InboundMessageStore};
use torca_foundation::Timestamp;
use torca_messaging::{Message, MessageDirection, MessageStatus};

use crate::{
    DatabaseKey, MigrationError, SqlCipherBackend, StorageBackend, StorageBackendError,
    StorageKernel, messaging_sql,
};

/// Failure while opening and migrating inbound durable storage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SqlCipherInboundStoreOpenError {
    Backend(StorageBackendError),
    Migration(MigrationError),
}
impl core::fmt::Display for SqlCipherInboundStoreOpenError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for SqlCipherInboundStoreOpenError {}
impl From<StorageBackendError> for SqlCipherInboundStoreOpenError {
    fn from(value: StorageBackendError) -> Self {
        Self::Backend(value)
    }
}
impl From<MigrationError> for SqlCipherInboundStoreOpenError {
    fn from(value: MigrationError) -> Self {
        Self::Migration(value)
    }
}

/// SQLCipher adapter that commits inbound deduplication and message persistence atomically.
pub struct SqlCipherInboundStore {
    backend: SqlCipherBackend,
}
impl SqlCipherInboundStore {
    pub fn open(
        path: impl AsRef<Path>,
        key: &DatabaseKey,
    ) -> Result<Self, SqlCipherInboundStoreOpenError> {
        let backend = SqlCipherBackend::open(path, key)?;
        Self::bootstrap(backend)
    }

    pub fn open_in_memory(key: &DatabaseKey) -> Result<Self, SqlCipherInboundStoreOpenError> {
        let backend = SqlCipherBackend::open_in_memory(key)?;
        Self::bootstrap(backend)
    }

    fn bootstrap(backend: SqlCipherBackend) -> Result<Self, SqlCipherInboundStoreOpenError> {
        let mut kernel = StorageKernel::new(backend);
        kernel.bootstrap()?;
        Ok(Self { backend: kernel.into_backend() })
    }
}

impl InboundMessageStore for SqlCipherInboundStore {
    fn persist_inbound(
        &mut self,
        envelope_id: torca_foundation::OpaqueId,
        message: Message,
    ) -> Result<bool, DurableDeliveryError> {
        if message.direction() != MessageDirection::Inbound
            || message.status() != MessageStatus::Delivered
        {
            return Err(DurableDeliveryError::InvalidState);
        }

        let envelope_id = envelope_id.into_bytes();
        let message_id = message.id().to_opaque().into_bytes();
        let conversation_id = message.conversation_id().to_opaque().into_bytes();
        let reply_id = message.reply_to().map(|reply| reply.message_id.to_opaque().into_bytes());
        let reply = reply_id.as_ref().map(<[u8; 16]>::as_slice);
        let attempt_count = i64::try_from(message.attempts().len())
            .map_err(|_| DurableDeliveryError::Storage("attempt count is too large".into()))?;

        self.backend.begin().map_err(backend_error)?;
        let result = (|| {
            let dedup_inserted = self.backend.connection().execute(
                messaging_sql::INSERT_INBOUND_DEDUP.sql,
                params![envelope_id.as_slice(), message.created_at().to_unix_millis()],
            )?;
            if dedup_inserted == 0 {
                return Ok::<bool, rusqlite::Error>(false);
            }
            self.backend.connection().execute(
                messaging_sql::INSERT_DOMAIN_MESSAGE.sql,
                params![
                    message_id.as_slice(),
                    conversation_id.as_slice(),
                    encode_direction(message.direction()),
                    encode_status(message.status()),
                    message.body().as_str(),
                    reply,
                    message.created_at().to_unix_millis(),
                    message.updated_at().to_unix_millis(),
                    attempt_count,
                    message.sent_at().map(Timestamp::to_unix_millis),
                    message.delivered_at().map(Timestamp::to_unix_millis),
                    message.read_at().map(Timestamp::to_unix_millis),
                ],
            )?;
            Ok(true)
        })();

        match result {
            Ok(inserted) => {
                self.backend.commit().map_err(backend_error)?;
                Ok(inserted)
            }
            Err(error) => {
                let _ = self.backend.rollback();
                if error.sqlite_error_code() == Some(ErrorCode::ConstraintViolation) {
                    Err(DurableDeliveryError::DuplicateMessage)
                } else {
                    Err(storage_error(error))
                }
            }
        }
    }
}

const fn encode_direction(value: MessageDirection) -> i64 {
    match value {
        MessageDirection::Outbound => 0,
        MessageDirection::Inbound => 1,
    }
}
const fn encode_status(value: MessageStatus) -> i64 {
    match value {
        MessageStatus::Queued => 0,
        MessageStatus::Sending => 1,
        MessageStatus::Sent => 2,
        MessageStatus::Delivered => 3,
        MessageStatus::Read => 4,
        MessageStatus::Failed => 5,
        MessageStatus::Cancelled => 6,
    }
}
fn backend_error(error: StorageBackendError) -> DurableDeliveryError {
    DurableDeliveryError::Storage(error.0)
}
fn storage_error(error: rusqlite::Error) -> DurableDeliveryError {
    let code = error
        .sqlite_error_code()
        .map_or_else(|| "unknown".to_owned(), |value| format!("{value:?}"));
    DurableDeliveryError::Storage(format!("SQLite inbound operation failed ({code})"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use torca_conversations::ConversationId;
    use torca_delivery::InboundMessageStore;
    use torca_messaging::{MessageBody, MessageId};

    #[test]
    fn inbound_message_persists_with_lifecycle_columns() {
        let mut store = SqlCipherInboundStore::open_in_memory(&DatabaseKey::new([0x73; 32]))
            .expect("open inbound store");
        let at = Timestamp::from_unix_millis(42).expect("timestamp");
        let message = Message::inbound(
            MessageId::from_u128(7),
            ConversationId::from_u128(8),
            MessageBody::new("hello").expect("body"),
            None,
            at,
        );

        assert!(
            store
                .persist_inbound(torca_foundation::OpaqueId::from_u128(9), message)
                .expect("persist")
        );
        let delivered: Option<i64> = store
            .backend
            .connection()
            .query_row("SELECT delivered_at_ms FROM messages", [], |row| row.get(0))
            .expect("read lifecycle");
        assert_eq!(delivered, Some(42));
    }
}
