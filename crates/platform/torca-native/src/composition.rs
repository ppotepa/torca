use core::fmt;

use torca_client_application::{
    ApplicationReadModels, ClientApplicationHandle, PendingOperationStore,
};
use torca_client_engine::{ClientEngine, ClientEngineActor};
use torca_crypto::{ManagedIdentityKeys, RustCryptoProvider};
use torca_platform::{PlatformServices, SecretNamespace};
use torca_storage_sqlite::{
    SqlCipherMessageStore, SqlCipherPairingRepository, SqlCipherPendingOperationStore,
    SqlCipherReceiptStore, SqlCipherSecurityProjection, SqlCipherSettingsStore, SqlCipherStore,
};

pub(crate) use crate::database_key::{DATABASE_KEY_HANDLE, load_or_create_database_key};
use crate::read_models::build_read_models;

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
    pub application: ClientApplicationHandle,
    pub actor: ClientEngineActor,
    pub read_models: ApplicationReadModels,
    pub pending: Box<dyn PendingOperationStore>,
}

/// The only production engine composition. Platform adapters provide paths and
/// protected-secret implementations; storage, identity and domain actors are
/// constructed exactly once here for both supported targets.
fn spawn_production_engine_for(
    platform: &dyn PlatformServices,
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
    let pending = SqlCipherPendingOperationStore::open(&database_path, &database_key)
        .map_err(|error| storage_error("open pending operation store", &error))?;
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
    let application = ClientApplicationHandle::new(engine.clone());
    Ok(ProductionEngineParts {
        application,
        actor,
        pending: Box::new(pending),
        read_models: build_read_models(history, security, settings),
    })
}

pub(crate) fn spawn_production_engine() -> Result<ProductionEngineParts, NativeCompositionError> {
    let platform = crate::platform_selector::platform_services()?;
    spawn_production_engine_for(platform.as_ref())
}

fn io_error(operation: &str, error: &std::io::Error) -> NativeCompositionError {
    NativeCompositionError::new(format!("{operation} failed ({:?})", error.kind()))
}

fn storage_error(operation: &str, error: &impl fmt::Display) -> NativeCompositionError {
    NativeCompositionError::new(format!("{operation} failed: {error}"))
}
