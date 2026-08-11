use core::fmt;

use torca_client_application::{
    ApplicationQueryError, ApplicationReadModels, ClientApplicationHandle, ContactSecuritySnapshot,
    ContactSecurityState, ConversationHistoryPort, ConversationMessagePage,
    ConversationMessageSummary, PendingOperationStore, RuntimeSettingsPort, SecurityProjectionPort,
};
use torca_client_engine::{ClientEngine, ClientEngineActor};
use torca_conversations::ConversationId;
use torca_crypto::{ManagedIdentityKeys, ProtectedSecretStore, RustCryptoProvider};
use torca_foundation::Timestamp;
use torca_messaging::{Message, MessageId};
use torca_platform::{PlatformServices, SecretNamespace};
use torca_storage_sqlite::{
    SqlCipherMessageStore, SqlCipherPairingRepository, SqlCipherPendingOperationStore,
    SqlCipherReceiptStore, SqlCipherSecurityProjection, SqlCipherSettingsStore, SqlCipherStore,
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
    pub application: ClientApplicationHandle,
    pub actor: ClientEngineActor,
    pub read_models: ApplicationReadModels,
    pub pending: Box<dyn PendingOperationStore>,
}

struct SqliteHistory(SqlCipherMessageStore);
impl ConversationHistoryPort for SqliteHistory {
    fn page_for_conversation(
        &self,
        id: ConversationId,
        before: Option<(Timestamp, MessageId)>,
        limit: usize,
    ) -> Result<ConversationMessagePage, ApplicationQueryError> {
        self.0
            .page_for_conversation(id, before, limit)
            .map(|page| ConversationMessagePage {
                messages: page.messages,
                has_more: page.has_more,
            })
            .map_err(|_| ApplicationQueryError::Unavailable)
    }
    fn search_conversation(
        &self,
        id: ConversationId,
        query: &str,
        limit: usize,
    ) -> Result<Vec<Message>, ApplicationQueryError> {
        self.0.search_conversation(id, query, limit).map_err(|_| ApplicationQueryError::Unavailable)
    }
    fn conversation_summaries(
        &self,
    ) -> Result<
        std::collections::BTreeMap<ConversationId, ConversationMessageSummary>,
        ApplicationQueryError,
    > {
        self.0
            .conversation_summaries()
            .map(|items| {
                items
                    .into_iter()
                    .map(|(id, item)| {
                        (
                            id,
                            ConversationMessageSummary {
                                conversation_id: item.conversation_id,
                                unread_count: item.unread_count,
                                last_activity_at: item.last_activity_at,
                                last_message: item.last_message,
                            },
                        )
                    })
                    .collect()
            })
            .map_err(|_| ApplicationQueryError::Unavailable)
    }
}

struct SqliteSecurity(SqlCipherSecurityProjection);
impl SecurityProjectionPort for SqliteSecurity {
    fn requires_reverification(&self, id: ConversationId) -> Result<bool, ApplicationQueryError> {
        self.0.requires_reverification(id).map_err(|_| ApplicationQueryError::Unavailable)
    }
    fn contact_states(
        &self,
    ) -> Result<
        std::collections::BTreeMap<torca_contacts::ContactId, ContactSecuritySnapshot>,
        ApplicationQueryError,
    > {
        self.0
            .contact_states()
            .map(|states| {
                states
                    .into_iter()
                    .map(|(id, snapshot)| {
                        let state = match snapshot.state {
                            torca_storage_sqlite::ContactSecurityState::Unverified => {
                                ContactSecurityState::Unverified
                            }
                            torca_storage_sqlite::ContactSecurityState::Verified => {
                                ContactSecurityState::Verified
                            }
                            torca_storage_sqlite::ContactSecurityState::IdentityChanged => {
                                ContactSecurityState::IdentityChanged
                            }
                        };
                        (id, ContactSecuritySnapshot { state, verified_at: snapshot.verified_at })
                    })
                    .collect()
            })
            .map_err(|_| ApplicationQueryError::Unavailable)
    }
}

struct SqliteSettings(SqlCipherSettingsStore);
impl RuntimeSettingsPort for SqliteSettings {
    fn notifications_enabled(&self) -> Result<bool, ApplicationQueryError> {
        self.0.notifications_enabled().map_err(|_| ApplicationQueryError::Unavailable)
    }
    fn set_notifications_enabled(
        &self,
        enabled: bool,
        at: i64,
    ) -> Result<(), ApplicationQueryError> {
        self.0
            .set_notifications_enabled(enabled, at)
            .map_err(|_| ApplicationQueryError::Unavailable)
    }
    fn read_receipts_enabled(&self) -> Result<bool, ApplicationQueryError> {
        self.0.read_receipts_enabled().map_err(|_| ApplicationQueryError::Unavailable)
    }
    fn set_read_receipts_enabled(
        &self,
        enabled: bool,
        at: i64,
    ) -> Result<(), ApplicationQueryError> {
        self.0
            .set_read_receipts_enabled(enabled, at)
            .map_err(|_| ApplicationQueryError::Unavailable)
    }
    fn new_contacts_acknowledged_at_ms(&self) -> Result<Option<i64>, ApplicationQueryError> {
        self.0.new_contacts_acknowledged_at_ms().map_err(|_| ApplicationQueryError::Unavailable)
    }
    fn acknowledge_new_contacts(&self, at: i64) -> Result<(), ApplicationQueryError> {
        self.0.acknowledge_new_contacts(at).map_err(|_| ApplicationQueryError::Unavailable)
    }
}

pub(crate) const DATABASE_KEY_HANDLE: torca_identity::KeyId =
    torca_identity::KeyId::from_u128(0x746f7263615f64625f6b6579);

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
        read_models: ApplicationReadModels {
            history: Box::new(SqliteHistory(history)),
            security: Box::new(SqliteSecurity(security)),
            settings: Box::new(SqliteSettings(settings)),
        },
    })
}

pub(crate) fn spawn_production_engine() -> Result<ProductionEngineParts, NativeCompositionError> {
    let platform = crate::platform_selector::platform_services()?;
    spawn_production_engine_for(platform.as_ref())
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
