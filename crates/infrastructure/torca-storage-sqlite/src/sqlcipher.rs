use core::fmt;
use std::path::Path;

use rusqlite::{Connection, OpenFlags};

use crate::{StorageBackend, StorageBackendError};

/// Raw 256-bit database key with redacted diagnostics and best-effort zeroing.
#[must_use]
#[derive(Eq, PartialEq)]
pub struct DatabaseKey([u8; 32]);

impl DatabaseKey {
    /// Creates a database key from caller-protected bytes.
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    fn expose(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for DatabaseKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("DatabaseKey([REDACTED])")
    }
}

impl Drop for DatabaseKey {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}

/// Concrete `rusqlite` backend compiled with bundled SQLCipher and vendored OpenSSL.
pub struct SqlCipherBackend {
    connection: Connection,
    in_transaction: bool,
    cipher_version: String,
}

impl SqlCipherBackend {
    /// Opens or creates an encrypted database and verifies that SQLCipher is active.
    pub fn open(path: impl AsRef<Path>, key: &DatabaseKey) -> Result<Self, StorageBackendError> {
        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_FULL_MUTEX;
        let connection = Connection::open_with_flags(path, flags).map_err(map_sqlite_error)?;
        configure_sqlcipher_logging(&connection)?;
        Self::from_connection(connection, key)
    }

    /// Opens an encrypted in-memory database, primarily for integration tests.
    pub fn open_in_memory(key: &DatabaseKey) -> Result<Self, StorageBackendError> {
        let connection = Connection::open_in_memory().map_err(map_sqlite_error)?;
        configure_sqlcipher_logging(&connection)?;
        Self::from_connection(connection, key)
    }

    fn from_connection(
        connection: Connection,
        key: &DatabaseKey,
    ) -> Result<Self, StorageBackendError> {
        apply_database_key(&connection, key.expose())?;
        let cipher_version = verify_sqlcipher(&connection)?;
        Ok(Self { connection, in_transaction: false, cipher_version })
    }

    /// Returns the active SQLCipher version string.
    pub fn cipher_version(&self) -> &str {
        &self.cipher_version
    }

    pub const fn connection(&self) -> &Connection {
        &self.connection
    }
}

fn configure_sqlcipher_logging(connection: &Connection) -> Result<(), StorageBackendError> {
    // SQLCipher defaults to WARN and emits one message for every failed best-effort mlock call.
    // Windows and Android commonly impose a small lock quota, so that default can flood logs
    // even though encryption remains operational. Preserve memory security and all ERROR-level
    // diagnostics while suppressing the non-fatal per-allocation warnings.
    connection.execute_batch("PRAGMA cipher_log_level = ERROR;").map_err(map_sqlite_error)
}

impl StorageBackend for SqlCipherBackend {
    fn schema_version(&self) -> Result<u32, StorageBackendError> {
        self.connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(map_sqlite_error)
    }

    fn execute_connection_batch(&mut self, sql: &'static str) -> Result<(), StorageBackendError> {
        if self.in_transaction {
            return Err(StorageBackendError("connection batch cannot run in transaction".into()));
        }
        self.connection.execute_batch(sql).map_err(map_sqlite_error)
    }

    fn begin(&mut self) -> Result<(), StorageBackendError> {
        if self.in_transaction {
            return Err(StorageBackendError("transaction already active".into()));
        }
        self.connection.execute_batch("BEGIN IMMEDIATE;").map_err(map_sqlite_error)?;
        self.in_transaction = true;
        Ok(())
    }

    fn execute_batch(&mut self, sql: &'static str) -> Result<(), StorageBackendError> {
        if !self.in_transaction {
            return Err(StorageBackendError("no active transaction".into()));
        }
        self.connection.execute_batch(sql).map_err(map_sqlite_error)
    }

    fn set_schema_version(&mut self, version: u32) -> Result<(), StorageBackendError> {
        if !self.in_transaction {
            return Err(StorageBackendError("no active transaction".into()));
        }
        self.connection.pragma_update(None, "user_version", version).map_err(map_sqlite_error)
    }

    fn commit(&mut self) -> Result<(), StorageBackendError> {
        if !self.in_transaction {
            return Err(StorageBackendError("no active transaction".into()));
        }
        self.connection.execute_batch("COMMIT;").map_err(map_sqlite_error)?;
        self.in_transaction = false;
        Ok(())
    }

    fn rollback(&mut self) -> Result<(), StorageBackendError> {
        if self.in_transaction {
            self.connection.execute_batch("ROLLBACK;").map_err(map_sqlite_error)?;
            self.in_transaction = false;
        }
        Ok(())
    }
}

fn apply_database_key(connection: &Connection, key: &[u8; 32]) -> Result<(), StorageBackendError> {
    let mut hex = String::with_capacity(64);
    for byte in key {
        use fmt::Write as _;
        write!(&mut hex, "{byte:02x}")
            .map_err(|_| StorageBackendError("database key encoding failed".into()))?;
    }

    // SQLCipher raw-key syntax requires the x'...' value inside a quoted PRAGMA value.
    // The generated content is fixed-length lowercase hexadecimal and contains no user text.
    connection.execute_batch(&format!("PRAGMA key = \"x'{hex}'\";")).map_err(map_sqlite_error)
}

fn verify_sqlcipher(connection: &Connection) -> Result<String, StorageBackendError> {
    let version: String = connection
        .query_row("PRAGMA cipher_version;", [], |row| row.get(0))
        .map_err(|_| StorageBackendError("SQLCipher support is unavailable".into()))?;
    if version.trim().is_empty() {
        return Err(StorageBackendError("SQLCipher returned an empty cipher version".into()));
    }

    connection
        .query_row(include_str!("../sql/queries/verify_database.sql"), [], |row| {
            row.get::<_, i64>(0)
        })
        .map_err(|_| StorageBackendError("database key verification failed".into()))?;

    Ok(version)
}

pub(crate) fn map_sqlite_error(error: rusqlite::Error) -> StorageBackendError {
    let code = error
        .sqlite_error_code()
        .map_or_else(|| "unknown".to_owned(), |value| format!("{value:?}"));
    StorageBackendError(format!("SQLite operation failed ({code})"))
}

#[cfg(test)]
mod tests {
    use crate::{DatabaseKey, SqlCipherBackend, StorageKernel, migrations};

    #[test]
    fn bundled_sqlcipher_bootstraps_the_embedded_schema() {
        let key = DatabaseKey::new([0x42; 32]);
        let backend = SqlCipherBackend::open_in_memory(&key).expect("open SQLCipher");
        assert!(!backend.cipher_version().is_empty());

        let mut kernel = StorageKernel::new(backend);
        assert_eq!(
            kernel.bootstrap().expect("migrate"),
            migrations().last().expect("migration registry is non-empty").version
        );
    }
}
