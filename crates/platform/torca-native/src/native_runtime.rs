use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

use torca_bridge::{
    BridgeMessagePage, EngineBridge, bridge_message_from_domain,
};
use torca_client_engine::{ClientEngineActor, EngineHandle};
use torca_conversations::ConversationId;
use torca_foundation::{OpaqueId, Timestamp};
use torca_messaging::MessageId;
use torca_runtime_host::{HostTorState, RuntimeHostHandle, RuntimeHostOwner};
use torca_storage_sqlite::SqlCipherMessageStore;

use crate::composition::{NativeCompositionError, spawn_production_engine};
use crate::json::{
    bridge_message_page_json, bridge_result_json, bridge_snapshot_json, empty_snapshot_json,
    error_result, success_result,
};
use crate::notification_json::notification_snapshot_json;
use crate::runtime_composition::spawn_production_host;

pub(crate) const ABI_OK: i32 = 0;
pub(crate) const ABI_ERROR: i32 = -1;
pub(crate) const ABI_CLOSED: i32 = -2;
const NETWORK_RETRY_DELAY: Duration = Duration::from_secs(5);

type HostStartResult = Result<(RuntimeHostHandle, RuntimeHostOwner), NativeCompositionError>;

pub struct NativeEngineRuntime {
    engine: EngineHandle,
    bridge: EngineBridge,
    actor: Option<ClientEngineActor>,
    history: SqlCipherMessageStore,
    host: Option<RuntimeHostOwner>,
    host_start: Option<Receiver<HostStartResult>>,
    host_retry_at: Option<Instant>,
    host_state_hint: HostTorState,
    pub(crate) last_result_json: String,
    pub(crate) snapshot_json: String,
    pub(crate) diagnostics_json: String,
    pub(crate) query_json: String,
}

impl NativeEngineRuntime {
    pub(crate) fn new() -> Result<Self, ()> {
        let parts = spawn_production_engine().map_err(|_| ())?;
        let engine = parts.engine;
        let bridge = EngineBridge::new(engine.clone());
        let mut runtime = Self {
            engine,
            bridge,
            actor: Some(parts.actor),
            history: parts.history,
            host: None,
            host_start: None,
            host_retry_at: None,
            host_state_hint: HostTorState::Stopped,
            last_result_json: success_result("initialized"),
            snapshot_json: empty_snapshot_json(),
            diagnostics_json: "{\"events\":[]}".into(),
            query_json: "{\"messages\":[],\"hasMore\":false}".into(),
        };
        if runtime.has_identity()? {
            runtime.begin_runtime_start();
        }
        if runtime.refresh_snapshot() != ABI_OK { return Err(()); }
        Ok(runtime)
    }

    pub(crate) fn execute(&mut self, command: torca_bridge::BridgeCommand) -> i32 {
        if self.is_closed() {
            self.last_result_json = error_result("native engine is closed");
            return ABI_CLOSED;
        }
        self.advance_runtime_start();
        let creates_identity = matches!(&command, torca_bridge::BridgeCommand::CreateIdentity { .. });
        let result = self.bridge.execute(command);
        if !result.ok {
            self.last_result_json = bridge_result_json(&result);
            return ABI_ERROR;
        }
        if creates_identity && !self.bridge.has_runtime() {
            self.begin_runtime_start();
        }
        self.last_result_json = bridge_result_json(&result);
        let _ = self.refresh_snapshot();
        ABI_OK
    }

    pub(crate) fn refresh_snapshot(&mut self) -> i32 {
        if self.is_closed() { return ABI_CLOSED; }
        self.advance_runtime_start();
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
        if !self.bridge.has_runtime() && snapshot.identity_name.is_some() {
            snapshot.tor_state = format!("{:?}", self.host_state_hint).to_lowercase();
        }
        self.snapshot_json = bridge_snapshot_json(&snapshot);
        ABI_OK
    }

    pub(crate) fn conversation_page(
        &mut self,
        conversation_id: &str,
        before_at_ms: Option<i64>,
        before_message_id: Option<&str>,
        limit: usize,
    ) -> i32 {
        if self.is_closed() { return ABI_CLOSED; }
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
        if self.is_closed() { return ABI_CLOSED; }
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

    pub(crate) fn notification_snapshot_json(&self) -> Result<String, ()> {
        if self.is_closed() { return Err(()); }
        self.bridge
            .full_snapshot()
            .map(|snapshot| notification_snapshot_json(&snapshot))
            .map_err(|_| ())
    }

    pub(crate) fn refresh_diagnostics(&mut self) -> i32 {
        if self.is_closed() { return ABI_CLOSED; }
        self.advance_runtime_start();
        match self.bridge.diagnostics_json() {
            Ok(value) => { self.diagnostics_json = value; ABI_OK }
            Err(error) => { self.last_result_json = error_result(&error.to_string()); ABI_ERROR }
        }
    }

    pub(crate) fn reject_argument(&mut self, error: &'static str) -> i32 {
        self.last_result_json = error_result(error);
        ABI_ERROR
    }

    pub(crate) fn close(&mut self) -> i32 {
        self.host_retry_at = None;
        self.host_state_hint = HostTorState::Stopped;
        self.host_start = None;
        if let Some(host) = self.host.take() {
            if host.shutdown().is_err() {
                self.last_result_json = error_result("secure runtime shutdown failed");
            }
        }
        let Some(actor) = self.actor.take() else { return ABI_OK; };
        match actor.shutdown() {
            Ok(()) => ABI_OK,
            Err(error) => { self.last_result_json = error_result(&error.to_string()); ABI_ERROR }
        }
    }

    fn apply_history_summaries(
        &self,
        snapshot: &mut torca_bridge::BridgeSnapshot,
    ) -> Result<(), &'static str> {
        let summaries = self.history.conversation_summaries().map_err(|_| "conversation summaries unavailable")?;
        for conversation in &mut snapshot.conversations {
            let id = conversation.id.parse::<OpaqueId>().map(ConversationId::from_opaque)
                .map_err(|_| "invalid conversation id in snapshot")?;
            let Some(summary) = summaries.get(&id) else { continue };
            conversation.unread_count = summary.unread_count;
            conversation.last_activity_at_ms = summary.last_activity_at.to_unix_millis();
            if let Some(message) = &summary.last_message {
                conversation.last_message_body = Some(message.body().as_str().to_owned());
                conversation.last_message_direction = Some(format!("{:?}", message.direction()).to_lowercase());
                conversation.last_message_status = Some(format!("{:?}", message.status()).to_lowercase());
            }
        }
        Ok(())
    }

    fn query_error(&mut self, error: &str) -> i32 {
        self.last_result_json = error_result(error);
        self.query_json = "{\"messages\":[],\"hasMore\":false}".into();
        ABI_ERROR
    }

    fn begin_runtime_start(&mut self) {
        if self.is_closed() || self.host.is_some() || self.host_start.is_some() { return; }
        self.host_retry_at = None;
        self.host_state_hint = HostTorState::Starting;
        let engine = self.engine.clone();
        let (sender, receiver) = mpsc::channel::<HostStartResult>();
        self.host_start = Some(receiver);
        thread::spawn(move || {
            let result = spawn_production_host(engine);
            if let Err(send_error) = sender.send(result) {
                if let Ok((_handle, owner)) = send_error.0 { let _ = owner.shutdown(); }
            }
        });
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
            match result {
                Ok((handle, owner)) => {
                    self.bridge.attach_runtime(handle);
                    self.host = Some(owner);
                    self.host_retry_at = None;
                    self.host_state_hint = HostTorState::Ready;
                }
                Err(_) => {
                    self.host_state_hint = HostTorState::Degraded;
                    self.host_retry_at = Some(Instant::now() + NETWORK_RETRY_DELAY);
                }
            }
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

    fn is_closed(&self) -> bool { self.actor.is_none() }
}
impl Drop for NativeEngineRuntime {
    fn drop(&mut self) { let _ = self.close(); }
}
