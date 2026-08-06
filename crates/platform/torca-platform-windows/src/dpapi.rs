use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::PathBuf;
use std::ptr::{null, null_mut};

use torca_crypto::{ProtectedSecretStore, ProtectedSecretStoreError};
use torca_identity::KeyId;
use windows_sys::Win32::Foundation::{GetLastError, LocalFree};
use windows_sys::Win32::Security::Cryptography::{
    CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
};

/// Current-user DPAPI store persisted as one protected blob per opaque key handle.
pub struct DpapiFileSecretStore {
    root: PathBuf,
}

impl DpapiFileSecretStore {
    /// Creates a store rooted in a caller-selected application-private directory.
    pub fn new(root: impl Into<PathBuf>) -> Result<Self, ProtectedSecretStoreError> {
        let root = root.into();
        fs::create_dir_all(&root).map_err(io_error)?;
        Ok(Self { root })
    }

    fn path(&self, key_id: KeyId) -> PathBuf {
        self.root.join(format!("{key_id}.dpapi"))
    }

    fn temporary_path(&self, key_id: KeyId) -> PathBuf {
        self.root.join(format!(".{key_id}.{}.tmp", std::process::id()))
    }
}

impl ProtectedSecretStore for DpapiFileSecretStore {
    fn insert(&mut self, key_id: KeyId, secret: &[u8]) -> Result<(), ProtectedSecretStoreError> {
        let target = self.path(key_id);
        if target.exists() {
            return Err(ProtectedSecretStoreError("protected key handle already exists".into()));
        }

        let protected = protect(secret)?;
        let temporary = self.temporary_path(key_id);
        let result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
                .map_err(io_error)?;
            file.write_all(&protected).map_err(io_error)?;
            file.sync_all().map_err(io_error)?;
            fs::rename(&temporary, &target).map_err(io_error)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }

    fn load(&self, key_id: KeyId) -> Result<Option<Vec<u8>>, ProtectedSecretStoreError> {
        let protected = match fs::read(self.path(key_id)) {
            Ok(value) => value,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(io_error(error)),
        };
        unprotect(&protected).map(Some)
    }

    fn delete(&mut self, key_id: KeyId) -> Result<bool, ProtectedSecretStoreError> {
        let path = self.path(key_id);
        let length = match fs::metadata(&path) {
            Ok(metadata) => metadata.len(),
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(io_error(error)),
        };

        if let Ok(mut file) = OpenOptions::new().write(true).open(&path) {
            let zeroes = [0_u8; 4096];
            let mut remaining = length;
            while remaining > 0 {
                let count = usize::try_from(remaining.min(zeroes.len() as u64)).map_err(|_| {
                    ProtectedSecretStoreError("protected file length is invalid".into())
                })?;
                file.write_all(&zeroes[..count]).map_err(io_error)?;
                remaining -= count as u64;
            }
            file.sync_all().map_err(io_error)?;
        }
        fs::remove_file(path).map_err(io_error)?;
        Ok(true)
    }
}

fn protect(secret: &[u8]) -> Result<Vec<u8>, ProtectedSecretStoreError> {
    let length = u32::try_from(secret.len())
        .map_err(|_| ProtectedSecretStoreError("secret is too large for DPAPI".into()))?;
    let input = CRYPT_INTEGER_BLOB { cbData: length, pbData: secret.as_ptr().cast_mut() };
    let mut output = CRYPT_INTEGER_BLOB::default();

    // SAFETY: all pointers refer to live buffers for the duration of the call. Optional pointers
    // are null, the output structure is initialized, and the returned LocalAlloc buffer is owned
    // by `DpapiOutput` immediately after success.
    let success = unsafe {
        CryptProtectData(
            &input,
            null(),
            null(),
            null_mut(),
            null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if success == 0 {
        return Err(dpapi_error("protect"));
    }
    DpapiOutput::new(output).copy_bytes(false)
}

fn unprotect(protected: &[u8]) -> Result<Vec<u8>, ProtectedSecretStoreError> {
    let length = u32::try_from(protected.len())
        .map_err(|_| ProtectedSecretStoreError("protected blob is too large for DPAPI".into()))?;
    let input = CRYPT_INTEGER_BLOB { cbData: length, pbData: protected.as_ptr().cast_mut() };
    let mut output = CRYPT_INTEGER_BLOB::default();

    // SAFETY: all pointers refer to live buffers for the call. No description is requested, and
    // the returned LocalAlloc plaintext buffer is copied, cleared and freed by `DpapiOutput`.
    let success = unsafe {
        CryptUnprotectData(
            &input,
            null_mut(),
            null(),
            null_mut(),
            null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if success == 0 {
        return Err(dpapi_error("unprotect"));
    }
    DpapiOutput::new(output).copy_bytes(true)
}

struct DpapiOutput {
    blob: CRYPT_INTEGER_BLOB,
    clear_on_drop: bool,
}

impl DpapiOutput {
    const fn new(blob: CRYPT_INTEGER_BLOB) -> Self {
        Self { blob, clear_on_drop: false }
    }

    fn copy_bytes(mut self, clear_on_drop: bool) -> Result<Vec<u8>, ProtectedSecretStoreError> {
        self.clear_on_drop = clear_on_drop;
        let length = usize::try_from(self.blob.cbData)
            .map_err(|_| ProtectedSecretStoreError("DPAPI output length is invalid".into()))?;
        if length == 0 {
            return Ok(Vec::new());
        }
        if self.blob.pbData.is_null() {
            return Err(ProtectedSecretStoreError("DPAPI returned a null output buffer".into()));
        }
        // SAFETY: DPAPI returned a buffer of exactly cbData bytes that remains allocated until
        // this owner is dropped.
        let bytes = unsafe { std::slice::from_raw_parts(self.blob.pbData, length) };
        Ok(bytes.to_vec())
    }
}

impl Drop for DpapiOutput {
    fn drop(&mut self) {
        if self.blob.pbData.is_null() {
            return;
        }
        let length = self.blob.cbData as usize;
        // SAFETY: pbData is the LocalAlloc buffer returned by DPAPI. It is valid for cbData bytes,
        // has not been freed, and this owner drops exactly once.
        unsafe {
            if self.clear_on_drop && length > 0 {
                std::ptr::write_bytes(self.blob.pbData, 0, length);
            }
            let _ = LocalFree(self.blob.pbData.cast());
        }
        self.blob.pbData = null_mut();
        self.blob.cbData = 0;
    }
}

fn dpapi_error(operation: &str) -> ProtectedSecretStoreError {
    // SAFETY: GetLastError has no pointer preconditions and is read immediately after failure.
    let code = unsafe { GetLastError() };
    ProtectedSecretStoreError(format!("DPAPI {operation} failed ({code})"))
}

fn io_error(error: std::io::Error) -> ProtectedSecretStoreError {
    ProtectedSecretStoreError(format!(
        "protected secret file operation failed ({:?})",
        error.kind()
    ))
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use torca_crypto::ProtectedSecretStore;
    use torca_identity::KeyId;

    use super::DpapiFileSecretStore;

    #[test]
    fn current_user_dpapi_round_trips_and_deletes_secret() {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock").as_nanos();
        let root = std::env::temp_dir().join(format!("torca-dpapi-{nonce}"));
        let key_id = KeyId::from_u128(1);
        let mut store = DpapiFileSecretStore::new(&root).expect("store");

        store.insert(key_id, b"protected-secret").expect("insert");
        assert_eq!(store.load(key_id).expect("load"), Some(b"protected-secret".to_vec()));
        assert!(store.delete(key_id).expect("delete"));
        assert_eq!(store.load(key_id).expect("missing"), None);
        let _ = std::fs::remove_dir_all(root);
    }
}
