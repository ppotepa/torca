use std::sync::Arc;
#[cfg(feature = "provider-webrtc")]
use std::sync::{OnceLock, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use torca_client_engine::EngineHandle;
use torca_communication_adapters::{
    ProductionCommunicationInputs, ProductionCommunicationOutput, ReadReceiptPolicy,
    build_production_communication,
};
use torca_connectivity::ConnectivityObserver;
use torca_crypto::{
    ManagedIdentityKeys, ManagedPeerSecrets, OwnedHandshakeSigner, RustCryptoProvider,
};
use torca_pairing_driver::PairingWorkerDriver;
use torca_platform::{PlatformServices, SecretNamespace};
use torca_radio_coordinator::SharedRadioCoordinator;
use torca_runtime::{RuntimeHandle, RuntimeOwner};
use torca_transport_api::CommissioningObserver;
#[cfg(feature = "provider-webrtc")]
use torca_transport_api::WebRtcSessionProvider;
#[cfg(feature = "provider-webrtc")]
use torca_transport_api::WebRtcSignalingProvider;
use torca_transport_api::{CommissioningEvent, CommissioningStage};
#[cfg(feature = "provider-webrtc")]
use torca_transport_webrtc::WebRtcHostBridge;

use crate::composition::{NativeCompositionError, load_or_create_database_key};
use crate::provider_composition::{
    ProviderCompositionInputs, ProviderPairingInputs, compose_selected_provider,
};

// The host may recreate its WebRTC bridge after a runtime teardown (for
// example after an Android activity/process restart). A one-shot OnceLock
// would retain a dead bridge forever, so the lock protects a replaceable slot.
#[cfg(feature = "provider-webrtc")]
static WEBRTC_PROVIDER: OnceLock<RwLock<Option<Arc<dyn WebRtcSessionProvider>>>> = OnceLock::new();
#[cfg(feature = "provider-webrtc")]
static WEBRTC_SIGNALING_PROVIDER: OnceLock<RwLock<Option<Arc<dyn WebRtcSignalingProvider>>>> =
    OnceLock::new();

#[cfg(feature = "provider-webrtc")]
fn webrtc_provider_slot() -> &'static RwLock<Option<Arc<dyn WebRtcSessionProvider>>> {
    WEBRTC_PROVIDER.get_or_init(|| RwLock::new(None))
}

#[cfg(feature = "provider-webrtc")]
fn webrtc_signaling_provider_slot() -> &'static RwLock<Option<Arc<dyn WebRtcSignalingProvider>>> {
    WEBRTC_SIGNALING_PROVIDER.get_or_init(|| RwLock::new(None))
}

#[cfg(feature = "provider-webrtc")]
pub(crate) fn clear_registered_webrtc_providers() {
    if let Ok(mut slot) = webrtc_provider_slot().write() {
        *slot = None;
    }
    if let Ok(mut slot) = webrtc_signaling_provider_slot().write() {
        *slot = None;
    }
}

#[cfg(feature = "provider-webrtc")]
/// Registers the platform-owned WebRTC signalling/DataChannel bridge.
///
/// Registration is intentionally explicit and scoped to one runtime
/// generation: a deployment has one selected transport provider, and WebRTC
/// must never silently fall back to a different implementation. Android and
/// desktop hosts should call this before starting native composition when
/// `TORCA_COMMUNICATION_PROVIDER=webrtc`. A later generation may replace a
/// bridge after the previous runtime has been torn down.
#[cfg(feature = "provider-webrtc")]
pub fn register_webrtc_session_provider(
    provider: Arc<dyn WebRtcSessionProvider>,
) -> Result<(), Arc<dyn WebRtcSessionProvider>> {
    let replacement = provider.clone();
    webrtc_provider_slot()
        .write()
        .map(|mut slot| {
            *slot = Some(provider);
        })
        .map_err(|_| replacement)
}

#[cfg(feature = "provider-webrtc")]
pub fn register_webrtc_signaling_provider(
    provider: Arc<dyn WebRtcSignalingProvider>,
) -> Result<(), Arc<dyn WebRtcSignalingProvider>> {
    let replacement = provider.clone();
    webrtc_signaling_provider_slot()
        .write()
        .map(|mut slot| {
            *slot = Some(provider);
        })
        .map_err(|_| replacement)
}

#[cfg(feature = "provider-webrtc")]
/// Registers one host bridge for both negotiated sessions and pairing
/// signaling. This is the preferred platform entry point when the SDK uses a
/// single owner for its WebRTC lifecycle.
#[cfg(feature = "provider-webrtc")]
pub fn register_webrtc_host_bridge(
    bridge: Arc<WebRtcHostBridge>,
) -> Result<(), Arc<WebRtcHostBridge>> {
    let session = Arc::clone(&bridge);
    let signaling = Arc::clone(&bridge);
    register_webrtc_session_provider(session).map_err(|_| Arc::clone(&bridge))?;
    register_webrtc_signaling_provider(signaling).map_err(|_| bridge)
}

#[cfg(feature = "provider-webrtc")]
pub(crate) fn registered_webrtc_session_provider()
-> Result<Arc<dyn WebRtcSessionProvider>, NativeCompositionError> {
    webrtc_provider_slot()
        .read()
        .ok()
        .and_then(|slot| slot.clone())
        .ok_or_else(|| NativeCompositionError::new("WebRTC session provider is not registered"))
}

#[cfg(feature = "provider-webrtc")]
pub(crate) fn registered_webrtc_signaling_provider()
-> Result<Arc<dyn WebRtcSignalingProvider>, NativeCompositionError> {
    webrtc_signaling_provider_slot()
        .read()
        .ok()
        .and_then(|slot| slot.clone())
        .ok_or_else(|| NativeCompositionError::new("WebRTC signaling provider is not registered"))
}

pub(crate) fn spawn_production_runtime(
    engine: EngineHandle,
    bootstrap_observer: CommissioningObserver,
    read_receipt_policy: ReadReceiptPolicy,
) -> Result<(RuntimeHandle, RuntimeOwner, SharedRadioCoordinator), NativeCompositionError> {
    let platform = crate::platform_selector::platform_services()?;
    spawn_runtime_for(platform.as_ref(), engine, bootstrap_observer, read_receipt_policy)
}

fn spawn_runtime_for(
    platform: &dyn PlatformServices,
    engine: EngineHandle,
    bootstrap_observer: CommissioningObserver,
    read_receipt_policy: ReadReceiptPolicy,
) -> Result<(RuntimeHandle, RuntimeOwner, SharedRadioCoordinator), NativeCompositionError> {
    let report_observer = Arc::clone(&bootstrap_observer);
    let report = move |progress: u8, code: &str, summary: &str| {
        report_observer(CommissioningEvent {
            stage: CommissioningStage::LocalRuntime,
            progress,
            attempt: 1,
            retry_after_ms: None,
            code: code.to_owned(),
            summary: summary.to_owned(),
        });
    };
    // A failed/retried startup must not inherit a bridge owned by the previous
    // runtime generation when the host no longer provides one.
    #[cfg(feature = "provider-webrtc")]
    {
        clear_registered_webrtc_providers();
        if let Some(provider) = platform.webrtc_session_provider() {
            let _ = register_webrtc_session_provider(provider);
        }
        if let Some(provider) = platform.webrtc_signaling_provider() {
            let _ = register_webrtc_signaling_provider(provider);
        }
    }
    let configured_provider = crate::transport_config::compiled_provider().map_err(|error| {
        NativeCompositionError::new(format!("invalid communication provider: {error:?}"))
    })?;
    // Provider adapters may compile while their complete commissioning,
    // rendezvous and platform lifecycle are still under construction. Do not
    // silently start Tor in that case: the artifact must fail before it
    // creates an identity or network side effect under the wrong provider
    // assumption. `torca-deploy` keeps these providers hidden until this gate
    // is removed alongside their real composition.
    crate::transport_config::ensure_deployment_ready(configured_provider)
        .map_err(NativeCompositionError::new)?;
    let paths = platform.app_paths();
    let database_path = paths.data.join("torca.db");
    let mut database_store = platform.open_secret_store(SecretNamespace::Storage);
    let database_key = load_or_create_database_key(
        database_store.as_mut(),
        crate::composition::DATABASE_KEY_HANDLE,
        RustCryptoProvider,
    )?;
    let identity = engine_identity(&engine)?;
    let key_id = identity.public().key().key_id();
    let identity_id = identity.public().identity_id().to_opaque();

    let rendezvous_endpoint =
        crate::provider_composition::compiled_rendezvous_endpoint(configured_provider)?;
    let provider = compose_selected_provider(
        configured_provider,
        ProviderCompositionInputs {
            data_dir: paths.data.clone(),
            provider_secret_store: platform.open_secret_store(SecretNamespace::Runtime),
            rendezvous_endpoint,
            startup_timeout: configured_provider.deployment_profile().startup_timeout,
            now: current_timestamp()?,
            bootstrap_observer,
        },
    )?;
    report(35, "PROVIDER_COMPOSED", "Selected communication provider composed");
    let crate::provider_composition::ProviderComponents {
        provider: composed_provider,
        lifecycle: communication_lifecycle,
        peer_transport_factory: transport_factory,
        routing: provider_routing,
        pairing_factory,
        rendezvous_probe,
        radio_media_factory,
    } = provider;
    debug_assert_eq!(composed_provider, configured_provider);
    let signer = OwnedHandshakeSigner::new(
        ManagedIdentityKeys::new(
            RustCryptoProvider,
            platform.open_secret_store(SecretNamespace::Identity),
        ),
        key_id,
    );
    let connectivity = ConnectivityObserver::default();
    let ProductionCommunicationOutput { driver: communication, radio } =
        build_production_communication(
            engine.clone(),
            &database_path,
            &database_key,
            &paths.cache.join("attachments"),
            &paths.data.join("attachments").join("staging"),
            ProductionCommunicationInputs {
                communication_provider: configured_provider,
                signer,
                peer_secret_store: platform.open_secret_store(SecretNamespace::Runtime),
                attachment_secret_store: platform.open_secret_store(SecretNamespace::Runtime),
                export_secret_store: platform.open_secret_store(SecretNamespace::Runtime),
                relationship_secret_store: platform.open_secret_store(SecretNamespace::Runtime),
                transport_factory,
                provider_routing,
                radio_media_factory,
                local_identity_id: identity_id,
                connectivity: connectivity.clone(),
                read_receipt_policy,
            },
        )
        .map_err(|_| NativeCompositionError::new("compose communication runtime failed"))?;
    report(60, "COMMUNICATION_COMPOSED", "Communication and attachment workers composed");

    let pairing = pairing_factory.build(ProviderPairingInputs {
        engine: engine.clone(),
        approval: Box::new(ManagedIdentityKeys::new(
            RustCryptoProvider,
            platform.open_secret_store(SecretNamespace::Identity),
        )),
        peer_secrets: Box::new(ManagedPeerSecrets::new(
            RustCryptoProvider,
            platform.open_secret_store(SecretNamespace::Runtime),
        )),
        connectivity: connectivity.clone(),
    })?;
    report(80, "PAIRING_COMPOSED", "Pairing driver composed");
    let pairing = PairingWorkerDriver::spawn(pairing)
        .map_err(|_| NativeCompositionError::new("spawn pairing supervisor failed"))?;
    report(90, "PAIRING_STARTED", "Pairing supervisor started");
    let (handle, owner) = RuntimeOwner::spawn_with_connectivity(
        engine,
        pairing,
        communication,
        communication_lifecycle,
        rendezvous_probe,
        connectivity,
    );
    report(100, "RUNTIME_COMPOSED", "Selected communication runtime is ready");
    Ok((handle, owner, radio))
}

fn engine_identity(
    engine: &EngineHandle,
) -> Result<torca_identity::Identity, NativeCompositionError> {
    engine
        .overview_snapshot()
        .map_err(|_| NativeCompositionError::new("load local identity failed"))?
        .identity
        .ok_or_else(|| NativeCompositionError::new("local identity is not initialized"))
}

fn current_timestamp() -> Result<torca_foundation::Timestamp, NativeCompositionError> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| NativeCompositionError::new("system clock is before Unix epoch"))?;
    let millis = i64::try_from(duration.as_millis())
        .map_err(|_| NativeCompositionError::new("system timestamp is out of range"))?;
    torca_foundation::Timestamp::from_unix_millis(millis)
        .map_err(|_| NativeCompositionError::new("system timestamp is invalid"))
}
