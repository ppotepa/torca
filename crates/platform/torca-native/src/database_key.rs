use torca_crypto::{CryptoProvider, ProtectedSecretStore};

use crate::composition::NativeCompositionError;

pub(crate) const DATABASE_KEY_HANDLE: torca_identity::KeyId =
    torca_identity::KeyId::from_u128(0x746f7263615f64625f6b6579);

pub(crate) fn load_or_create_database_key<C: CryptoProvider>(
    store: &mut dyn ProtectedSecretStore,
    handle: torca_identity::KeyId,
    mut crypto: C,
) -> Result<torca_storage_sqlite::DatabaseKey, NativeCompositionError> {
    match store.load(handle).map_err(|error| secret_error("load database key", &error))? {
        Some(mut bytes) => {
            if bytes.len() != 32 {
                bytes.fill(0);
                return Err(NativeCompositionError::new(
                    "protected database key has an invalid length",
                ));
            }
            let mut key = [0_u8; 32];
            key.copy_from_slice(&bytes);
            bytes.fill(0);
            Ok(torca_storage_sqlite::DatabaseKey::new(key))
        }
        None => {
            let mut key = [0_u8; 32];
            crypto
                .fill_random(&mut key)
                .map_err(|_| NativeCompositionError::new("database key generation failed"))?;
            if let Err(error) = store.insert(handle, &key) {
                key.fill(0);
                return Err(secret_error("persist database key", &error));
            }
            Ok(torca_storage_sqlite::DatabaseKey::new(key))
        }
    }
}

fn secret_error(
    operation: &str,
    error: &torca_crypto::ProtectedSecretStoreError,
) -> NativeCompositionError {
    NativeCompositionError::new(format!("{operation} failed: {error}"))
}
