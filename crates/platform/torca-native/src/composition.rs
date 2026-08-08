use core::fmt;

use torca_client_engine::{ClientEngineActor, EngineHandle};
use torca_storage_sqlite::{SqlCipherMessageStore, SqlCipherSecurityProjection};

#[cfg(target_os = "android")]
#[path = "android.rs"]
pub(crate) mod android;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NativeCompositionError(String);
impl NativeCompositionError {
    pub(crate) fn new(message: impl Into<String>) -> Self { Self(message.into()) }
}
impl fmt::Display for NativeCompositionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> core::fmt::Result { formatter.write_str(&self.0) }
}
impl std::error::Error for NativeCompositionError {}

pub(crate) struct ProductionEngineParts {
    pub engine: EngineHandle,
    pub actor: ClientEngineActor,
    pub history: SqlCipherMessageStore,
    pub security: SqlCipherSecurityProjection,
}

pub(crate) const DATABASE_KEY_HANDLE: torca_identity::KeyId =
    torca_identity::KeyId::from_u128(0x746f7263615f64625f6b6579);

#[cfg(windows)]
pub(crate) fn spawn_production_engine() -> Result<ProductionEngineParts, NativeCompositionError> {
    use crate::app_paths::windows_app_root;
    use torca_client_engine::ClientEngine;
    use torca_crypto::{ManagedIdentityKeys, RustCryptoProvider};
    use torca_pairing::InMemoryPairingRepository;
    use torca_platform_windows::DpapiFileSecretStore;
    use torca_storage_sqlite::{SqlCipherReceiptStore, SqlCipherStore};

    let root = windows_app_root()?;
    let database_dir = root.join("data");
    std::fs::create_dir_all(&database_dir)
        .map_err(|error| io_error("create application data directory", &error))?;
    let database_path = database_dir.join("torca.db");

    let mut database_secret_store = DpapiFileSecretStore::new(root.join("secrets").join("database"))
        .map_err(|error| secret_error("open database secret store", &error))?;
    let database_key = load_or_create_database_key(
        &mut database_secret_store,
        DATABASE_KEY_HANDLE,
        RustCryptoProvider,
    )?;

    let identity_repository = SqlCipherStore::open(&database_path, &database_key)
        .map_err(|error| storage_error("open identity repository", &error))?;
    let relationships = SqlCipherStore::open(&database_path, &database_key)
        .map_err(|error| storage_error("open relationship repository", &error))?;
    let messages = SqlCipherMessageStore::open(&database_path, &database_key)
        .map_err(|error| storage_error("open message repository", &error))?;
    let history = SqlCipherMessageStore::open(&database_path, &database_key)
        .map_err(|error| storage_error("open history reader", &error))?;
    let security = SqlCipherSecurityProjection::open(&database_path, &database_key)
        .map_err(|error| storage_error("open security projection", &error))?;
    let receipts = SqlCipherReceiptStore::open(&database_path, &database_key)
        .map_err(|error| storage_error("open receipt repository", &error))?;

    let identity_secret_store = DpapiFileSecretStore::new(root.join("secrets").join("identity"))
        .map_err(|error| secret_error("open identity secret store", &error))?;
    let identity_keys = ManagedIdentityKeys::new(RustCryptoProvider, identity_secret_store);
    let engine = ClientEngine::new(
        identity_repository,
        identity_keys,
        InMemoryPairingRepository::default(),
        relationships,
        messages,
        receipts,
    );
    let (engine, actor) = ClientEngineActor::spawn(engine);
    Ok(ProductionEngineParts { engine, actor, history, security })
}

#[cfg(target_os = "android")]
pub(crate) fn spawn_production_engine() -> Result<ProductionEngineParts, NativeCompositionError> {
    use self::android::{AndroidProtectedSecretStore, database_path};
    use torca_client_engine::ClientEngine;
    use torca_crypto::{ManagedIdentityKeys, RustCryptoProvider};
    use torca_pairing::InMemoryPairingRepository;
    use torca_storage_sqlite::{SqlCipherReceiptStore, SqlCipherStore};

    let database_path = database_path()
        .map_err(|error| secret_error("resolve Android database path", &error))?;
    let mut database_secret_store = AndroidProtectedSecretStore::new("database");
    let database_key = load_or_create_database_key(
        &mut database_secret_store,
        DATABASE_KEY_HANDLE,
        RustCryptoProvider,
    )?;
    let identity_repository = SqlCipherStore::open(&database_path, &database_key)
        .map_err(|error| storage_error("open identity repository", &error))?;
    let relationships = SqlCipherStore::open(&database_path, &database_key)
        .map_err(|error| storage_error("open relationship repository", &error))?;
    let messages = SqlCipherMessageStore::open(&database_path, &database_key)
        .map_err(|error| storage_error("open message repository", &error))?;
    let history = SqlCipherMessageStore::open(&database_path, &database_key)
        .map_err(|error| storage_error("open history reader", &error))?;
    let security = SqlCipherSecurityProjection::open(&database_path, &database_key)
        .map_err(|error| storage_error("open security projection", &error))?;
    let receipts = SqlCipherReceiptStore::open(&database_path, &database_key)
        .map_err(|error| storage_error("open receipt repository", &error))?;
    let identity_keys = ManagedIdentityKeys::new(
        RustCryptoProvider,
        AndroidProtectedSecretStore::new("identity"),
    );
    let engine = ClientEngine::new(
        identity_repository,
        identity_keys,
        InMemoryPairingRepository::default(),
        relationships,
        messages,
        receipts,
    );
    let (engine, actor) = ClientEngineActor::spawn(engine);
    Ok(ProductionEngineParts { engine, actor, history, security })
}

#[cfg(any(windows, target_os = "android"))]
pub(crate) fn load_or_create_database_key<
    S: torca_crypto::ProtectedSecretStore,
    C: torca_crypto::CryptoProvider,
>(
    store: &mut S,
    handle: torca_identity::KeyId,
    mut crypto: C,
) -> Result<torca_storage_sqlite::DatabaseKey, NativeCompositionError> {
    match store.load(handle).map_err(|error| secret_error("load database key", &error))? {
        Some(mut bytes) => {
            if bytes.len() != 32 {
                bytes.fill(0);
                return Err(NativeCompositionError::new("protected database key has an invalid length"));
            }
            let mut key = [0_u8; 32];
            key.copy_from_slice(&bytes);
            bytes.fill(0);
            Ok(torca_storage_sqlite::DatabaseKey::new(key))
        }
        None => {
            let mut key = [0_u8; 32];
            crypto.fill_random(&mut key)
                .map_err(|_| NativeCompositionError::new("database key generation failed"))?;
            if let Err(error) = store.insert(handle, &key) {
                key.fill(0);
                return Err(secret_error("persist database key", &error));
            }
            Ok(torca_storage_sqlite::DatabaseKey::new(key))
        }
    }
}

#[cfg(windows)]
fn io_error(operation: &str, error: &std::io::Error) -> NativeCompositionError {
    NativeCompositionError::new(format!("{operation} failed ({:?})", error.kind()))
}
#[cfg(any(windows, target_os = "android"))]
fn secret_error(
    operation: &str,
    error: &torca_crypto::ProtectedSecretStoreError,
) -> NativeCompositionError {
    NativeCompositionError::new(format!("{operation} failed: {error}"))
}
#[cfg(any(windows, target_os = "android"))]
fn storage_error(operation: &str, error: &impl fmt::Display) -> NativeCompositionError {
    NativeCompositionError::new(format!("{operation} failed: {error}"))
}

#[cfg(not(any(windows, target_os = "android")))]
pub(crate) fn spawn_production_engine() -> Result<ProductionEngineParts, NativeCompositionError> {
    Err(NativeCompositionError::new(
        "production native composition is not implemented for this platform",
    ))
}
