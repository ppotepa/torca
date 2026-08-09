use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use std::{
    any::Any,
    panic::{AssertUnwindSafe, catch_unwind},
};

use serde_json::json;
use torca_client_engine::{ClientEngineActor, EngineHandle};
use torca_contract::{BridgeMessagePage, ContractRuntime, bridge_message_from_domain};
use torca_conversations::ConversationId;
use torca_foundation::{OpaqueId, Timestamp};
use torca_logging::{Level, Logger, default_root};
use torca_messaging::MessageDirection;
use torca_messaging::MessageId;
use torca_runtime::{RuntimeHandle, RuntimeOwner, TorState};
use torca_storage_sqlite::{
    ContactSecurityState, SqlCipherMessageStore, SqlCipherSecurityProjection,
    SqlCipherSettingsStore,
};

use crate::composition::{NativeCompositionError, spawn_production_engine};
use crate::json::{
    bridge_message_page_json, bridge_result_json, bridge_snapshot_json, empty_snapshot_json,
    error_result, success_result,
};
use crate::runtime_composition::spawn_production_runtime;

pub(crate) const ABI_OK: i32 = 0;
pub(crate) const ABI_ERROR: i32 = -1;
pub(crate) const ABI_CLOSED: i32 = -2;
const NETWORK_RETRY_DELAY: Duration = Duration::from_secs(5);
const NETWORK_START_OBSERVE_TIMEOUT: Duration = Duration::from_secs(120);
const NETWORK_MAX_ATTEMPTS: u32 = 3;

type HostStartResult = Result<(RuntimeHandle, RuntimeOwner), NativeCompositionError>;

pub struct TorcaRuntime {
    engine: EngineHandle,
    bridge: ContractRuntime,
    actor: Option<ClientEngineActor>,
    history: SqlCipherMessageStore,
    security: SqlCipherSecurityProjection,
    settings: SqlCipherSettingsStore,
    host: Option<RuntimeOwner>,
    host_start: Option<Receiver<HostStartResult>>,
    host_start_deadline: Option<Instant>,
    host_retry_at: Option<Instant>,
    host_failures: u32,
    host_state_hint: TorState,
    pub(crate) last_result_json: String,
    pub(crate) snapshot_json: String,
    pub(crate) query_json: String,
    logger: Option<Logger>,
    notification_seen: HashMap<String, u32>,
    pub(crate) notification_cursor: u64,
    notification_events: Vec<torca_contract::NotificationEvent>,
    notifications_enabled: bool,
}

impl TorcaRuntime {
    pub(crate) fn new() -> Result<Self, String> {
        let parts = spawn_production_engine()
            .map_err(|error| format!("native engine composition failed: {error}"))?;
        let engine = parts.engine;
        let bridge = ContractRuntime::new(engine.clone());
        #[cfg(target_os = "android")]
        let log_root =
            crate::composition::android::log_root_path().unwrap_or_else(|_| default_root());
        #[cfg(not(target_os = "android"))]
        let log_root = default_root();
        let logger = match Logger::new(
            log_root,
            std::env::var("TORCA_DEVICE_ID").unwrap_or_else(|_| "native".into()),
            crate::torca_runtime::compiled_build_id(),
        ) {
            Ok(logger) => Some(logger),
            Err(error) => {
                // A logger failure must never erase the only startup
                // diagnostic.  Packaged Windows/Android launches still have
                // stderr/logcat available for collection.
                eprintln!("Torca native logger startup failed: {error}");
                None
            }
        };
        let mut runtime = Self {
            engine,
            bridge,
            actor: Some(parts.actor),
            history: parts.history,
            security: parts.security,
            host: None,
            host_start: None,
            host_start_deadline: None,
            host_retry_at: None,
            host_failures: 0,
            host_state_hint: TorState::Stopped,
            last_result_json: success_result("initialized"),
            snapshot_json: empty_snapshot_json(),
            query_json: "{\"messages\":[],\"hasMore\":false}".into(),
            logger,
            notification_seen: HashMap::new(),
            notification_cursor: 0,
            notification_events: Vec::new(),
            notifications_enabled: parts.settings.notifications_enabled().unwrap_or(true),
            settings: parts.settings,
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
                let _ = self.settings.set_notifications_enabled(
                    *enabled,
                    i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX),
                );
            }
        }
        let is_profile = matches!(&command, torca_contract::BridgeCommand::UpdateProfile { .. });
        if is_profile {
            self.log_profile(request_id, "PROFILE_REQUEST_RECEIVED");
            let ready = self
                .bridge
                .snapshot()
                .map(|snapshot| {
                    matches!(snapshot.bootstrap_phase.as_str(), "ready_for_profile" | "ready")
                })
                .unwrap_or(false);
            if !ready {
                self.last_result_json = error_result("PROFILE_NOT_READY");
                return ABI_ERROR;
            }
            self.log_profile(request_id, "PROFILE_COMMAND_QUEUED");
            self.log_profile(request_id, "PROFILE_COMMAND_STARTED");
            self.log_profile(request_id, "PROFILE_STORAGE_STARTED");
        }
        if let Some(conversation_id) = command_conversation_id(&command) {
            match self.security.requires_reverification(conversation_id) {
                Ok(true) => {
                    self.last_result_json = error_result(
                        "contact identity changed; safety number re-verification required",
                    );
                    return ABI_ERROR;
                }
                Ok(false) => {}
                Err(_) => {
                    self.last_result_json = error_result("contact security state unavailable");
                    return ABI_ERROR;
                }
            }
        }
        let result = self.bridge.execute(command);
        if !result.ok {
            if is_profile {
                self.log_profile(request_id, "PROFILE_STORAGE_FAILED");
            }
            self.log(
                "bridge",
                Level::Error,
                "command",
                "BRIDGE_COMMAND_FAILED",
                "Bridge command rejected by native engine",
            );
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
        ABI_OK
    }

    pub(crate) fn refresh_snapshot(&mut self) -> i32 {
        if self.is_closed() {
            return ABI_CLOSED;
        }
        self.advance_runtime_start();
        if let Err(error) = self.bridge.advance_bootstrap() {
            self.last_result_json = error_result(&error.to_string());
            return ABI_ERROR;
        }
        let mut snapshot = match self.bridge.snapshot() {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.last_result_json = error_result(&error.to_string());
                return ABI_ERROR;
            }
        };
        if let Err(error) = self.apply_history_summaries(&mut snapshot) {
            self.last_result_json = error_result(error);
            return ABI_ERROR;
        }
        if let Err(error) = self.apply_security_states(&mut snapshot) {
            self.last_result_json = error_result(error);
            return ABI_ERROR;
        }
        if !self.bridge.has_runtime() && snapshot.identity_name.is_some() {
            snapshot.tor_state = format!("{:?}", self.host_state_hint).to_lowercase();
            self.apply_host_state_hint(&mut snapshot);
        }
        let snapshot_json = bridge_snapshot_json(&snapshot);
        self.snapshot_json = serde_json::from_str::<serde_json::Value>(&snapshot_json)
            .map(|mut value| {
                value["notificationsEnabled"] = serde_json::Value::Bool(self.notifications_enabled);
                value.to_string()
            })
            .unwrap_or(snapshot_json);
        ABI_OK
    }

    fn apply_host_state_hint(&self, snapshot: &mut torca_contract::BridgeSnapshot) {
        let (phase, state, code) = match self.host_state_hint {
            TorState::Starting => ("starting", "running", None),
            TorState::Degraded | TorState::Failed => {
                ("failed", "failed", Some("TOR_RUNTIME_FAILED"))
            }
            TorState::Stopped => ("idle", "pending", None),
            TorState::Ready => ("starting", "running", None),
        };
        snapshot.bootstrap_phase = phase.into();
        if let Some(step) =
            snapshot.bootstrap_steps.iter_mut().find(|step| step.id == "tor_network")
        {
            step.state = state.into();
            step.code = code.map(str::to_owned);
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
        match self.history.page_for_conversation(conversation_id, before, limit) {
            Ok(page) => {
                let page = BridgeMessagePage {
                    messages: page.messages.into_iter().map(bridge_message_from_domain).collect(),
                    has_more: page.has_more,
                };
                self.query_json = bridge_message_page_json(&page);
                ABI_OK
            }
            Err(error) => self.query_error(&error.to_string()),
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
        match self.history.search_conversation(conversation_id, query, limit) {
            Ok(messages) => {
                let page = BridgeMessagePage {
                    messages: messages.into_iter().map(bridge_message_from_domain).collect(),
                    has_more: false,
                };
                self.query_json = bridge_message_page_json(&page);
                ABI_OK
            }
            Err(error) => self.query_error(&error.to_string()),
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
        self.host_start_deadline = None;
        if let Some(host) = self.host.take() {
            if host.shutdown().is_err() {
                self.last_result_json = error_result("secure runtime shutdown failed");
            }
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
            .map(|event| {
                serde_json::json!({
                    "cursor": event.cursor,
                    "eventId": event.event_id,
                    "kind": event.kind,
                    "conversationId": event.conversation_id,
                    "contactDisplayName": event.contact_display_name,
                    "createdAt": event.created_at_ms,
                })
            })
            .collect::<Vec<_>>();
        self.query_json = serde_json::json!({
            "afterCursor": after_cursor,
            "events": events,
        })
        .to_string();
        ABI_OK
    }

    pub(crate) fn parse_pairing_uri(&mut self, raw_uri: &str) -> i32 {
        let Some(query) = raw_uri.strip_prefix("torca://pair?") else {
            self.query_json = "{}".into();
            return ABI_ERROR;
        };
        let mut version = None;
        let mut code = None;
        for field in query.split('&') {
            let Some((key, value)) = field.split_once('=') else { continue };
            match key {
                "v" => version = Some(value),
                "code" => code = Some(value),
                _ => {}
            }
        }
        let Some(code) = code
            .filter(|_| version == Some("1"))
            .map(str::to_ascii_uppercase)
            .filter(|value| (6..=16).contains(&value.len()))
            .filter(|value| {
                value.bytes().all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
            })
        else {
            self.query_json = "{}".into();
            return ABI_ERROR;
        };
        self.query_json = serde_json::json!({ "code": code }).to_string();
        ABI_OK
    }

    fn collect_notification_events(&mut self) -> Result<(), ()> {
        if !self.notifications_enabled {
            return Ok(());
        }
        let summaries = self.history.conversation_summaries().map_err(|_| ())?;
        let snapshot = self.bridge.snapshot().map_err(|_| ())?;
        let contact_names = snapshot
            .contacts
            .iter()
            .map(|contact| (contact.id.clone(), contact.display_name.clone()))
            .collect::<HashMap<_, _>>();
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
            self.notification_events.push(torca_contract::NotificationEvent {
                cursor: self.notification_cursor,
                event_id,
                kind: "message_received".into(),
                conversation_id: key,
                contact_display_name,
                created_at_ms: message.created_at().to_unix_millis(),
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
        if event == "terminating" { self.close() } else { ABI_OK }
    }

    fn apply_history_summaries(
        &self,
        snapshot: &mut torca_contract::BridgeSnapshot,
    ) -> Result<(), &'static str> {
        let summaries = self
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
                    Some(format!("{:?}", message.direction()).to_lowercase());
                conversation.last_message_status =
                    Some(format!("{:?}", message.status()).to_lowercase());
            }
        }
        Ok(())
    }

    fn apply_security_states(
        &self,
        snapshot: &mut torca_contract::BridgeSnapshot,
    ) -> Result<(), &'static str> {
        let states =
            self.security.contact_states().map_err(|_| "contact security state unavailable")?;
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
        let engine = self.engine.clone();
        let (sender, receiver) = mpsc::channel::<HostStartResult>();
        self.host_start = Some(receiver);
        self.host_start_deadline = Some(Instant::now() + NETWORK_START_OBSERVE_TIMEOUT);
        thread::spawn(move || {
            let result = match catch_unwind(AssertUnwindSafe(|| spawn_production_runtime(engine))) {
                Ok(result) => result,
                Err(payload) => Err(NativeCompositionError::new(format!(
                    "production network runtime worker panicked: {}",
                    panic_message(payload)
                ))),
            };
            if let Err(send_error) = sender.send(result) {
                if let Ok((_handle, owner)) = send_error.0 {
                    let _ = owner.shutdown();
                }
            }
        });
    }

    fn create_bootstrap_identity(&mut self) -> Result<(), ()> {
        let identity_id_hex = crate::torca_runtime::secure_id_hex().map_err(|_| ())?;
        let elapsed = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| ())?;
        let at_ms = i64::try_from(elapsed.as_millis()).map_err(|_| ())?;
        let result = self.bridge.bootstrap_identity(&identity_id_hex, at_ms);
        if result.ok { Ok(()) } else { Err(()) }
    }

    fn advance_runtime_start(&mut self) {
        let outcome = match self.host_start.as_ref() {
            Some(receiver) => match receiver.try_recv() {
                Ok(result) => Some(result),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => Some(Err(NativeCompositionError::new(
                    "network runtime startup worker disconnected",
                ))),
            },
            None => None,
        };
        if let Some(result) = outcome {
            self.host_start = None;
            self.host_start_deadline = None;
            match result {
                Ok((handle, owner)) => {
                    self.bridge.attach_runtime(handle);
                    self.host = Some(owner);
                    self.host_retry_at = None;
                    self.host_failures = 0;
                    self.host_state_hint = TorState::Ready;
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
                    self.host_state_hint = if self.host_failures >= NETWORK_MAX_ATTEMPTS {
                        TorState::Failed
                    } else {
                        TorState::Degraded
                    };
                    self.log(
                        "bootstrap",
                        Level::Error,
                        "runtime",
                        "RUNTIME_START_FAILED",
                        &format!("Production network runtime start failed: {error}"),
                    );
                    self.host_retry_at = (self.host_failures < NETWORK_MAX_ATTEMPTS).then(|| {
                        let delay = match self.host_failures {
                            1 => Duration::from_secs(5),
                            2 => Duration::from_secs(15),
                            _ => NETWORK_RETRY_DELAY,
                        };
                        Instant::now() + delay
                    });
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
        self.engine.overview_snapshot().map(|snapshot| snapshot.identity.is_some()).map_err(|_| ())
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

fn command_conversation_id(command: &torca_contract::BridgeCommand) -> Option<ConversationId> {
    let raw = match command {
        torca_contract::BridgeCommand::QueueMessage { conversation_id_hex, .. }
        | torca_contract::BridgeCommand::QueueAttachment { conversation_id_hex, .. } => {
            conversation_id_hex
        }
        _ => return None,
    };
    raw.parse::<OpaqueId>().ok().map(ConversationId::from_opaque)
}

impl Drop for TorcaRuntime {
    fn drop(&mut self) {
        let _ = self.close();
    }
}
