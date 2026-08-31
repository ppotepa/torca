use std::collections::{HashMap, HashSet};
use std::sync::{
    Arc,
    atomic::AtomicBool,
    mpsc::{self, Receiver, TryRecvError},
};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use std::{
    any::Any,
    panic::{AssertUnwindSafe, catch_unwind},
};

use serde_json::{Value, json};
use torca_client_application::{
    ApplicationError, ApplicationReadModels, ClientApplicationRuntime, ContactSecurityState,
};
use torca_client_engine::ClientEngineActor;
use torca_contract::{
    BridgeMessagePage, bridge_message_from_domain, bridge_result_from_application,
    bridge_snapshot_from_application, decode_application_command,
};
use torca_conversations::ConversationId;
use torca_foundation::{ClassifiedError, ErrorDescriptor, OpaqueId, Timestamp};
use torca_logging::{Level, Logger, default_root};
use torca_messaging::MessageDirection;
use torca_messaging::MessageId;
use torca_radio_coordinator::SharedRadioCoordinator;
use torca_runtime::{CommunicationState, RuntimeHandle, RuntimeOwner};
use torca_runtime_policy::RuntimeEventHub;
use torca_runtime_policy::{BatteryPreferences, EffectiveBatteryPolicy, SystemEnergyState};
use torca_transport_api::{CommissioningEvent, CommissioningObserver, CommissioningStage};

use crate::battery_policy::BatteryPolicyState;
use crate::composition::{NativeCompositionError, spawn_production_engine};
use crate::json::{
    bridge_message_page_json, bridge_result_json, bridge_snapshot_value, empty_snapshot_json,
    error_result, success_result,
};
use crate::runtime_composition::spawn_production_runtime;
use torca_communication_adapters::ReadReceiptPolicy;
use torca_storage_sqlite::SqlCipherNotificationStore;

pub(crate) const ABI_OK: i32 = 0;
pub(crate) const ABI_ERROR: i32 = -1;
pub(crate) const ABI_CLOSED: i32 = -2;
const NETWORK_RETRY_DELAY: Duration = Duration::from_secs(5);
// A provider startup attempt must produce a terminal result quickly enough for
// the host UI to offer retry.  The provider itself owns tighter operation
// timeouts (for example Iroh endpoint bind), while this is the composition
// level safety net for platform-specific hangs.
const NETWORK_START_OBSERVE_TIMEOUT: Duration = Duration::from_secs(30);
// Startup progress is event-driven, but a full bounded mailbox can reject an
// InternalWake. A short poll exists only while the provider worker is active,
// preventing a lost wake from turning a sub-second composition into a wait
// for the 30-second safety deadline.
const NETWORK_START_POLL_INTERVAL: Duration = Duration::from_millis(100);
const NETWORK_MAX_ATTEMPTS: u32 = 3;
const INCOMING_REACHABILITY_PROGRESS_STALL_AFTER: Duration = Duration::from_secs(120);

type HostStartResult =
    Result<(RuntimeHandle, RuntimeOwner, SharedRadioCoordinator), NativeCompositionError>;

enum HostStartEvent {
    Progress(CommissioningEvent),
    Finished(HostStartResult),
}

fn open_startup_logger() -> Option<Logger> {
    #[cfg(target_os = "android")]
    let log_root = crate::composition::android::log_root_path().unwrap_or_else(|_| default_root());
    #[cfg(not(target_os = "android"))]
    let log_root = default_root();
    match Logger::new(
        log_root,
        std::env::var("TORCA_DEVICE_ID").unwrap_or_else(|_| "native".into()),
        crate::torca_runtime::compiled_build_id(),
    ) {
        Ok(logger) => {
            eprintln!("Torca native diagnostics: {}", logger.directory().display());
            Some(logger)
        }
        Err(error) => {
            eprintln!("Torca native logger startup failed: {error}");
            None
        }
    }
}

pub struct TorcaRuntime {
    application_runtime: ClientApplicationRuntime,
    event_hub: Arc<RuntimeEventHub>,
    actor: Option<ClientEngineActor>,
    host: Option<RuntimeOwner>,
    host_start: Option<Receiver<HostStartEvent>>,
    host_start_started_at: Option<Instant>,
    host_start_started_at_ms: Option<i64>,
    host_last_progress_at_ms: Option<i64>,
    host_progress: u8,
    host_attempt: u32,
    host_status_code: Option<String>,
    host_status_summary: Option<String>,
    host_incoming_started_at_ms: Option<i64>,
    host_incoming_last_progress_at_ms: Option<i64>,
    host_incoming_progress: u8,
    host_incoming_attempt: u32,
    host_incoming_status_code: Option<String>,
    host_incoming_status_summary: Option<String>,
    host_incoming_retry_at: Option<Instant>,
    host_start_deadline: Option<Instant>,
    host_retry_at: Option<Instant>,
    host_failures: u32,
    host_state_hint: CommunicationState,
    network_changed_pending: bool,
    last_incoming_log_state: Option<(String, Option<String>)>,
    last_rendezvous_log_state: Option<(String, Option<String>)>,
    last_peer_log_state: HashMap<String, (String, u32)>,
    last_message_log_state: HashMap<String, (String, u32)>,
    last_attachment_log_state: HashMap<String, (String, u64, u32)>,
    last_radio_log_state: HashMap<String, String>,
    network_ready_logged: bool,
    pub(crate) last_result_json: String,
    pub(crate) last_error_descriptor: Option<ErrorDescriptor>,
    pub(crate) snapshot_json: String,
    /// Parsed projection retained alongside the ABI string. Native request
    /// routing frequently needs to embed the current snapshot in another
    /// response; reparsing the same JSON for every poll doubled CPU and
    /// allocation work on larger contact books.
    pub(crate) snapshot_value: serde_json::Value,
    pub(crate) query_json: String,
    logger: Option<Logger>,
    notification_seen: HashMap<String, u32>,
    contact_notification_seen: HashSet<String>,
    pairing_notification_seen: HashSet<String>,
    pub(crate) notification_cursor: u64,
    pub(crate) notification_last_scan_revision: u64,
    pub(crate) notification_store: SqlCipherNotificationStore,
    notification_events: Vec<torca_contract::NotificationEvent>,
    notifications_enabled: bool,
    read_receipts_enabled: bool,
    battery_policy: BatteryPolicyState,
    read_receipt_policy: ReadReceiptPolicy,
    /// Actor mailbox used only to wake native startup progress. Keeping this
    /// as an event path removes the former 100ms startup polling loop.
    pub(crate) actor_waker: Option<Arc<dyn Fn() + Send + Sync>>,
    /// Coalesces progress callbacks while one internal wake is already queued.
    pub(crate) actor_wake_pending: Option<Arc<AtomicBool>>,
}

include!("core_methods.rs");
include!("projection_methods.rs");
include!("operation_methods.rs");
include!("projection_helpers.rs");
include!("startup_methods.rs");

include!("support.rs");
