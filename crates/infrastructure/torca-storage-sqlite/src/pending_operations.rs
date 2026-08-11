use std::path::Path;

use rusqlite::params;
use torca_client_application::{
    PendingOperation, PendingOperationKind, PendingOperationStore, PendingOperationStoreError,
};
use torca_foundation::OpaqueId;

use crate::{DatabaseKey, SqlCipherBackend, SqlCipherStoreOpenError};

const ENQUEUE_SQL: &str = include_str!("../sql/commands/pending_operation_enqueue.sql");
const COMPLETE_SQL: &str = include_str!("../sql/commands/pending_operation_complete.sql");
const RESCHEDULE_SQL: &str = include_str!("../sql/commands/pending_operation_reschedule.sql");
const DUE_SQL: &str = include_str!("../sql/queries/pending_operations_due.sql");

#[derive(Debug)]
pub enum PendingOperationStorageError {
    Open(SqlCipherStoreOpenError),
}

impl core::fmt::Display for PendingOperationStorageError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for PendingOperationStorageError {}

pub struct SqlCipherPendingOperationStore {
    backend: SqlCipherBackend,
}

impl SqlCipherPendingOperationStore {
    pub fn open(
        path: impl AsRef<Path>,
        key: &DatabaseKey,
    ) -> Result<Self, PendingOperationStorageError> {
        let backend = SqlCipherBackend::open(path, key).map_err(|error| {
            PendingOperationStorageError::Open(SqlCipherStoreOpenError::Backend(error))
        })?;
        Self::bootstrap(backend)
    }

    #[cfg(test)]
    fn open_in_memory(key: &DatabaseKey) -> Result<Self, PendingOperationStorageError> {
        let backend = SqlCipherBackend::open_in_memory(key).map_err(|error| {
            PendingOperationStorageError::Open(SqlCipherStoreOpenError::Backend(error))
        })?;
        Self::bootstrap(backend)
    }

    fn bootstrap(backend: SqlCipherBackend) -> Result<Self, PendingOperationStorageError> {
        let mut kernel = crate::StorageKernel::new(backend);
        kernel.bootstrap().map_err(|error| {
            PendingOperationStorageError::Open(SqlCipherStoreOpenError::Migration(error))
        })?;
        Ok(Self { backend: kernel.into_backend() })
    }
}

impl PendingOperationStore for SqlCipherPendingOperationStore {
    fn enqueue(&mut self, operation: PendingOperation) -> Result<(), PendingOperationStoreError> {
        let (kind, text, binary) = encode_kind(&operation.kind);
        self.backend
            .connection()
            .execute(
                ENQUEUE_SQL,
                params![
                    operation.id.to_string(),
                    operation.resource_id.to_string(),
                    kind,
                    text,
                    binary,
                    i64::from(operation.attempts),
                    operation.next_attempt_at_ms,
                    operation.created_at_ms,
                    operation.last_error,
                ],
            )
            .map_err(|_| PendingOperationStoreError::Unavailable)?;
        Ok(())
    }

    fn due(
        &self,
        now_ms: i64,
        limit: usize,
    ) -> Result<Vec<PendingOperation>, PendingOperationStoreError> {
        let limit = i64::try_from(limit).map_err(|_| PendingOperationStoreError::Unavailable)?;
        let mut statement = self
            .backend
            .connection()
            .prepare(DUE_SQL)
            .map_err(|_| PendingOperationStoreError::Unavailable)?;
        let rows = statement
            .query_map(params![now_ms, limit], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<Vec<u8>>>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, Option<String>>(8)?,
                ))
            })
            .map_err(|_| PendingOperationStoreError::Unavailable)?;
        rows.map(|row| {
            let (id, resource_id, kind, text, binary, attempts, next, created, error) =
                row.map_err(|_| PendingOperationStoreError::Unavailable)?;
            Ok(PendingOperation {
                id: id.parse().map_err(|_| PendingOperationStoreError::Unavailable)?,
                resource_id: resource_id
                    .parse()
                    .map_err(|_| PendingOperationStoreError::Unavailable)?,
                kind: decode_kind(&kind, text, binary)?,
                attempts: u32::try_from(attempts)
                    .map_err(|_| PendingOperationStoreError::Unavailable)?,
                next_attempt_at_ms: next,
                created_at_ms: created,
                last_error: error,
            })
        })
        .collect()
    }

    fn complete(&mut self, id: OpaqueId) -> Result<(), PendingOperationStoreError> {
        self.backend
            .connection()
            .execute(COMPLETE_SQL, [id.to_string()])
            .map_err(|_| PendingOperationStoreError::Unavailable)?;
        Ok(())
    }

    fn reschedule(
        &mut self,
        id: OpaqueId,
        attempts: u32,
        next_attempt_at_ms: i64,
        error: &str,
    ) -> Result<(), PendingOperationStoreError> {
        self.backend
            .connection()
            .execute(
                RESCHEDULE_SQL,
                params![id.to_string(), i64::from(attempts), next_attempt_at_ms, error],
            )
            .map_err(|_| PendingOperationStoreError::Unavailable)?;
        Ok(())
    }
}

fn encode_kind(kind: &PendingOperationKind) -> (&'static str, Option<String>, Option<Vec<u8>>) {
    match kind {
        PendingOperationKind::CreatePairing => ("pairing.create", None, None),
        PendingOperationKind::JoinPairing { code, ticket } => {
            ("pairing.join", Some(code.clone()), ticket.map(|value| value.to_vec()))
        }
        PendingOperationKind::ApprovePairing => ("pairing.approve", None, None),
        PendingOperationKind::RejectPairing => ("pairing.reject", None, None),
        PendingOperationKind::CancelPairing => ("pairing.cancel", None, None),
        PendingOperationKind::RenameContact { display_name } => {
            ("contact.rename", Some(display_name.clone()), None)
        }
        PendingOperationKind::VerifyContact => ("contact.verify", None, None),
        PendingOperationKind::ResetContactVerification => {
            ("contact.verification.reset", None, None)
        }
        PendingOperationKind::BlockContact => ("contact.block", None, None),
        PendingOperationKind::UnblockContact => ("contact.unblock", None, None),
        PendingOperationKind::RemoveContact => ("contact.remove", None, None),
        PendingOperationKind::ClearConversationHistory => {
            ("conversation.history.clear", None, None)
        }
        PendingOperationKind::MarkConversationRead => ("conversation.mark_read", None, None),
    }
}

fn decode_kind(
    kind: &str,
    text: Option<String>,
    binary: Option<Vec<u8>>,
) -> Result<PendingOperationKind, PendingOperationStoreError> {
    match kind {
        "pairing.create" => Ok(PendingOperationKind::CreatePairing),
        "pairing.join" => {
            let ticket = binary
                .map(|bytes| <[u8; 16]>::try_from(bytes.as_slice()))
                .transpose()
                .map_err(|_| PendingOperationStoreError::Unavailable)?;
            Ok(PendingOperationKind::JoinPairing {
                code: text.ok_or(PendingOperationStoreError::Unavailable)?,
                ticket,
            })
        }
        "pairing.approve" => Ok(PendingOperationKind::ApprovePairing),
        "pairing.reject" => Ok(PendingOperationKind::RejectPairing),
        "pairing.cancel" => Ok(PendingOperationKind::CancelPairing),
        "contact.rename" => Ok(PendingOperationKind::RenameContact {
            display_name: text.ok_or(PendingOperationStoreError::Unavailable)?,
        }),
        "contact.verify" => Ok(PendingOperationKind::VerifyContact),
        "contact.verification.reset" => Ok(PendingOperationKind::ResetContactVerification),
        "contact.block" => Ok(PendingOperationKind::BlockContact),
        "contact.unblock" => Ok(PendingOperationKind::UnblockContact),
        "contact.remove" => Ok(PendingOperationKind::RemoveContact),
        "conversation.history.clear" => Ok(PendingOperationKind::ClearConversationHistory),
        "conversation.mark_read" => Ok(PendingOperationKind::MarkConversationRead),
        _ => Err(PendingOperationStoreError::Unavailable),
    }
}

#[cfg(test)]
mod tests {
    use super::SqlCipherPendingOperationStore;
    use crate::DatabaseKey;
    use torca_client_application::{PendingOperation, PendingOperationKind, PendingOperationStore};

    #[test]
    fn pairing_operation_survives_store_reopen_boundary() {
        let mut store =
            SqlCipherPendingOperationStore::open_in_memory(&DatabaseKey::new([0x41; 32]))
                .expect("pending store");
        let id = "00000000000000000000000000000011".parse().expect("id");
        store
            .enqueue(PendingOperation {
                id,
                resource_id: id,
                kind: PendingOperationKind::JoinPairing {
                    code: "ABC123".into(),
                    ticket: Some([7; 16]),
                },
                attempts: 0,
                next_attempt_at_ms: 10,
                created_at_ms: 10,
                last_error: None,
            })
            .expect("enqueue");
        let due = store.due(10, 8).expect("due operations");
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].resource_id, id);
        assert!(matches!(due[0].kind, PendingOperationKind::JoinPairing { .. }));
        assert_eq!(store.all().expect("all operations").len(), 1);
    }
}
