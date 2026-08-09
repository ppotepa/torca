use std::path::Path;

use rusqlite::params;
use torca_foundation::{OpaqueId, Timestamp};
use torca_messaging::MessageId;
use torca_receipts::{Receipt, ReceiptError, ReceiptId, ReceiptKind, ReceiptRepository};

use crate::{
    DatabaseKey, MigrationError, SqlCipherBackend, StorageBackendError, StorageKernel, receipt_sql,
};

/// Failure while opening the concrete receipt repository.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SqlCipherReceiptStoreOpenError {
    Backend(StorageBackendError),
    Migration(MigrationError),
}
impl core::fmt::Display for SqlCipherReceiptStoreOpenError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for SqlCipherReceiptStoreOpenError {}
impl From<StorageBackendError> for SqlCipherReceiptStoreOpenError {
    fn from(value: StorageBackendError) -> Self {
        Self::Backend(value)
    }
}
impl From<MigrationError> for SqlCipherReceiptStoreOpenError {
    fn from(value: MigrationError) -> Self {
        Self::Migration(value)
    }
}

/// SQLCipher-backed receipt repository.
pub struct SqlCipherReceiptStore {
    backend: SqlCipherBackend,
}

impl SqlCipherReceiptStore {
    pub fn open(
        path: impl AsRef<Path>,
        key: &DatabaseKey,
    ) -> Result<Self, SqlCipherReceiptStoreOpenError> {
        let backend = SqlCipherBackend::open(path, key)?;
        Self::bootstrap(backend)
    }

    pub fn open_in_memory(key: &DatabaseKey) -> Result<Self, SqlCipherReceiptStoreOpenError> {
        let backend = SqlCipherBackend::open_in_memory(key)?;
        Self::bootstrap(backend)
    }

    fn bootstrap(backend: SqlCipherBackend) -> Result<Self, SqlCipherReceiptStoreOpenError> {
        let mut kernel = StorageKernel::new(backend);
        kernel.bootstrap()?;
        Ok(Self { backend: kernel.into_backend() })
    }
}

impl ReceiptRepository for SqlCipherReceiptStore {
    fn record(&mut self, receipt: Receipt) -> Result<bool, ReceiptError> {
        let receipt_id = receipt.id.to_opaque().into_bytes();
        let message_id = receipt.message_id.to_opaque().into_bytes();
        let changed = self
            .backend
            .connection()
            .execute(
                receipt_sql::INSERT.sql,
                params![
                    receipt_id.as_slice(),
                    message_id.as_slice(),
                    encode_kind(receipt.kind),
                    receipt.at.to_unix_millis(),
                ],
            )
            .map_err(|_| ReceiptError::RepositoryFailure)?;
        Ok(changed == 1)
    }

    fn for_message(&self, message_id: MessageId) -> Result<Vec<Receipt>, ReceiptError> {
        let message_bytes = message_id.to_opaque().into_bytes();
        let mut statement = self
            .backend
            .connection()
            .prepare(receipt_sql::FOR_MESSAGE.sql)
            .map_err(|_| ReceiptError::RepositoryFailure)?;
        let rows = statement
            .query_map(params![message_bytes.as_slice()], |row| {
                Ok(ReceiptRow {
                    receipt_id: row.get(0)?,
                    kind: row.get(1)?,
                    received_at_ms: row.get(2)?,
                })
            })
            .map_err(|_| ReceiptError::RepositoryFailure)?;
        rows.map(|row| {
            let row = row.map_err(|_| ReceiptError::RepositoryFailure)?;
            let id = ReceiptId::from_opaque(OpaqueId::from_bytes(fixed_16(row.receipt_id)?));
            let kind = decode_kind(row.kind)?;
            let at = Timestamp::from_unix_millis(row.received_at_ms)
                .map_err(|_| ReceiptError::RepositoryFailure)?;
            Ok(Receipt { id, message_id, kind, at })
        })
        .collect()
    }
}

struct ReceiptRow {
    receipt_id: Vec<u8>,
    kind: i64,
    received_at_ms: i64,
}

fn fixed_16(value: Vec<u8>) -> Result<[u8; 16], ReceiptError> {
    value.try_into().map_err(|_| ReceiptError::RepositoryFailure)
}
const fn encode_kind(value: ReceiptKind) -> i64 {
    match value {
        ReceiptKind::Delivered => 0,
        ReceiptKind::Read => 1,
    }
}
fn decode_kind(value: i64) -> Result<ReceiptKind, ReceiptError> {
    match value {
        0 => Ok(ReceiptKind::Delivered),
        1 => Ok(ReceiptKind::Read),
        _ => Err(ReceiptError::RepositoryFailure),
    }
}

#[cfg(test)]
mod tests {
    use torca_messaging::MessageId;
    use torca_receipts::ReceiptRepository;

    use crate::{DatabaseKey, SqlCipherReceiptStore};

    #[test]
    fn unknown_message_has_no_receipts() {
        let key = DatabaseKey::new([0x27; 32]);
        let store = SqlCipherReceiptStore::open_in_memory(&key).expect("open store");
        assert!(store.for_message(MessageId::from_u128(42)).expect("query receipts").is_empty());
    }
}
