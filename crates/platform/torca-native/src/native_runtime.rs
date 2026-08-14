use std::collections::{HashMap, HashSet};
use std::sync::{
    Arc,
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
use torca_foundation::{OpaqueId, Timestamp};
use torca_logging::{Level, Logger, default_root};
use torca_messaging::MessageDirection;
use torca_messaging::MessageId;
use torca_radio_coordinator::SharedRadioCoordinator;
use torca_runtime::{RuntimeHandle, RuntimeOwner, TorState};
use torca_runtime_policy::RuntimeEventHub;
use torca_tor::{TorBootstrapEvent, TorBootstrapObserver, TorBootstrapStage};

use crate::composition::{NativeCompositionError, spawn_production_engine};
use crate::json::{
    bridge_message_page_json, bridge_result_json, bridge_snapshot_json, empty_snapshot_json,
    error_result, success_result,
};
use crate::runtime_composition::spawn_production_runtime;
use torca_communication_adapters::ReadReceiptPolicy;

pub(crate) const ABI_OK: i32 = 0;
pub(crate) const ABI_ERROR: i32 = -1;
pub(crate) const ABI_CLOSED: i32 = -2;
const NETWORK_RETRY_DELAY: Duration = Duration::from_secs(5);
const NETWORK_START_OBSERVE_TIMEOUT: Duration = Duration::from_secs(120);
const NETWORK_MAX_ATTEMPTS: u32 = 3;
const ONION_PROGRESS_STALL_AFTER: Duration = Duration::from_secs(120);

type HostStartResult =
    Result<(RuntimeHandle, RuntimeOwner, SharedRadioCoordinator), NativeCompositionError>;

enum HostStartEvent {
    Progress(TorBootstrapEvent),
    Finished(HostStartResult),
}

/// Open diagnostics before production composition starts.  Tor/SQLCipher
/// bootstrap can take long enough to fail or be interrupted; initializing the
/// logger only after composition made exactly those failures invisible in
/// packaged Android runs.
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
    host_onion_started_at_ms: Option<i64>,
    host_onion_last_progress_at_ms: Option<i64>,
    host_onion_progress: u8,
    host_onion_attempt: u32,
    host_onion_status_code: Option<String>,
    host_onion_status_summary: Option<String>,
    host_onion_retry_at: Option<Instant>,
    host_start_deadline: Option<Instant>,
    host_retry_at: Option<Instant>,
    host_failures: u32,
    host_state_hint: TorState,
    network_changed_pending: bool,
    last_onion_log_state: Option<(String, Option<String>)>,
    last_relay_log_state: Option<(String, Option<String>)>,
    last_peer_log_state: HashMap<String, (String, u32)>,
    last_attachment_log_state: HashMap<String, (String, u64, u32)>,
    last_radio_log_state: HashMap<String, String>,
    network_ready_logged: bool,
    pub(crate) last_result_json: String,
    pub(crate) snapshot_json: String,
    pub(crate) query_json: String,
    logger: Option<Logger>,
    notification_seen: HashMap<String, u32>,
    /// Contacts present when this process attached are not new notifications.
    /// Newly completed pairings are emitted exactly once during this runtime run.
    contact_notification_seen: HashSet<String>,
    pairing_notification_seen: HashSet<String>,
    pub(crate) notification_cursor: u64,
    notification_events: Vec<torca_contract::NotificationEvent>,
    notifications_enabled: bool,
    read_receipts_enabled: bool,
    read_receipt_policy: ReadReceiptPolicy,
}

impl TorcaRuntime {
    fn read_models(&self) -> &ApplicationReadModels {
        self.application_runtime
            .read_models()
            .expect("production composition always installs application read-model ports")
    }

    pub(crate) fn new(event_hub: Arc<RuntimeEventHub>) -> Result<Self, String> {
        let logger = open_startup_logger();
        let parts = match spawn_production_engine() {
            Ok(parts) => parts,
            Err(error) => {
                if let Some(logger) = logger.as_ref() {
                    let _ = logger.event(
                        "bootstrap",
                        Level::Error,
                        "composition",
                        "COMPOSITION_FAILED",
                        &format!("Native engine composition failed: {error}"),
                    );
                }
                return Err(format!("native engine composition failed: {error}"));
            }
        };
        let application = parts.application.clone();
        let mut application_runtime = ClientApplicationRuntime::new(application.clone());
        application_runtime.attach_read_models(parts.read_models);
        application_runtime.attach_pending_store(parts.pending);
        let notifications_enabled = application_runtime
            .read_models()
            .and_then(|models| models.settings.notifications_enabled().ok())
            .unwrap_or(true);
        let read_receipts_enabled = application_runtime
            .read_models()
            .and_then(|models| models.settings.read_receipts_enabled().ok())
            .unwrap_or(true);
        let read_receipt_policy = ReadReceiptPolicy::new(read_receipts_enabled);
        let contact_notification_seen = application_runtime
            .snapshot_context()
            .map(bridge_snapshot_from_application)
            .map(|snapshot| snapshot.contacts.into_iter().map(|contact| contact.id).collect())
            .unwrap_or_default();
        let mut runtime = Self {
            application_runtime,
            event_hub,
            actor: Some(parts.actor),
            host: None,
            host_start: None,
            host_start_started_at: None,
            host_start_started_at_ms: None,
            host_last_progress_at_ms: None,
            host_progress: 0,
            host_attempt: 0,
            host_status_code: None,
            host_status_summary: None,
            host_onion_started_at_ms: None,
            host_onion_last_progress_at_ms: None,
            host_onion_progress: 0,
            host_onion_attempt: 0,
            host_onion_status_code: None,
            host_onion_status_summary: None,
            host_onion_retry_at: None,
            host_start_deadline: None,
            host_retry_at: None,
            host_failures: 0,
            host_state_hint: TorState::Stopped,
            network_changed_pending: false,
            last_onion_log_state: None,
            last_relay_log_state: None,
            last_peer_log_state: HashMap::new(),
            last_attachment_log_state: HashMap::new(),
            last_radio_log_state: HashMap::new(),
            network_ready_logged: false,
            last_result_json: success_result("initialized"),
            snapshot_json: empty_snapshot_json(),
            query_json: "{\"messages\":[],\"hasMore\":false}".into(),
            logger,
            notification_seen: HashMap::new(),
            contact_notification_seen,
            pairing_notification_seen: HashSet::new(),
            notification_cursor: 0,
            notification_events: Vec::new(),
            notifications_enabled,
            read_receipts_enabled,
            read_receipt_policy,
        };
        runtime.log(
            "runtime",
            Level::Info,
            "native",
            "RUNTIME_INITIALIZED",
            "Native runtime initialized",
        );
        if !runtime.has_identity().map_err(|_| "read local identity failed".to_owned())? {
            runtime.log(
                "bootstrap",
                Level::Info,
                "identity",
                "IDENTITY_CREATING",
                "No local identity found; creating device identity",
            );
            runtime
                .create_bootstrap_identity()
                .map_err(|_| "create bootstrap device identity failed".to_owned())?;
            runtime.log(
                "bootstrap",
                Level::Info,
                "identity",
                "IDENTITY_CREATED",
                "Device identity created",
            );
        }
        runtime.begin_runtime_start();
        if runtime.refresh_snapshot() != ABI_OK {
            runtime.log(
                "runtime",
                Level::Error,
                "native",
                "SNAPSHOT_UNAVAILABLE",
                "Initial native snapshot unavailable",
            );
            eprintln!("Torca native engine initialization failed: initial snapshot unavailable");
            return Err("initial native snapshot unavailable".to_owned());
        }
        runtime.log(
            "bootstrap",
            Level::Info,
            "runtime",
            "LOCAL_READY",
            "Local runtime and initial snapshot are ready",
        );
        Ok(runtime)
    }

    pub(crate) fn execute_with_request_id(
        &mut self,
        command: torca_contract::BridgeCommand,
        request_id: &str,
    ) -> i32 {
        if self.is_closed() {
            self.last_result_json = error_result("native engine is closed");
            return ABI_CLOSED;
        }
        self.advance_runtime_start();
        if let torca_contract::BridgeCommand::SetNotifications { enabled } = &command {
            self.notifications_enabled = *enabled;
            if let Ok(elapsed) = SystemTime::now().duration_since(UNIX_EPOCH) {
                let _ = self.read_models().settings.set_notifications_enabled(
                    *enabled,
                    i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX),
                );
            }
        }
        if let torca_contract::BridgeCommand::SetReadReceipts { enabled } = &command {
            let now = unix_time_ms().unwrap_or(0);
            if self.read_models().settings.set_read_receipts_enabled(*enabled, now).is_err() {
                self.last_result_json = error_result("read receipt setting storage unavailable");
                return ABI_ERROR;
            }
            self.read_receipts_enabled = *enabled;
            self.read_receipt_policy.set_enabled(*enabled);
        }
        if matches!(&command, torca_contract::BridgeCommand::AcknowledgeNewContacts) {
            let now = unix_time_ms().unwrap_or(0);
            if self.read_models().settings.acknowledge_new_contacts(now).is_err() {
                self.last_result_json = error_result("contact acknowledgement storage unavailable");
                return ABI_ERROR;
            }
        }
        let is_profile = matches!(&command, torca_contract::BridgeCommand::UpdateProfile { .. });
        let pairing_operation = match &command {
            torca_contract::BridgeCommand::CreatePairing { session_id_hex } => {
                Some(("pairing.create", session_id_hex.clone()))
            }
            torca_contract::BridgeCommand::JoinPairing { session_id_hex, .. } => {
                Some(("pairing.join", session_id_hex.clone()))
            }
            torca_contract::BridgeCommand::ApprovePairing { session_id_hex } => {
                Some(("pairing.approve", session_id_hex.clone()))
            }
            torca_contract::BridgeCommand::RejectPairing { session_id_hex } => {
                Some(("pairing.reject", session_id_hex.clone()))
            }
            torca_contract::BridgeCommand::CancelPairing { session_id_hex } => {
                Some(("pairing.cancel", session_id_hex.clone()))
            }
            _ => None,
        };
        let radio_operation = match &command {
            torca_contract::BridgeCommand::SetRadioEnabled { .. } => Some("radio.set_enabled"),
            torca_contract::BridgeCommand::BeginRadioTransmission { .. } => {
                Some("radio.begin_transmission")
            }
            torca_contract::BridgeCommand::EndRadioTransmission { .. } => {
                Some("radio.end_transmission")
            }
            _ => None,
        };
        if is_profile {
            self.log_profile(request_id, "PROFILE_REQUEST_RECEIVED");
            self.log_profile(request_id, "PROFILE_COMMAND_QUEUED");
            self.log_profile(request_id, "PROFILE_COMMAND_STARTED");
            self.log_profile(request_id, "PROFILE_STORAGE_STARTED");
        }
        if let Some((operation, session_id)) = &pairing_operation {
            self.log_pairing(request_id, operation, session_id, "PAIRING_REQUEST_STARTED", None);
        }
        let result = bridge_result_from_application(match decode_application_command(command) {
            Ok(command) => self.application_runtime.execute(command),
            Err(error) => Err(ApplicationError::invalid_input(error)),
        });
        if !result.ok {
            if is_profile {
                self.log_profile(request_id, "PROFILE_STORAGE_FAILED");
            }
            if let (Some(logger), Some(operation)) = (&self.logger, radio_operation) {
                let context = json!({
                    "operation": operation,
                    "errorCode": &result.error_code,
                    "error": &result.error,
                })
                .to_string();
                let _ = logger.event_with_context(
                    "radio",
                    Level::Error,
                    "command",
                    "BRIDGE_COMMAND_FAILED",
                    "Radio command rejected by native engine",
                    Some(&context),
                );
            } else {
                self.log(
                    "bridge",
                    Level::Error,
                    "command",
                    "BRIDGE_COMMAND_FAILED",
                    "Bridge command rejected by native engine",
                );
            }
            if let Some((operation, session_id)) = &pairing_operation {
                self.log_pairing(
                    request_id,
                    operation,
                    session_id,
                    "PAIRING_REQUEST_FAILED",
                    result.error_code.as_deref(),
                );
            }
            self.last_result_json = bridge_result_json(&result);
            return ABI_ERROR;
        }
        if is_profile {
            self.log_profile(request_id, "PROFILE_STORAGE_COMMITTED");
        }
        self.last_result_json = bridge_result_json(&result);
        let _ = self.refresh_snapshot();
        if is_profile {
            self.log_profile(request_id, "PROFILE_SNAPSHOT_PUBLISHED");
            self.log_profile(request_id, "PROFILE_REQUEST_SUCCEEDED");
        }
        if let Some((operation, session_id)) = &pairing_operation {
            self.log_pairing(
                request_id,
                operation,
                session_id,
                "PAIRING_REQUEST_SUCCEEDED",
                Some(result.kind.as_str()),
            );
        }
        ABI_OK
    }

    pub(crate) fn refresh_snapshot(&mut self) -> i32 {
        if self.is_closed() {
            return ABI_CLOSED;
        }
        self.advance_runtime_start();
        if let Err(error) = self.application_runtime.advance_bootstrap() {
            self.last_result_json = error_result(&error.to_string());
            return ABI_ERROR;
        }
        let mut snapshot =
            match self.application_runtime.snapshot_context().map(bridge_snapshot_from_application)
            {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    self.last_result_json = error_result(&error.to_string());
                    return ABI_ERROR;
                }
            };
        // Read-model projections are optional sections of the application snapshot.
        // A transient history/security query failure must not suppress a fresh transport
        // snapshot and leave the process-wide connection indicators showing stale data.
        let _ = self.apply_history_summaries(&mut snapshot);
        let _ = self.apply_security_states(&mut snapshot);
        self.apply_navigation_badges(&mut snapshot);
        // Keep transport indicators live after the application shell opens, but
        // never gate the shell on relay control-plane health. Pairing/P2P keeps
        // its narrower relay/onion prerequisites at operation level.
        if !self.application_runtime.has_runtime() {
            snapshot.tor_state = torca_contract::tor_state_name(self.host_state_hint).into();
        }
        // A freshly reset client already owns a device identity but has no
        // display name yet. Host state must still be projected so the shell
        // can leave warm-up and show profile setup.
        self.apply_host_state_hint(&mut snapshot);
        self.log_network_transitions(&snapshot);
        let snapshot_json = bridge_snapshot_json(&snapshot);
        self.snapshot_json = serde_json::from_str::<serde_json::Value>(&snapshot_json)
            .map(|mut value| {
                value["notificationsEnabled"] = serde_json::Value::Bool(self.notifications_enabled);
                value["readReceiptsEnabled"] = serde_json::Value::Bool(self.read_receipts_enabled);
                value.to_string()
            })
            .unwrap_or(snapshot_json);
        ABI_OK
    }

    fn log_network_transitions(&mut self, snapshot: &torca_contract::BridgeSnapshot) {
        let onion = snapshot
            .bootstrap_steps
            .iter()
            .find(|step| step.id == "onion_service")
            .map(|step| (step.state.clone(), step.code.clone()));
        let relay = snapshot
            .bootstrap_steps
            .iter()
            .find(|step| step.id == "secure_relay")
            .map(|step| (step.state.clone(), step.code.clone()));

        if let Some(current) = onion.as_ref()
            && self.last_onion_log_state.as_ref() != Some(current)
        {
            let (level, code, message) = network_transition_event("ONION", current);
            self.log("tor", level, "onion_service", &code, &message);
            self.last_onion_log_state = Some(current.clone());
        }
        if let Some(current) = relay.as_ref()
            && self.last_relay_log_state.as_ref() != Some(current)
        {
            let (level, code, message) = network_transition_event("RELAY", current);
            self.log("relay", level, "relay_connection", &code, &message);
            self.last_relay_log_state = Some(current.clone());
        }
        for contact in &snapshot.contacts {
            let current =
                (contact.peer_health.state.clone(), contact.peer_health.reconnect_attempt);
            if self.last_peer_log_state.get(&contact.id) != Some(&current) {
                let code = format!("PEER_{}", current.0.to_ascii_uppercase());
                self.log(
                    "messaging",
                    if current.0 == "ready" { Level::Info } else { Level::Warn },
                    "peer_connection",
                    &code,
                    &format!(
                        "contact={} state={} reconnect_attempt={}",
                        contact.id, current.0, current.1
                    ),
                );
                self.last_peer_log_state.insert(contact.id.clone(), current);
            }
        }
        for attachment in &snapshot.attachments {
            let current = (attachment.status.clone(), attachment.offset, attachment.attempt_count);
            if self.last_attachment_log_state.get(&attachment.id) != Some(&current) {
                self.log(
                    "messaging",
                    if attachment.status == "failed" { Level::Error } else { Level::Info },
                    "attachment_transfer",
                    "ATTACHMENT_STATE_CHANGED",
                    &format!(
                        "attachment={} direction={} status={} offset={}/{} attempt={} error={}",
                        attachment.id,
                        attachment.direction,
                        attachment.status,
                        attachment.offset,
                        attachment.size,
                        attachment.attempt_count,
                        attachment.last_error_code.as_deref().unwrap_or("none")
                    ),
                );
                self.last_attachment_log_state.insert(attachment.id.clone(), current);
            }
        }

        for radio in &snapshot.radio.contacts {
            let current = format!(
                "local_enabled={} remote_state={} state={}",
                radio.local_enabled, radio.remote_state, radio.state
            );
            if self.last_radio_log_state.get(&radio.contact_id) != Some(&current) {
                let level = if radio.state == "ready" || radio.state == "receiving" {
                    Level::Info
                } else if radio.state == "reconnecting" || radio.state == "unavailable" {
                    Level::Warn
                } else {
                    Level::Debug
                };
                self.log(
                    "radio",
                    level,
                    "session",
                    "RADIO_STATE_CHANGED",
                    &format!("contact={} {}", radio.contact_id, current),
                );
                self.last_radio_log_state.insert(radio.contact_id.clone(), current);
            }
        }
        let session_state = snapshot.radio.session.as_ref().map_or_else(
            || "none".to_owned(),
            |session| {
                format!(
                    "contact={} state={} floor={} burst_elapsed_ms={}",
                    session.contact_id, session.state, session.floor, session.burst_elapsed_ms
                )
            },
        );
        if self.last_radio_log_state.get("active_session") != Some(&session_state) {
            self.log("radio", Level::Info, "session", "RADIO_SESSION_CHANGED", &session_state);
            self.last_radio_log_state.insert("active_session".into(), session_state);
        }

        let tor = snapshot
            .bootstrap_steps
            .iter()
            .find(|step| step.id == "tor_network")
            .map(|step| (step.state.clone(), step.code.clone()));
        // `NETWORK_READY` is the point at which the local Tor runtime is
        // usable. Relay and local onion publication are independent,
        // recoverable capabilities; requiring either one here used to leave a
        // fully functioning local application permanently on warming-up.
        let network_ready = tor.as_ref().is_some_and(|(state, _)| state == "ready");
        if network_ready && !self.network_ready_logged {
            self.log(
                "bootstrap",
                Level::Info,
                "network",
                "NETWORK_READY",
                "Tor runtime is ready; relay and local onion continue in the background",
            );
            self.network_ready_logged = true;
        }
    }

    pub(crate) fn reconcile_pending_operations(&mut self) -> bool {
        let before_snapshot = self.snapshot_json.clone();
        let before_cursor = self.notification_cursor;
        let _ = self.application_runtime.advance_pending_operations();

        // Pending work is completed by the application runtime without a
        // foreground FFI command. Refresh the read model here so the actor can
        // publish one revision for contacts, messages, receipts and radio
        // projections created by that background work. Without this refresh
        // the event hub compares the same stale JSON before/after maintenance
        // and Flutter waits for its 30-second safety timeout.
        let _ = self.refresh_snapshot();
        self.snapshot_json != before_snapshot || self.notification_cursor != before_cursor
    }

    pub(crate) fn next_pending_operation_delay(&self) -> Option<std::time::Duration> {
        self.application_runtime.next_pending_operation_delay()
    }

    fn apply_host_state_hint(&self, snapshot: &mut torca_contract::BridgeSnapshot) {
        // The local application runtime is the shell gate. Tor, onion and relay
        // are independently observable capabilities and operation prerequisites.
        // Once RuntimeOwner is attached, the application bootstrap state is
        // authoritative: it waits for the required local onion capability and
        // a terminal relay check before exposing profile setup. Before that,
        // project only the host startup failure/running state.
        if !self.application_runtime.has_runtime() {
            snapshot.bootstrap_phase = projected_host_bootstrap_phase(self.host_state_hint).into();
        }
        let network_state = if self.host_progress >= 100 {
            "ready"
        } else if matches!(self.host_state_hint, TorState::Degraded | TorState::Failed) {
            "failed"
        } else if self.host_retry_at.is_some() {
            // Retry is an active bootstrap step. Keep the wire state inside
            // the canonical BootstrapStepState vocabulary; retry_at_ms and
            // the diagnostic code carry the richer retry information.
            "running"
        } else if self.host_start.is_some() {
            "running"
        } else {
            "pending"
        };
        debug_assert!(canonical_bootstrap_wire_state(network_state));
        if let Some(step) =
            snapshot.bootstrap_steps.iter_mut().find(|step| step.id == "tor_network")
        {
            step.state = network_state.into();
            step.code = self.host_status_code.clone();
            step.progress = self.host_progress;
            step.attempt = self.host_attempt;
            step.started_at_ms = self.host_start_started_at_ms;
            step.last_progress_at_ms = self.host_last_progress_at_ms;
            step.retry_at_ms = self.host_retry_at.and_then(instant_to_unix_ms);
        }
        // Once RuntimeOwner is attached, its live OnionService probe is the
        // authority. The startup observer is intentionally finite and keeping
        // projecting its last percentage produced false STALLED states minutes
        // after Arti had already reported Reachable.
        if self.application_runtime.has_runtime() {
            return;
        }
        let onion_stalled = self.host_onion_attempt > 0
            && self.host_onion_progress < 100
            && self.host_onion_last_progress_at_ms.is_some_and(|last_progress| {
                unix_time_ms().ok().and_then(|now| now.checked_sub(last_progress)).is_some_and(
                    |elapsed_ms| {
                        elapsed_ms
                            >= i64::try_from(ONION_PROGRESS_STALL_AFTER.as_millis())
                                .unwrap_or(i64::MAX)
                    },
                )
            });
        let onion_state = if self.host_progress < 100 {
            if network_state == "failed" { "blocked" } else { "pending" }
        } else if self.host_onion_progress >= 100 {
            "ready"
        } else if matches!(self.host_state_hint, TorState::Degraded | TorState::Failed) {
            "failed"
        } else if self.host_onion_retry_at.is_some() {
            "running"
        } else if onion_stalled {
            // A stalled publication is still being observed by the startup
            // worker. `verifying` keeps elapsed time visible while the
            // diagnostic code explains why progress has paused.
            "verifying"
        } else if self.host_onion_attempt > 0 {
            "running"
        } else {
            "pending"
        };
        debug_assert!(canonical_bootstrap_wire_state(onion_state));
        if let Some(step) =
            snapshot.bootstrap_steps.iter_mut().find(|step| step.id == "onion_service")
        {
            step.state = onion_state.into();
            step.code = if onion_state == "blocked" {
                Some("TOR_NETWORK_REQUIRED".into())
            } else if onion_stalled {
                Some("ONION_PUBLICATION_STALLED".into())
            } else {
                self.host_onion_status_code.clone()
            };
            step.progress = self.host_onion_progress;
            step.attempt = self.host_onion_attempt;
            step.started_at_ms = self.host_onion_started_at_ms;
            step.last_progress_at_ms = self.host_onion_last_progress_at_ms;
            step.retry_at_ms = self.host_onion_retry_at.and_then(instant_to_unix_ms);
        }
    }

    pub(crate) fn conversation_page(
        &mut self,
        conversation_id: &str,
        before_at_ms: Option<i64>,
        before_message_id: Option<&str>,
        limit: usize,
    ) -> i32 {
        if self.is_closed() {
            return ABI_CLOSED;
        }
        let conversation_id = match conversation_id.parse::<OpaqueId>() {
            Ok(value) => ConversationId::from_opaque(value),
            Err(_) => return self.query_error("invalid conversation id"),
        };
        let before = match (before_at_ms, before_message_id) {
            (Some(at_ms), Some(message_id)) => {
                let at = match Timestamp::from_unix_millis(at_ms) {
                    Ok(value) => value,
                    Err(_) => return self.query_error("invalid page timestamp"),
                };
                let message_id = match message_id.parse::<OpaqueId>() {
                    Ok(value) => MessageId::from_opaque(value),
                    Err(_) => return self.query_error("invalid page message id"),
                };
                Some((at, message_id))
            }
            (None, None) => None,
            _ => return self.query_error("incomplete page cursor"),
        };
        match self.read_models().history.page_for_conversation(conversation_id, before, limit) {
            Ok(page) => {
                let page = BridgeMessagePage {
                    messages: page.messages.into_iter().map(bridge_message_from_domain).collect(),
                    has_more: page.has_more,
                };
                self.query_json = bridge_message_page_json(&page);
                ABI_OK
            }
            Err(_) => self.query_error("conversation history unavailable"),
        }
    }

    pub(crate) fn search_messages(
        &mut self,
        conversation_id: &str,
        query: &str,
        limit: usize,
    ) -> i32 {
        if self.is_closed() {
            return ABI_CLOSED;
        }
        let conversation_id = match conversation_id.parse::<OpaqueId>() {
            Ok(value) => ConversationId::from_opaque(value),
            Err(_) => return self.query_error("invalid conversation id"),
        };
        match self.read_models().history.search_conversation(conversation_id, query, limit) {
            Ok(messages) => {
                let page = BridgeMessagePage {
                    messages: messages.into_iter().map(bridge_message_from_domain).collect(),
                    has_more: false,
                };
                self.query_json = bridge_message_page_json(&page);
                ABI_OK
            }
            Err(_) => self.query_error("conversation search unavailable"),
        }
    }

    pub(crate) fn close(&mut self) -> i32 {
        self.log(
            "runtime",
            Level::Info,
            "native",
            "RUNTIME_STOPPING",
            "Native runtime shutdown requested",
        );
        self.host_retry_at = None;
        self.host_failures = 0;
        self.host_state_hint = TorState::Stopped;
        self.host_start = None;
        self.host_start_started_at = None;
        self.host_start_started_at_ms = None;
        self.host_last_progress_at_ms = None;
        self.host_progress = 0;
        self.host_attempt = 0;
        self.host_status_code = None;
        self.host_status_summary = None;
        self.host_onion_started_at_ms = None;
        self.host_onion_last_progress_at_ms = None;
        self.host_onion_progress = 0;
        self.host_onion_attempt = 0;
        self.host_onion_status_code = None;
        self.host_onion_status_summary = None;
        self.host_onion_retry_at = None;
        self.host_start_deadline = None;
        if let Some(host) = self.host.take()
            && host.shutdown().is_err()
        {
            self.last_result_json = error_result("secure runtime shutdown failed");
        }
        let Some(actor) = self.actor.take() else {
            if let Some(logger) = &self.logger {
                let _ = logger.finish("completed", "runtime already stopped");
            }
            return ABI_OK;
        };
        match actor.shutdown() {
            Ok(()) => {
                if let Some(logger) = &self.logger {
                    let _ = logger.finish("completed", "runtime stopped");
                }
                ABI_OK
            }
            Err(error) => {
                self.log(
                    "runtime",
                    Level::Error,
                    "native",
                    "RUNTIME_STOP_FAILED",
                    &error.to_string(),
                );
                if let Some(logger) = &self.logger {
                    let _ = logger.finish("failed", &error.to_string());
                }
                self.last_result_json = error_result(&error.to_string());
                ABI_ERROR
            }
        }
    }

    pub(crate) fn notification_events_json(&mut self, after_cursor: u64) -> i32 {
        let _ = self.collect_notification_events();
        let events = self
            .notification_events
            .iter()
            .filter(|event| event.cursor > after_cursor)
            .map(notification_event_json)
            .collect::<Vec<_>>();
        self.query_json = serde_json::json!({
            "afterCursor": after_cursor,
            "events": events,
        })
        .to_string();
        ABI_OK
    }

    pub(crate) fn diagnostics_json(&mut self) -> i32 {
        match self.application_runtime.diagnostics_json() {
            Ok(diagnostics) => {
                let mut value = serde_json::from_str::<Value>(&diagnostics)
                    .unwrap_or_else(|_| Value::Object(serde_json::Map::new()));
                if let Some(counters) = value.get_mut("counters").and_then(Value::as_object_mut)
                    && let Some(stats) = self.event_hub.stats()
                {
                    // This is a real count of successful native event waits,
                    // not an estimate of physical battery consumption.
                    counters.insert("ffiWakes".into(), Value::from(stats.wakeups));
                }
                if let Some(counters) = value.get_mut("counters").and_then(Value::as_object_mut) {
                    counters.insert(
                        "radioWakeups".into(),
                        Value::from(self.application_runtime.radio_wake_count()),
                    );
                }
                self.query_json = value.to_string();
                ABI_OK
            }
            Err(error) => {
                self.last_result_json = error_result(&error.to_string());
                ABI_ERROR
            }
        }
    }

    pub(crate) fn avatar_genome_json(&mut self, identity_id: Option<&str>) -> i32 {
        match self.application_runtime.avatar_genome_json(identity_id) {
            Ok(value) => {
                self.query_json = value;
                ABI_OK
            }
            Err(_) => {
                self.query_json = serde_json::json!({
                    "errorCode": "avatar_unavailable"
                })
                .to_string();
                ABI_ERROR
            }
        }
    }

    pub(crate) fn parse_pairing_uri(&mut self, raw_uri: &str) -> i32 {
        let parsed =
            torca_pairing_coordinator::decode_invite_uri(raw_uri).map(Some).or_else(|_| {
                torca_pairing::PairingCode::new(raw_uri).map(|code| (code, None)).map(Some)
            });
        let Ok(Some((code, ticket))) = parsed else {
            self.query_json = "{}".into();
            return ABI_ERROR;
        };
        self.query_json = serde_json::json!({
            "code": code.as_str(),
            "ticket": ticket.as_ref().map(|value| value.as_hex()),
        })
        .to_string();
        ABI_OK
    }

    pub(crate) fn encode_pairing_uri(&mut self, raw_code: &str) -> i32 {
        let Ok(code) = torca_pairing::PairingCode::new(raw_code) else {
            self.query_json = "{}".into();
            return ABI_ERROR;
        };
        self.query_json = serde_json::json!({
            "uri": torca_pairing_coordinator::encode_invite_uri(&code, None),
        })
        .to_string();
        ABI_OK
    }

    fn collect_notification_events(&mut self) -> Result<(), ()> {
        if !self.notifications_enabled {
            return Ok(());
        }
        let summaries = self.read_models().history.conversation_summaries().map_err(|_| ())?;
        let snapshot = self
            .application_runtime
            .snapshot_context()
            .map(bridge_snapshot_from_application)
            .map_err(|_| ())?;
        let contact_names = snapshot
            .contacts
            .iter()
            .map(|contact| (contact.id.clone(), contact.display_name.clone()))
            .collect::<HashMap<_, _>>();
        for pairing in &snapshot.pairings {
            if pairing.role != "creator"
                || !matches!(pairing.state.as_str(), "peer_joined" | "awaiting_approval")
                || !self.pairing_notification_seen.insert(pairing.id.clone())
            {
                continue;
            }
            self.notification_cursor = self.notification_cursor.saturating_add(1);
            let event_id = crate::torca_runtime::secure_id_hex()
                .unwrap_or_else(|_| format!("notification-{}", self.notification_cursor));
            let intent = torca_notifications::notification_intent(
                torca_notifications::NotificationEvent::PairingApprovalRequested {
                    pairing_id: OpaqueId::from_u128(self.notification_cursor as u128),
                },
                torca_notifications::NotificationPrivacy::Redacted,
                None,
            );
            self.notification_events.push(torca_contract::NotificationEvent {
                cursor: self.notification_cursor,
                event_id,
                kind: "pairing_request".into(),
                conversation_id: String::new(),
                contact_display_name: pairing
                    .remote_display_name
                    .clone()
                    .filter(|name| !name.trim().is_empty())
                    .unwrap_or_else(|| "New contact request".into()),
                created_at_ms: pairing.expires_at_ms.saturating_sub(5 * 60 * 1_000),
                title: intent.as_ref().map_or_else(|| "Torca".into(), |value| value.title.clone()),
                body: intent.as_ref().map_or_else(String::new, |value| value.body.clone()),
            });
        }
        for contact in &snapshot.contacts {
            if !self.contact_notification_seen.insert(contact.id.clone()) {
                continue;
            }
            self.notification_cursor = self.notification_cursor.saturating_add(1);
            let event_id = crate::torca_runtime::secure_id_hex()
                .unwrap_or_else(|_| format!("notification-{}", self.notification_cursor));
            let conversation_id = snapshot
                .conversations
                .iter()
                .find(|conversation| conversation.contact_id == contact.id)
                .map_or_else(String::new, |conversation| conversation.id.clone());
            let intent = torca_notifications::notification_intent(
                torca_notifications::NotificationEvent::ContactAdded {
                    contact_id: OpaqueId::from_u128(self.notification_cursor as u128),
                },
                torca_notifications::NotificationPrivacy::Redacted,
                None,
            );
            self.notification_events.push(torca_contract::NotificationEvent {
                cursor: self.notification_cursor,
                event_id,
                kind: "contact_added".into(),
                conversation_id,
                contact_display_name: contact.display_name.clone(),
                created_at_ms: contact.created_at_ms,
                title: intent.as_ref().map_or_else(|| "Torca".into(), |value| value.title.clone()),
                body: intent.as_ref().map_or_else(String::new, |value| value.body.clone()),
            });
        }
        for (conversation_id, summary) in summaries {
            let key = conversation_id.to_string();
            let unread = summary.unread_count;
            let previous = self.notification_seen.insert(key.clone(), unread).unwrap_or(0);
            let Some(message) = summary.last_message else { continue };
            if message.direction() != MessageDirection::Inbound || unread <= previous {
                continue;
            }
            self.notification_cursor = self.notification_cursor.saturating_add(1);
            let contact_display_name = snapshot_contact_name(&snapshot, &contact_names, &key);
            let event_id = crate::torca_runtime::secure_id_hex()
                .unwrap_or_else(|_| format!("notification-{}", self.notification_cursor));
            let intent = torca_notifications::notification_intent(
                torca_notifications::NotificationEvent::IncomingMessage {
                    contact_id: OpaqueId::from_u128(self.notification_cursor as u128),
                    conversation_id: OpaqueId::from_u128(self.notification_cursor as u128),
                },
                torca_notifications::NotificationPrivacy::Redacted,
                None,
            );
            self.notification_events.push(torca_contract::NotificationEvent {
                cursor: self.notification_cursor,
                event_id,
                kind: "message_received".into(),
                conversation_id: key,
                contact_display_name,
                created_at_ms: message.created_at().to_unix_millis(),
                title: intent.as_ref().map_or_else(|| "Torca".into(), |value| value.title.clone()),
                body: intent.as_ref().map_or_else(String::new, |value| value.body.clone()),
            });
        }
        if self.notification_events.len() > 256 {
            let remove = self.notification_events.len() - 256;
            self.notification_events.drain(..remove);
        }
        Ok(())
    }

    pub(crate) fn lifecycle(&mut self, event: &str) -> i32 {
        if !matches!(
            event,
            "host_started"
                | "foregrounded"
                | "backgrounded"
                | "network_changed"
                | "low_memory"
                | "terminating"
        ) {
            self.last_result_json = error_result("unknown lifecycle event");
            return ABI_ERROR;
        }
        self.log("runtime", Level::Info, "lifecycle", "LIFECYCLE_EVENT", event);
        let radio_lifecycle = match event {
            "foregrounded" | "host_started" => {
                Some(torca_radio_coordinator::HostRadioLifecycle::Foreground)
            }
            "backgrounded" => Some(torca_radio_coordinator::HostRadioLifecycle::Background),
            "terminating" => Some(torca_radio_coordinator::HostRadioLifecycle::Terminating),
            _ => None,
        };
        if let Some(lifecycle) = radio_lifecycle {
            let _ = self.application_runtime.radio_lifecycle(lifecycle);
        }
        if event == "network_changed" {
            if let Some(host) = &self.host {
                host.network_changed();
            } else {
                self.network_changed_pending = true;
            }
        }
        if event == "terminating" { self.close() } else { ABI_OK }
    }

    fn apply_history_summaries(
        &self,
        snapshot: &mut torca_contract::BridgeSnapshot,
    ) -> Result<(), &'static str> {
        let summaries = self
            .read_models()
            .history
            .conversation_summaries()
            .map_err(|_| "conversation summaries unavailable")?;
        for conversation in &mut snapshot.conversations {
            let id = conversation
                .id
                .parse::<OpaqueId>()
                .map(ConversationId::from_opaque)
                .map_err(|_| "invalid conversation id in snapshot")?;
            let Some(summary) = summaries.get(&id) else { continue };
            conversation.unread_count = summary.unread_count;
            conversation.last_activity_at_ms = summary.last_activity_at.to_unix_millis();
            if let Some(message) = &summary.last_message {
                conversation.last_message_body = Some(message.body().as_str().to_owned());
                conversation.last_message_direction =
                    Some(torca_contract::message_direction_name(message.direction()).into());
                conversation.last_message_status =
                    Some(torca_contract::message_status_name(message.status()).into());
            }
        }
        Ok(())
    }

    fn apply_security_states(
        &self,
        snapshot: &mut torca_contract::BridgeSnapshot,
    ) -> Result<(), &'static str> {
        let states = self
            .read_models()
            .security
            .contact_states()
            .map_err(|_| "contact security state unavailable")?;
        for contact in &mut snapshot.contacts {
            let id = contact
                .id
                .parse::<OpaqueId>()
                .map(torca_contacts::ContactId::from_opaque)
                .map_err(|_| "invalid contact id in snapshot")?;
            let Some(security) = states.get(&id) else { continue };
            contact.verification_status = match security.state {
                ContactSecurityState::Unverified => "unverified",
                ContactSecurityState::Verified => "verified",
                ContactSecurityState::IdentityChanged => "changed",
            }
            .into();
            contact.verified_at_ms = security.verified_at.map(|at| at.to_unix_millis());
        }
        Ok(())
    }

    fn apply_navigation_badges(&self, snapshot: &mut torca_contract::BridgeSnapshot) {
        snapshot.unread_messages_count = snapshot
            .conversations
            .iter()
            .fold(0_u32, |total, conversation| total.saturating_add(conversation.unread_count));
        let acknowledged_at =
            self.read_models().settings.new_contacts_acknowledged_at_ms().ok().flatten();
        snapshot.new_contacts_count = snapshot
            .contacts
            .iter()
            .filter(|contact| match acknowledged_at {
                Some(at) => contact.created_at_ms > at,
                None => true,
            })
            .count()
            .try_into()
            .unwrap_or(u32::MAX);
        snapshot.pairing_attention_count = snapshot
            .pairings
            .iter()
            .filter(|pairing| pairing.role == "creator" && pairing.state == "awaitingapproval")
            .count()
            .try_into()
            .unwrap_or(u32::MAX);
    }

    fn query_error(&mut self, error: &str) -> i32 {
        self.last_result_json = error_result(error);
        self.query_json = "{\"messages\":[],\"hasMore\":false}".into();
        ABI_ERROR
    }

    fn begin_runtime_start(&mut self) {
        if self.is_closed() || self.host.is_some() || self.host_start.is_some() {
            return;
        }
        self.host_retry_at = None;
        self.host_state_hint = TorState::Starting;
        self.log(
            "bootstrap",
            Level::Info,
            "runtime",
            "TOR_STARTING",
            "Starting production network runtime",
        );
        let engine = self.application_runtime.handle().engine_handle();
        let (sender, receiver) = mpsc::channel::<HostStartEvent>();
        self.host_start = Some(receiver);
        self.host_start_started_at = Some(Instant::now());
        self.host_start_started_at_ms = unix_time_ms().ok();
        self.host_last_progress_at_ms = self.host_start_started_at_ms;
        self.host_progress = 0;
        self.host_attempt = 1;
        self.host_status_code = Some("TOR_BOOTSTRAP_STARTING".into());
        self.host_status_summary = Some("Starting embedded Tor bootstrap".into());
        self.host_onion_started_at_ms = None;
        self.host_onion_last_progress_at_ms = None;
        self.host_onion_progress = 0;
        self.host_onion_attempt = 0;
        self.host_onion_status_code = None;
        self.host_onion_status_summary = None;
        self.host_onion_retry_at = None;
        self.host_start_deadline = Some(Instant::now() + NETWORK_START_OBSERVE_TIMEOUT);
        let progress_sender = sender.clone();
        let observer: TorBootstrapObserver = std::sync::Arc::new(move |progress| {
            let _ = progress_sender.send(HostStartEvent::Progress(progress));
        });
        let read_receipt_policy = self.read_receipt_policy.clone();
        thread::spawn(move || {
            let result = match catch_unwind(AssertUnwindSafe(|| {
                spawn_production_runtime(engine, observer, read_receipt_policy)
            })) {
                Ok(result) => result,
                Err(payload) => Err(NativeCompositionError::new(format!(
                    "production network runtime worker panicked: {}",
                    panic_message(payload)
                ))),
            };
            if let Err(send_error) = sender.send(HostStartEvent::Finished(result))
                && let HostStartEvent::Finished(Ok((_handle, owner, _radio))) = send_error.0
            {
                let _ = owner.shutdown();
            }
        });
    }

    fn create_bootstrap_identity(&mut self) -> Result<(), ()> {
        let identity_id_hex = crate::torca_runtime::secure_id_hex().map_err(|_| ())?;
        let elapsed = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| ())?;
        let at_ms = i64::try_from(elapsed.as_millis()).map_err(|_| ())?;
        let identity_id = identity_id_hex.parse::<OpaqueId>().map_err(|_| ())?;
        self.application_runtime.bootstrap_identity(identity_id, at_ms).map(|_| ()).map_err(|_| ())
    }

    fn apply_bootstrap_progress(&mut self, progress: &TorBootstrapEvent) -> bool {
        let retry_at = progress
            .retry_after_ms
            .and_then(|delay_ms| Instant::now().checked_add(Duration::from_millis(delay_ms)));
        match progress.stage {
            TorBootstrapStage::Network => {
                let changed = progress.progress != self.host_progress
                    || self.host_status_code.as_deref() != Some(progress.code)
                    || progress.attempt != self.host_attempt
                    || self.host_status_summary.as_deref() != Some(progress.summary.as_str());
                if progress.progress > self.host_progress {
                    self.host_last_progress_at_ms = unix_time_ms().ok();
                }
                self.host_progress = self.host_progress.max(progress.progress);
                self.host_attempt = progress.attempt;
                self.host_status_code = Some(progress.code.into());
                self.host_status_summary = Some(progress.summary.clone());
                self.host_retry_at = retry_at;
                changed
            }
            TorBootstrapStage::OnionService => {
                let changed = progress.progress != self.host_onion_progress
                    || self.host_onion_status_code.as_deref() != Some(progress.code)
                    || progress.attempt != self.host_onion_attempt
                    || self.host_onion_status_summary.as_deref() != Some(progress.summary.as_str());
                let now_ms = unix_time_ms().ok();
                if self.host_onion_started_at_ms.is_none() {
                    self.host_onion_started_at_ms = now_ms;
                    self.host_onion_last_progress_at_ms = now_ms;
                }
                if progress.progress > self.host_onion_progress {
                    self.host_onion_last_progress_at_ms = now_ms;
                }
                self.host_onion_progress = self.host_onion_progress.max(progress.progress);
                self.host_onion_attempt = progress.attempt;
                self.host_onion_status_code = Some(progress.code.into());
                self.host_onion_status_summary = Some(progress.summary.clone());
                self.host_onion_retry_at = retry_at;
                changed
            }
        }
    }

    #[allow(clippy::while_let_loop)]
    fn advance_runtime_start(&mut self) {
        let mut outcome = None;
        loop {
            let event = match self.host_start.as_ref() {
                Some(receiver) => receiver.try_recv(),
                None => break,
            };
            match event {
                Ok(HostStartEvent::Progress(progress)) => {
                    let changed = self.apply_bootstrap_progress(&progress);
                    if changed {
                        self.log("tor", Level::Info, "bootstrap", progress.code, &progress.summary);
                    }
                }
                Ok(HostStartEvent::Finished(result)) => {
                    outcome = Some(result);
                    break;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    outcome = Some(Err(NativeCompositionError::new(
                        "network runtime startup worker disconnected",
                    )));
                    break;
                }
            }
        }
        if let Some(result) = outcome {
            self.host_start = None;
            self.host_start_started_at = None;
            self.host_start_deadline = None;
            match result {
                Ok((handle, owner, radio)) => {
                    if self.network_changed_pending {
                        handle.network_changed();
                        self.network_changed_pending = false;
                    }
                    self.application_runtime.attach_runtime(handle);
                    self.application_runtime.attach_radio(radio);
                    self.host = Some(owner);
                    self.host_retry_at = None;
                    self.host_failures = 0;
                    self.host_state_hint = TorState::Ready;
                    self.host_progress = 100;
                    self.host_attempt = self.host_attempt.max(1);
                    self.host_status_code = Some("TOR_BOOTSTRAP_READY".into());
                    self.host_status_summary = Some("Tor network bootstrap completed".into());
                    self.host_onion_progress = self.host_onion_progress.max(5);
                    self.host_onion_status_code = Some("ONION_SERVICE_PUBLISHING".into());
                    self.host_onion_status_summary =
                        Some("Waiting for private onion service reachability".into());
                    self.log(
                        "bootstrap",
                        Level::Info,
                        "runtime",
                        "TOR_READY",
                        "Production network runtime is ready",
                    );
                }
                Err(error) => {
                    self.host_failures = self.host_failures.saturating_add(1);
                    let retry_exhausted = self.host_failures >= NETWORK_MAX_ATTEMPTS;
                    self.host_state_hint =
                        if retry_exhausted { TorState::Failed } else { TorState::Degraded };
                    self.log(
                        "bootstrap",
                        Level::Error,
                        "runtime",
                        "RUNTIME_START_FAILED",
                        &format!("Production network runtime start failed: {error}"),
                    );
                    self.host_retry_at = (!retry_exhausted).then(|| {
                        let delay = match self.host_failures {
                            1 => Duration::from_secs(5),
                            2 => Duration::from_secs(15),
                            _ => NETWORK_RETRY_DELAY,
                        };
                        Instant::now() + delay
                    });
                    if self.host_retry_at.is_some() {
                        self.host_status_code = Some("TOR_BOOTSTRAP_RETRYING".into());
                        self.host_last_progress_at_ms = unix_time_ms().ok();
                    } else {
                        self.host_status_code = Some("TOR_RUNTIME_FAILED".into());
                    }
                }
            }
        } else if self.host_start.is_some()
            && self.host_start_deadline.is_some_and(|deadline| Instant::now() >= deadline)
        {
            // Tor's first directory bootstrap can legitimately exceed two
            // minutes (Android has observed several-minute cold starts).  Keep
            // the worker and the UI in the starting state so a late success is
            // adopted instead of presenting a false terminal failure.
            self.host_start_deadline = None;
            self.log(
                "bootstrap",
                Level::Warn,
                "runtime",
                "RUNTIME_START_SLOW",
                "Production network runtime is still bootstrapping after 120 seconds",
            );
        }
        if self.host.is_none()
            && self.host_start.is_none()
            && self.host_retry_at.is_some_and(|deadline| Instant::now() >= deadline)
            && self.has_identity().unwrap_or(false)
        {
            self.begin_runtime_start();
        }
    }

    fn has_identity(&self) -> Result<bool, ()> {
        self.application_runtime
            .handle()
            .overview()
            .map(|snapshot| snapshot.identity.is_some())
            .map_err(|_| ())
    }

    fn is_closed(&self) -> bool {
        self.actor.is_none()
    }

    fn log(&self, domain: &str, level: Level, component: &str, code: &str, message: &str) {
        if let Some(logger) = &self.logger {
            let _ = logger.event(domain, level, component, code, message);
        }
    }

    fn log_profile(&self, request_id: &str, code: &str) {
        if let Some(logger) = &self.logger {
            let context = json!({
                "requestId": request_id,
                "operation": "profile.set",
                "stage": code,
            })
            .to_string();
            let _ = logger.event_with_context(
                "profile",
                Level::Info,
                "profile",
                code,
                "profile operation stage",
                Some(&context),
            );
        }
    }

    fn log_pairing(
        &self,
        request_id: &str,
        operation: &str,
        session_id: &str,
        code: &str,
        outcome: Option<&str>,
    ) {
        if let Some(logger) = &self.logger {
            let context = json!({
                "requestId": request_id,
                "operation": operation,
                "sessionId": session_id,
                "outcome": outcome,
            })
            .to_string();
            let level = if code == "PAIRING_REQUEST_FAILED" { Level::Error } else { Level::Info };
            let _ = logger.event_with_context(
                "pairing",
                level,
                "command",
                code,
                "pairing operation stage",
                Some(&context),
            );
        }
    }
}

fn network_transition_event(
    component: &str,
    (state, diagnostic): &(String, Option<String>),
) -> (Level, String, String) {
    let suffix = match state.as_str() {
        "ready" => "READY",
        "failed" => "FAILED",
        "degraded" => "DEGRADED",
        "retrying" => "RETRYING",
        "running" | "checking" => "CONNECTING",
        _ => "PENDING",
    };
    let level = match suffix {
        "FAILED" => Level::Error,
        "DEGRADED" | "RETRYING" => Level::Warn,
        _ => Level::Info,
    };
    let code = format!("{component}_{suffix}");
    let detail = diagnostic.as_deref().unwrap_or("no diagnostic code");
    let message = format!("{component} state changed to {state} ({detail})");
    (level, code, message)
}

fn canonical_bootstrap_wire_state(state: &str) -> bool {
    matches!(
        state,
        "pending" | "running" | "verifying" | "ready" | "degraded" | "failed" | "blocked"
    )
}

const fn projected_host_bootstrap_phase(host_state: TorState) -> &'static str {
    if matches!(host_state, TorState::Degraded | TorState::Failed) { "degraded" } else { "running" }
}

fn notification_event_json(event: &torca_contract::NotificationEvent) -> serde_json::Value {
    serde_json::json!({
        "cursor": event.cursor,
        "eventId": event.event_id,
        "kind": event.kind,
        "conversationId": event.conversation_id,
        "contactDisplayName": event.contact_display_name,
        "createdAtMs": event.created_at_ms,
    })
}

fn snapshot_contact_name(
    snapshot: &torca_contract::BridgeSnapshot,
    contact_names: &HashMap<String, String>,
    conversation_id: &str,
) -> String {
    snapshot
        .conversations
        .iter()
        .find(|conversation| conversation.id == conversation_id)
        .and_then(|conversation| contact_names.get(&conversation.contact_id))
        .cloned()
        .unwrap_or_default()
}

fn panic_message(payload: Box<dyn Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_owned()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "non-string panic payload".to_owned()
    }
}

fn unix_time_ms() -> Result<i64, ()> {
    let elapsed = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| ())?;
    i64::try_from(elapsed.as_millis()).map_err(|_| ())
}

fn instant_to_unix_ms(deadline: Instant) -> Option<i64> {
    let remaining = deadline.checked_duration_since(Instant::now()).unwrap_or_default();
    let remaining_ms = i64::try_from(remaining.as_millis()).ok()?;
    unix_time_ms().ok()?.checked_add(remaining_ms)
}

impl Drop for TorcaRuntime {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

#[cfg(test)]
mod tests {
    use super::{
        TorState, canonical_bootstrap_wire_state, notification_event_json,
        projected_host_bootstrap_phase,
    };

    #[test]
    fn host_projection_cannot_unlock_profile_before_runtime_bootstrap_finishes() {
        assert_eq!(projected_host_bootstrap_phase(TorState::Starting), "running");
        assert_eq!(projected_host_bootstrap_phase(TorState::Ready), "running");
        assert_eq!(projected_host_bootstrap_phase(TorState::Failed), "degraded");
    }

    #[test]
    fn host_bootstrap_projection_uses_only_contract_states() {
        for state in ["pending", "running", "verifying", "ready", "degraded", "failed", "blocked"] {
            assert!(canonical_bootstrap_wire_state(state));
        }
        assert!(!canonical_bootstrap_wire_state("retrying"));
        assert!(!canonical_bootstrap_wire_state("stalled"));
    }

    #[test]
    fn notification_wire_uses_created_at_ms() {
        let event = torca_contract::NotificationEvent {
            cursor: 11,
            event_id: "event-11".into(),
            kind: "message_received".into(),
            conversation_id: "conversation-1".into(),
            contact_display_name: "Alice".into(),
            created_at_ms: 1_700_000_000_123,
            title: "New message".into(),
            body: "Private message received".into(),
        };
        let value = notification_event_json(&event);
        assert_eq!(value["createdAtMs"], 1_700_000_000_123_i64);
        assert!(value.get("createdAt").is_none());
    }
}
