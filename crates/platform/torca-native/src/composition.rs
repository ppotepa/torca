use core::fmt;

use torca_client_engine::{ClientEngine, ClientEngineActor, EngineHandle};
use torca_crypto::{ManagedIdentityKeys, ProtectedSecretStore, RustCryptoProvider};
use torca_platform::{PlatformServices, RelayEndpoint, SecretNamespace};
use torca_storage_sqlite::{
    SqlCipherMessageStore, SqlCipherPairingRepository, SqlCipherReceiptStore,
    SqlCipherSecurityProjection, SqlCipherSettingsStore, SqlCipherStore,
};

#[cfg(target_os = "android")]
#[path = "android.rs"]
pub(crate) mod android;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NativeCompositionError(String);
impl NativeCompositionError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}
impl fmt::Display for NativeCompositionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(&self.0)
    }
}
impl std::error::Error for NativeCompositionError {}

pub(crate) struct ProductionEngineParts {
    pub engine: EngineHandle,
    pub actor: ClientEngineActor,
    pub history: SqlCipherMessageStore,
    pub security: SqlCipherSecurityProjection,
    pub settings: SqlCipherSettingsStore,
}

pub(crate) const DATABASE_KEY_HANDLE: torca_identity::KeyId =
    torca_identity::KeyId::from_u128(0x746f7263615f64625f6b6579);

/// The only production engine composition. Platform adapters provide paths and
/// protected-secret implementations; storage, identity and domain actors are
/// constructed exactly once here for both supported targets.
fn spawn_production_engine_for<P: PlatformServices>(
    platform: &P,
) -> Result<ProductionEngineParts, NativeCompositionError> {
    let paths = platform.app_paths();
    std::fs::create_dir_all(&paths.data)
        .map_err(|error| io_error("create application data directory", &error))?;
    let database_path = paths.data.join("torca.db");
    let mut database_secret_store = platform.open_secret_store(SecretNamespace::Storage);
    let database_key = load_or_create_database_key(
        database_secret_store.as_mut(),
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
    let settings = SqlCipherSettingsStore::open(&database_path, &database_key)
        .map_err(|error| storage_error("open runtime settings store", &error))?;
    let pairings =
        SqlCipherPairingRepository::open(&database_path, &database_key).map_err(|error| {
            NativeCompositionError::new(format!("open pairing session store failed: {error}"))
        })?;

    let identity_keys = ManagedIdentityKeys::new(
        RustCryptoProvider,
        platform.open_secret_store(SecretNamespace::Identity),
    );
    let engine = ClientEngine::new(
        identity_repository,
        identity_keys,
        pairings,
        relationships,
        messages,
        receipts,
    );
    let (engine, actor) = ClientEngineActor::spawn(engine);
    Ok(ProductionEngineParts { engine, actor, history, security, settings })
}

pub(crate) fn spawn_production_engine() -> Result<ProductionEngineParts, NativeCompositionError> {
    #[cfg(windows)]
    {
        use crate::app_paths::windows_app_root;
        use torca_platform_windows::WindowsPlatformServices;

        let root = windows_app_root()?;
        let platform = WindowsPlatformServices::new(
            root.join("data"),
            root.join("cache"),
            root.join("logs"),
            RelayEndpoint { host: String::new(), port: 0 },
        );
        return spawn_production_engine_for(&platform);
    }

    #[cfg(target_os = "android")]
    {
        use crate::composition::android::{database_path, log_root_path};
        use torca_platform_android::AndroidPlatformServices;

        let database = database_path().map_err(|error| {
            NativeCompositionError::new(format!("resolve Android database path failed: {error}"))
        })?;
        let data = database.parent().map_or_else(|| database.clone(), std::path::Path::to_path_buf);
        let logs = log_root_path().unwrap_or_else(|_| data.join("logs"));
        let platform = AndroidPlatformServices::new(
            data.clone(),
            data.join("cache"),
            logs,
            RelayEndpoint { host: String::new(), port: 0 },
        )
        .with_secret_store_factory(|namespace| {
            let name = match namespace {
                torca_platform::SecretNamespace::Identity => "identity",
                torca_platform::SecretNamespace::Storage => "database",
                torca_platform::SecretNamespace::Runtime => "peer",
            };
            Box::new(crate::composition::android::AndroidProtectedSecretStore::new(name))
        });
        return spawn_production_engine_for(&platform);
    }

    #[cfg(not(any(windows, target_os = "android")))]
    {
        Err(NativeCompositionError::new(
            "production native composition is not implemented for this platform",
        ))
    }
}

pub(crate) fn load_or_create_database_key<C: torca_crypto::CryptoProvider>(
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

fn io_error(operation: &str, error: &std::io::Error) -> NativeCompositionError {
    NativeCompositionError::new(format!("{operation} failed ({:?})", error.kind()))
}

fn secret_error(
    operation: &str,
    error: &torca_crypto::ProtectedSecretStoreError,
) -> NativeCompositionError {
    NativeCompositionError::new(format!("{operation} failed: {error}"))
}

fn storage_error(operation: &str, error: &impl fmt::Display) -> NativeCompositionError {
    NativeCompositionError::new(format!("{operation} failed: {error}"))
}
