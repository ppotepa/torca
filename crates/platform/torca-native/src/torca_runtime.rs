use core::{ptr, slice, str};
use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use torca_contract::{BridgeCommand, CONTRACT_VERSION, generated};
use torca_crypto::{CryptoProvider, RustCryptoProvider};

use crate::native_runtime::{ABI_OK, TorcaRuntime};

const NATIVE_ABI: u16 = 1;
const STORAGE_EPOCH: u16 = 2;
const MAILBOX_CAPACITY: usize = 256;
const DEFAULT_QUERY_TIMEOUT: Duration = Duration::from_secs(5);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(15);
const PENDING_RECONCILE_INTERVAL: Duration = Duration::from_secs(1);
const IDEMPOTENCY_MAX_ENTRIES: usize = 1024;
const IDEMPOTENCY_TTL: Duration = Duration::from_secs(15 * 60);
const BUILD_ID: &str = match option_env!("TORCA_BUILD_ID") {
    Some(value) => value,
    None => "dev",
};
const PRODUCT_VERSION: &str = match option_env!("TORCA_PRODUCT_VERSION") {
    Some(value) => value,
    None => "0.2.0-alpha.0",
};
const SOURCE_COMMIT: &str = match option_env!("TORCA_SOURCE_COMMIT") {
    Some(value) => value,
    None => "working-tree",
};
const SOURCE_FINGERPRINT: &str = match option_env!("TORCA_SOURCE_FINGERPRINT") {
    Some(value) => value,
    None => "development",
};
const RELAY_ENDPOINT_HASH: &str = match option_env!("TORCA_RELAY_ENDPOINT_HASH") {
    Some(value) => value,
    None => "configured-at-build",
};

/// Build identity compiled into the native library.  Runtime logging must use
/// this value rather than an environment variable: packaged Windows/Android
/// launches do not inherit the build shell environment.
pub(crate) const fn compiled_build_id() -> &'static str {
    BUILD_ID
}

static REGISTRY: OnceLock<Mutex<Option<Arc<RuntimeHandleInner>>>> = OnceLock::new();
static METADATA: OnceLock<Vec<u8>> = OnceLock::new();
static INITIALIZATION_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

enum ActorMessage {
    Invoke {
        request: String,
        response: SyncSender<Vec<u8>>,
    },
    #[allow(dead_code)]
    Lifecycle {
        event: String,
        response: SyncSender<i32>,
    },
    Shutdown {
        response: SyncSender<()>,
    },
}

struct RuntimeHandleInner {
    sender: SyncSender<ActorMessage>,
    startup_error: Option<String>,
}

#[repr(C)]
pub struct TorcaRuntimeHandle {
    inner: Arc<RuntimeHandleInner>,
    response: Mutex<Vec<u8>>,
}

struct ActorState {
    runtime: TorcaRuntime,
    runtime_id: String,
    revision: u64,
    completed: IdempotencyLedger,
    next_pending_reconcile_at: Instant,
}

struct CompletedCommand {
    response: Vec<u8>,
    completed_at: Instant,
}

struct IdempotencyLedger {
    entries: HashMap<String, CompletedCommand>,
    order: VecDeque<String>,
    max_entries: usize,
    ttl: Duration,
}

impl Default for IdempotencyLedger {
    fn default() -> Self {
        Self::with_limits(IDEMPOTENCY_MAX_ENTRIES, IDEMPOTENCY_TTL)
    }
}

impl IdempotencyLedger {
    fn with_limits(max_entries: usize, ttl: Duration) -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            max_entries: max_entries.max(1),
            ttl,
        }
    }

    fn prune(&mut self, now: Instant) {
        while let Some(request_id) = self.order.front().cloned() {
            let expired = self
                .entries
                .get(&request_id)
                .is_none_or(|entry| now.duration_since(entry.completed_at) >= self.ttl);
            if !expired && self.entries.len() <= self.max_entries {
                break;
            }
            self.order.pop_front();
            self.entries.remove(&request_id);
        }
    }

    fn get(&mut self, request_id: &str, now: Instant) -> Option<Vec<u8>> {
        self.prune(now);
        self.entries.get(request_id).map(|entry| entry.response.clone())
    }

    fn insert(&mut self, request_id: String, response: Vec<u8>, now: Instant) {
        self.prune(now);
        if self.entries.contains_key(&request_id) {
            self.order.retain(|value| value != &request_id);
        }
        self.entries.insert(request_id.clone(), CompletedCommand { response, completed_at: now });
        self.order.push_back(request_id);
        self.prune(now);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn torca_runtime_metadata_ptr() -> *const u8 {
    metadata().as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn torca_runtime_metadata_len() -> usize {
    metadata().len()
}

fn metadata() -> &'static [u8] {
    METADATA.get_or_init(|| {
        serde_json::to_vec(&json!({
            "productVersion": PRODUCT_VERSION,
            "buildNumber": 1,
            "buildId": BUILD_ID,
            "sourceCommit": SOURCE_COMMIT,
            "sourceFingerprint": SOURCE_FINGERPRINT,
            "nativeAbi": NATIVE_ABI,
            "contractSchema": CONTRACT_VERSION,
            "storageEpoch": STORAGE_EPOCH,
            "schemaVersion": 1,
            "wireVersion": 1,
            "relayEndpointHash": RELAY_ENDPOINT_HASH,
            "targetPlatform": std::env::consts::OS,
            "targetArchitecture": std::env::consts::ARCH,
            "capabilities": {
                "maxAttachmentBytes": torca_attachments::MAX_ATTACHMENT_BYTES,
                "maxVideoAttachmentBytes": 5 * 1024 * 1024,
                "maxQueuedAttachments": 5,
                "maxAttachmentSourceBytes": 64 * 1024 * 1024,
            },
        }))
        .expect("static runtime metadata is serializable")
    })
}

#[cfg(target_os = "android")]
pub(crate) fn notification_snapshot_json(after_cursor: u64) -> String {
    let registry = REGISTRY.get_or_init(|| Mutex::new(None));
    let inner = match registry.lock().ok().and_then(|guard| guard.as_ref().cloned()) {
        Some(inner) => inner,
        None => return crate::notification_json::notification_events_json(after_cursor),
    };
    let request = torca_contract::notification_poll_request_json(
        &format!("android-notifications-{after_cursor}"),
        after_cursor,
    );
    let (tx, rx) = mpsc::sync_channel(1);
    if send_with_timeout(
        &inner.sender,
        ActorMessage::Invoke { request, response: tx },
        Duration::from_secs(2),
    )
    .is_err()
    {
        return crate::notification_json::notification_events_json(after_cursor);
    }
    let Ok(response) = rx.recv_timeout(DEFAULT_QUERY_TIMEOUT) else {
        return crate::notification_json::notification_events_json(after_cursor);
    };
    extract_notification_snapshot(&response).map_or_else(
        || crate::notification_json::notification_events_json(after_cursor),
        |value| value.to_string(),
    )
}

#[cfg(any(target_os = "android", test))]
fn extract_notification_snapshot(response: &[u8]) -> Option<Value> {
    let response = serde_json::from_slice::<Value>(response).ok()?;
    let runtime_id = response.get("runtimeId")?.as_str()?.to_owned();
    let mut snapshot = response.get("snapshot")?.clone();
    snapshot.as_object_mut()?.insert("runtimeId".into(), Value::String(runtime_id));
    Some(snapshot)
}

#[unsafe(no_mangle)]
pub extern "C" fn torca_alloc(length: usize) -> *mut u8 {
    if length == 0 {
        return ptr::null_mut();
    }
    Box::into_raw(vec![0_u8; length].into_boxed_slice()).cast::<u8>()
}

#[unsafe(no_mangle)]
/// # Safety
/// `data` must be a pointer and length previously returned by this library.
pub unsafe extern "C" fn torca_free(data: *mut u8, length: usize) {
    if data.is_null() || length == 0 {
        return;
    }
    let raw = ptr::slice_from_raw_parts_mut(data, length);
    unsafe {
        drop(Box::from_raw(raw));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn torca_runtime_acquire() -> *mut TorcaRuntimeHandle {
    let registry = REGISTRY.get_or_init(|| Mutex::new(None));
    let existing = match registry.lock() {
        Ok(guard) => guard.as_ref().cloned(),
        Err(_) => return ptr::null_mut(),
    };
    let inner = if let Some(existing) = existing {
        existing
    } else {
        // Never hold the registry mutex while constructing storage, identity or
        // the network runtime. A separate initialization gate serializes the
        // slow path, then the registry is acquired only for the short publish.
        let initialization = INITIALIZATION_LOCK.get_or_init(|| Mutex::new(()));
        let Ok(_initialization_guard) = initialization.lock() else {
            return ptr::null_mut();
        };
        if let Ok(guard) = registry.lock() {
            if let Some(existing) = guard.as_ref().cloned() {
                existing
            } else {
                drop(guard);
                let Ok(value) = spawn_runtime() else {
                    return ptr::null_mut();
                };
                let Ok(mut guard) = registry.lock() else {
                    return ptr::null_mut();
                };
                if let Some(existing) = guard.as_ref().cloned() {
                    existing
                } else {
                    *guard = Some(Arc::clone(&value));
                    value
                }
            }
        } else {
            return ptr::null_mut();
        }
    };
    Box::into_raw(Box::new(TorcaRuntimeHandle { inner, response: Mutex::new(Vec::new()) }))
}

fn spawn_runtime() -> Result<Arc<RuntimeHandleInner>, ()> {
    let (sender, receiver) = mpsc::sync_channel(MAILBOX_CAPACITY);
    let (ready_tx, ready_rx) = mpsc::sync_channel::<Result<(), String>>(1);
    thread::Builder::new()
        .name("torca-runtime".into())
        .spawn(move || match TorcaRuntime::new() {
            Ok(runtime) => {
                let runtime_id = secure_id_hex().unwrap_or_else(|_| "runtime-unavailable".into());
                let _ = ready_tx.send(Ok(()));
                actor_loop(
                    receiver,
                    ActorState {
                        runtime,
                        runtime_id,
                        revision: 1,
                        completed: IdempotencyLedger::default(),
                        next_pending_reconcile_at: Instant::now(),
                    },
                );
            }
            Err(error) => {
                eprintln!("Torca runtime startup failed: {error}");
                let _ = ready_tx.send(Err(error));
            }
        })
        .map_err(|_| ())?;
    let startup_error = match ready_rx.recv_timeout(Duration::from_secs(10)) {
        Ok(Ok(())) => None,
        Ok(Err(error)) => Some(error),
        Err(_) => return Err(()),
    };
    Ok(Arc::new(RuntimeHandleInner { sender, startup_error }))
}

fn actor_loop(receiver: Receiver<ActorMessage>, mut state: ActorState) {
    loop {
        let message = match receiver.recv_timeout(Duration::from_secs(1)) {
            Ok(message) => message,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                state.maintain();
                continue;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        };
        match message {
            ActorMessage::Invoke { request, response } => {
                let _ = response.send(state.invoke(&request));
                // Flutter polls snapshots frequently.  Maintenance must be
                // deadline-driven, not dependent on finding a one-second gap
                // in the actor mailbox, otherwise durable pairing work can
                // remain queued forever while the UI is open.
                state.maintain();
            }
            ActorMessage::Lifecycle { event, response } => {
                let _ = response.send(state.runtime.lifecycle(&event));
                state.maintain();
            }
            ActorMessage::Shutdown { response } => {
                let _ = state.runtime.close();
                let _ = response.send(());
                break;
            }
        }
    }
}

#[allow(dead_code)]
fn dispatch_lifecycle(event: &str) -> i32 {
    let registry = REGISTRY.get_or_init(|| Mutex::new(None));
    let Some(inner) = registry.lock().ok().and_then(|guard| guard.as_ref().cloned()) else {
        return -1;
    };
    let (tx, rx) = mpsc::sync_channel(1);
    if send_with_timeout(
        &inner.sender,
        ActorMessage::Lifecycle { event: event.to_owned(), response: tx },
        Duration::from_secs(2),
    )
    .is_err()
    {
        return -1;
    }
    rx.recv_timeout(DEFAULT_QUERY_TIMEOUT).unwrap_or(-1)
}

impl ActorState {
    fn maintain(&mut self) {
        if Instant::now() < self.next_pending_reconcile_at {
            return;
        }
        self.next_pending_reconcile_at = Instant::now() + PENDING_RECONCILE_INTERVAL;
        // This is the sole reconciliation path.  Snapshot reads are pure and
        // cannot trigger network traffic or queue mutations.
        self.runtime.reconcile_pending_operations();
    }

    fn invoke(&mut self, raw: &str) -> Vec<u8> {
        let started = Instant::now();
        let request: Value = match serde_json::from_str(raw) {
            Ok(value) => value,
            Err(_) => {
                return self.error(
                    "",
                    "CONTRACT_REQUEST_INVALID",
                    "contract.request.invalid",
                    false,
                );
            }
        };
        let request_id = request.get("requestId").and_then(Value::as_str).unwrap_or_default();
        if request.get("schema").and_then(Value::as_u64) != Some(1) {
            return self.error(
                request_id,
                "CONTRACT_SCHEMA_MISMATCH",
                "contract.schema.mismatch",
                false,
            );
        }
        let name = request.get("name").and_then(Value::as_str).unwrap_or_default();
        let payload = request.get("payload").cloned().unwrap_or_else(|| json!({}));
        let kind = request.get("kind").and_then(Value::as_str).unwrap_or_default();
        if !generated::contains(kind, name) {
            return self.error(
                request_id,
                "CONTRACT_OPERATION_UNKNOWN",
                "contract.operation.unknown",
                false,
            );
        }
        // requestId is an idempotency key for mutating commands only. Queries
        // must always observe the current runtime state, even when a caller
        // retries them with the same transport correlation id.
        if is_idempotent_command(kind)
            && !request_id.is_empty()
            && let Some(response) = self.completed.get(request_id, Instant::now())
        {
            return response;
        }
        let before_snapshot = self.runtime.snapshot_json.clone();
        let before_notification_cursor = self.runtime.notification_cursor;
        let code = match (kind, name) {
            ("query", "snapshot.get") => self.runtime.refresh_snapshot(),
            ("query", "conversation.page") => {
                let conversation =
                    payload.get("conversationId").and_then(Value::as_str).unwrap_or_default();
                let before =
                    payload.get("beforeMessageId").and_then(Value::as_str).unwrap_or_default();
                let before_at_ms = payload.get("beforeAtMs").and_then(Value::as_i64);
                let limit =
                    payload.get("limit").and_then(Value::as_u64).unwrap_or(100).clamp(1, 200)
                        as u32;
                let cursor = if before.is_empty() { None } else { Some(before) };
                self.runtime.conversation_page(conversation, before_at_ms, cursor, limit as usize)
            }
            ("query", "conversation.search") => {
                let conversation =
                    payload.get("conversationId").and_then(Value::as_str).unwrap_or_default();
                let query = payload.get("query").and_then(Value::as_str).unwrap_or_default();
                let limit =
                    payload.get("limit").and_then(Value::as_u64).unwrap_or(100).clamp(1, 200)
                        as u32;
                self.runtime.search_messages(conversation, query, limit as usize)
            }
            ("query", "notifications.poll") => {
                let cursor = payload.get("afterCursor").and_then(Value::as_u64).unwrap_or(0);
                self.runtime.notification_events_json(cursor)
            }
            ("query", "runtime.poll") => {
                let cursor = payload.get("afterCursor").and_then(Value::as_u64).unwrap_or(0);
                let snapshot_code = self.runtime.refresh_snapshot();
                if snapshot_code != ABI_OK {
                    snapshot_code
                } else {
                    let events_code = self.runtime.notification_events_json(cursor);
                    if events_code == ABI_OK {
                        let snapshot = serde_json::from_str::<Value>(&self.runtime.snapshot_json)
                            .unwrap_or(Value::Null);
                        let events = serde_json::from_str::<Value>(&self.runtime.query_json)
                            .unwrap_or(Value::Null);
                        self.runtime.query_json = serde_json::json!({
                            "snapshot": snapshot,
                            "events": events.get("events").cloned().unwrap_or_else(|| serde_json::json!([])),
                            "afterCursor": events.get("afterCursor").cloned().unwrap_or(Value::from(cursor)),
                        }).to_string();
                    }
                    events_code
                }
            }
            ("query", "diagnostics.get") => self.runtime.diagnostics_json(),
            ("query", "pairing.parse") => {
                let uri = payload.get("uri").and_then(Value::as_str).unwrap_or_default();
                self.runtime.parse_pairing_uri(uri)
            }
            ("query", "pairing.encode") => {
                let code = payload.get("code").and_then(Value::as_str).unwrap_or_default();
                self.runtime.encode_pairing_uri(code)
            }
            ("command", _) => {
                let command = match bridge_command(name, &payload) {
                    Ok(command) => command,
                    Err((code, key)) => return self.error(request_id, code, key, false),
                };
                self.runtime.execute_with_request_id(command, request_id)
            }
            ("lifecycle", event) => self.runtime.lifecycle(event),
            _ => {
                return self.error(
                    request_id,
                    "CONTRACT_OPERATION_UNKNOWN",
                    "contract.operation.unknown",
                    false,
                );
            }
        };
        if code != ABI_OK {
            return self.native_error(request_id);
        }
        let counts_for_revision = operation_counts_for_revision(kind, name);
        let state_changed = counts_for_revision
            && (self.runtime.snapshot_json != before_snapshot
                || self.runtime.notification_cursor != before_notification_cursor);
        if state_changed {
            self.revision = self.revision.saturating_add(1);
        }
        let mut snapshot: Value = if name == "conversation.page"
            || name == "conversation.search"
            || name == "notifications.poll"
            || name == "runtime.poll"
            || name == "diagnostics.get"
            || name == "pairing.parse"
            || name == "pairing.encode"
        {
            serde_json::from_str(&self.runtime.query_json).unwrap_or(Value::Null)
        } else {
            serde_json::from_str(&self.runtime.snapshot_json).unwrap_or(Value::Null)
        };
        if name != "conversation.page"
            && name != "conversation.search"
            && name != "notifications.poll"
            && name != "runtime.poll"
            && name != "diagnostics.get"
            && let Value::Object(object) = &mut snapshot
        {
            object.insert("runtimeId".into(), Value::String(self.runtime_id.clone()));
            object.insert("revision".into(), Value::from(self.revision));
            object
                .insert("notificationCursor".into(), Value::from(self.runtime.notification_cursor));
        }
        let operation_result =
            serde_json::from_str::<Value>(&self.runtime.last_result_json).unwrap_or(Value::Null);
        let result_kind = operation_result
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or(if name == "profile.set" { "profile_updated" } else { "snapshot" });
        let resource_id = operation_result.get("resourceId").cloned().unwrap_or(Value::Null);
        let invite_uri = operation_result.get("inviteUri").cloned().unwrap_or(Value::Null);
        let response = serde_json::to_vec(&json!({
            "schema": 1, "requestId": request_id, "status": "succeeded",
            "resultKind": result_kind,
            "resourceId": resource_id,
            "inviteUri": invite_uri,
            "runtimeId": self.runtime_id, "revision": self.revision, "snapshot": snapshot,
            "error": Value::Null, "timing": { "queuedMs": 0, "executionMs": started.elapsed().as_millis() }
        })).expect("runtime response is serializable");
        if is_idempotent_command(kind) && !request_id.is_empty() {
            self.completed.insert(request_id.to_owned(), response.clone(), Instant::now());
        }
        response
    }

    fn error(&self, request_id: &str, code: &str, message_key: &str, retryable: bool) -> Vec<u8> {
        serde_json::to_vec(&json!({
            "schema": 1, "requestId": request_id, "status": "failed", "resultKind": "error",
            "runtimeId": self.runtime_id, "revision": self.revision, "snapshot": Value::Null,
            "error": { "code": code, "category": "runtime", "severity": "error",
                "retryable": retryable, "messageKey": message_key, "diagnosticId": secure_id_hex().unwrap_or_default() },
            "timing": { "queuedMs": 0, "executionMs": 0 }
        })).expect("runtime error is serializable")
    }

    fn native_error(&self, request_id: &str) -> Vec<u8> {
        let descriptor = serde_json::from_str::<Value>(&self.runtime.last_result_json)
            .ok()
            .and_then(|value| {
                let kind = value.get("kind")?.as_str()?.strip_prefix("error:")?.to_owned();
                Some(kind)
            })
            .unwrap_or_else(|| "RUNTIME_OPERATION_FAILED".into());
        let normalized = descriptor.to_ascii_uppercase();
        let message_key = match normalized.as_str() {
            "PROFILE_NOT_READY" => "profile.not_ready",
            "PROFILE_SNAPSHOT_INCONSISTENT" => "profile.snapshot.inconsistent",
            "RELAY_DEGRADED" => "relay.degraded",
            "RELAY_NOT_READY" => "relay.not_ready",
            "IDENTITY_CHANGED" => "identity.changed",
            _ => "runtime.operation.failed",
        };
        self.error(request_id, &normalized, message_key, normalized != "PROFILE_NOT_READY")
    }
}

#[unsafe(no_mangle)]
/// # Safety
/// `handle` must be a valid handle returned by `torca_runtime_acquire` and
/// must not be used after this call.
pub unsafe extern "C" fn torca_runtime_release(handle: *mut TorcaRuntimeHandle) {
    if !handle.is_null() {
        unsafe {
            drop(Box::from_raw(handle));
        }
    }
}

#[unsafe(no_mangle)]
/// # Safety
/// `handle` must be valid and `request` must reference `request_length` readable bytes.
pub unsafe extern "C" fn torca_runtime_invoke(
    handle: *mut TorcaRuntimeHandle,
    request: *const u8,
    request_length: usize,
    timeout_ms: u32,
) -> i32 {
    let Some(handle) = (unsafe { handle.as_ref() }) else {
        return -1;
    };
    if request.is_null() || request_length == 0 {
        return -1;
    }
    let Ok(request) = str::from_utf8(unsafe { slice::from_raw_parts(request, request_length) })
    else {
        return -1;
    };
    if handle.inner.startup_error.is_some() {
        let request_id = serde_json::from_str::<Value>(request)
            .ok()
            .and_then(|value| value.get("requestId").and_then(Value::as_str).map(str::to_owned))
            .unwrap_or_default();
        let response = serde_json::to_vec(&json!({
            "schema": 1,
            "requestId": request_id,
            "status": "failed",
            "resultKind": "error",
            "runtimeId": "runtime-unavailable",
            "revision": 0,
            "snapshot": Value::Null,
            "error": {
                "code": "RUNTIME_STARTUP_FAILED",
                "category": "runtime",
                "severity": "error",
                "retryable": true,
                "messageKey": "runtime.startup.failed",
                "diagnosticId": secure_id_hex().unwrap_or_default()
            },
            "timing": { "queuedMs": 0, "executionMs": 0 }
        }))
        .unwrap_or_else(|_| b"{\"status\":\"failed\"}".to_vec());
        if let Ok(mut target) = handle.response.lock() {
            *target = response;
            return ABI_OK;
        }
        return -1;
    }
    let (tx, rx) = mpsc::sync_channel(1);
    if send_with_timeout(
        &handle.inner.sender,
        ActorMessage::Invoke { request: request.to_owned(), response: tx },
        Duration::from_secs(2),
    )
    .is_err()
    {
        return -2;
    }
    let timeout = if timeout_ms == 0 {
        DEFAULT_QUERY_TIMEOUT
    } else {
        Duration::from_millis(u64::from(timeout_ms))
    };
    let Ok(response) = rx.recv_timeout(timeout) else {
        return -2;
    };
    let Ok(mut target) = handle.response.lock() else {
        return -1;
    };
    *target = response;
    ABI_OK
}

#[unsafe(no_mangle)]
/// # Safety
/// `handle` must be a valid runtime handle and the returned pointer is valid
/// until the next invocation on that handle.
pub unsafe extern "C" fn torca_runtime_response_ptr(
    handle: *const TorcaRuntimeHandle,
) -> *const u8 {
    let Some(handle) = (unsafe { handle.as_ref() }) else {
        return ptr::null();
    };
    handle.response.lock().map_or(ptr::null(), |value| value.as_ptr())
}

#[unsafe(no_mangle)]
/// # Safety
/// `handle` must be a valid runtime handle.
pub unsafe extern "C" fn torca_runtime_response_len(handle: *const TorcaRuntimeHandle) -> usize {
    let Some(handle) = (unsafe { handle.as_ref() }) else {
        return 0;
    };
    handle.response.lock().map_or(0, |value| value.len())
}

#[unsafe(no_mangle)]
pub extern "C" fn torca_runtime_shutdown(timeout_ms: u32) -> i32 {
    let registry = REGISTRY.get_or_init(|| Mutex::new(None));
    let inner = match registry.lock() {
        Ok(mut value) => value.take(),
        Err(_) => return -1,
    };
    let Some(inner) = inner else {
        return ABI_OK;
    };
    if inner.startup_error.is_some() {
        return ABI_OK;
    }
    let (tx, rx) = mpsc::sync_channel(1);
    if send_with_timeout(
        &inner.sender,
        ActorMessage::Shutdown { response: tx },
        Duration::from_secs(2),
    )
    .is_err()
    {
        return -2;
    }
    let timeout = if timeout_ms == 0 {
        SHUTDOWN_TIMEOUT
    } else {
        Duration::from_millis(u64::from(timeout_ms))
    };
    if rx.recv_timeout(timeout).is_ok() { ABI_OK } else { -2 }
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_torca_host_NativeRuntimeBridge_nativeEnsureRuntime(
    _env: *mut jni::sys::JNIEnv,
    _class: jni::sys::jclass,
) -> jni::sys::jboolean {
    let handle = torca_runtime_acquire();
    if handle.is_null() {
        0
    } else {
        let available =
            unsafe { handle.as_ref().is_some_and(|value| value.inner.startup_error.is_none()) };
        unsafe {
            torca_runtime_release(handle);
        }
        u8::from(available)
    }
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_torca_host_NativeRuntimeBridge_nativeRuntimeAvailable(
    _env: *mut jni::sys::JNIEnv,
    _class: jni::sys::jclass,
) -> jni::sys::jboolean {
    let registry = REGISTRY.get_or_init(|| Mutex::new(None));
    registry.lock().map_or(0, |guard| {
        u8::from(guard.as_ref().is_some_and(|value| value.startup_error.is_none()))
    })
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_torca_host_NativeRuntimeBridge_nativeLifecycleEvent(
    env: *mut jni::sys::JNIEnv,
    _class: jni::sys::jclass,
    event: jni::sys::jstring,
) -> jni::sys::jboolean {
    let Ok(mut env) = (unsafe { jni::JNIEnv::from_raw(env) }) else { return 0 };
    let event = unsafe { jni::objects::JString::from_raw(event) };
    let Ok(event) = env.get_string(&event) else { return 0 };
    u8::from(dispatch_lifecycle(event.to_string_lossy().as_ref()) == ABI_OK)
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_torca_host_NativeRuntimeBridge_nativeNotificationSnapshotJson(
    env: *mut jni::sys::JNIEnv,
    _class: jni::sys::jclass,
    after_cursor: jni::sys::jlong,
) -> jni::sys::jstring {
    let payload = notification_snapshot_json(after_cursor.max(0) as u64);
    let Ok(env) = (unsafe { jni::JNIEnv::from_raw(env) }) else {
        return core::ptr::null_mut();
    };
    env.new_string(payload).map_or(core::ptr::null_mut(), |value| value.into_raw())
}

fn send_with_timeout<T>(
    sender: &SyncSender<T>,
    mut message: T,
    timeout: Duration,
) -> Result<(), ()> {
    let deadline = Instant::now() + timeout;
    loop {
        match sender.try_send(message) {
            Ok(()) => return Ok(()),
            Err(TrySendError::Disconnected(_)) => return Err(()),
            Err(TrySendError::Full(returned)) => {
                if Instant::now() >= deadline {
                    return Err(());
                }
                message = returned;
                thread::yield_now();
            }
        }
    }
}

fn bridge_command(
    name: &str,
    payload: &Value,
) -> Result<BridgeCommand, (&'static str, &'static str)> {
    let text = |field: &str| {
        payload
            .get(field)
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .ok_or(("CONTRACT_PAYLOAD_INVALID", "contract.payload.invalid"))
    };
    let generated = || secure_id_hex().map_err(|_| ("RUNTIME_ID_FAILED", "runtime.id.failed"));
    let now = || now_ms().map_err(|_| ("CLOCK_UNAVAILABLE", "runtime.clock.unavailable"));
    match name {
        "notifications.set" => Ok(BridgeCommand::SetNotifications {
            enabled: payload
                .get("enabled")
                .and_then(Value::as_bool)
                .ok_or(("CONTRACT_PAYLOAD_INVALID", "contract.payload.invalid"))?,
        }),
        "privacy.read_receipts.set" => Ok(BridgeCommand::SetReadReceipts {
            enabled: payload
                .get("enabled")
                .and_then(Value::as_bool)
                .ok_or(("CONTRACT_PAYLOAD_INVALID", "contract.payload.invalid"))?,
        }),
        "contacts.acknowledge_new" => Ok(BridgeCommand::AcknowledgeNewContacts),
        "profile.set" => {
            Ok(BridgeCommand::UpdateProfile { display_name: text("displayName")?, at_ms: now()? })
        }
        "pairing.create" => Ok(BridgeCommand::CreatePairing { session_id_hex: generated()? }),
        "pairing.join" => Ok(BridgeCommand::JoinPairing {
            session_id_hex: generated()?,
            code: text("code")?,
            ticket: payload.get("ticket").and_then(Value::as_str).map(str::to_owned),
        }),
        "pairing.approve" => {
            Ok(BridgeCommand::ApprovePairing { session_id_hex: text("sessionIdHex")? })
        }
        "pairing.reject" => {
            Ok(BridgeCommand::RejectPairing { session_id_hex: text("sessionIdHex")? })
        }
        "pairing.cancel" => {
            Ok(BridgeCommand::CancelPairing { session_id_hex: text("sessionIdHex")? })
        }
        "contact.rename" => Ok(BridgeCommand::RenameContact {
            contact_id_hex: text("contactIdHex")?,
            display_name: text("displayName")?,
        }),
        "contact.verify" => {
            Ok(BridgeCommand::VerifyContact { contact_id_hex: text("contactIdHex")? })
        }
        "contact.verification.reset" => {
            Ok(BridgeCommand::ResetContactVerification { contact_id_hex: text("contactIdHex")? })
        }
        "contact.block" => {
            Ok(BridgeCommand::BlockContact { contact_id_hex: text("contactIdHex")? })
        }
        "contact.unblock" => {
            Ok(BridgeCommand::UnblockContact { contact_id_hex: text("contactIdHex")? })
        }
        "contact.remove" => {
            Ok(BridgeCommand::RemoveContact { contact_id_hex: text("contactIdHex")? })
        }
        "conversation.start" => {
            Ok(BridgeCommand::StartConversation { contact_id_hex: text("contactIdHex")? })
        }
        "conversation.clear" => Ok(BridgeCommand::ClearConversationHistory {
            conversation_id_hex: text("conversationIdHex")?,
        }),
        "message.send" => Ok(BridgeCommand::QueueMessage {
            message_id_hex: generated()?,
            conversation_id_hex: text("conversationIdHex")?,
            body: text("body")?,
            reply_to_message_id_hex: payload
                .get("replyToMessageIdHex")
                .and_then(Value::as_str)
                .filter(|v| !v.is_empty())
                .map(str::to_owned),
            at_ms: now()?,
        }),
        "message.retry" => {
            Ok(BridgeCommand::RetryMessage { message_id_hex: text("messageIdHex")?, at_ms: now()? })
        }
        "conversation.read" => {
            let conversation_id_hex = text("conversationIdHex")?;
            Ok(BridgeCommand::MarkConversationRead { conversation_id_hex })
        }
        "attachment.queue" => Ok(BridgeCommand::QueueAttachment {
            attachment_id_hex: generated()?,
            message_id_hex: generated()?,
            conversation_id_hex: text("conversationIdHex")?,
            source_path: text("sourcePath")?,
            name: text("name")?,
            media_type: text("mediaType")?,
            size: payload
                .get("size")
                .and_then(Value::as_u64)
                .ok_or(("CONTRACT_PAYLOAD_INVALID", "contract.payload.invalid"))?,
            at_ms: now()?,
        }),
        "attachment.retry" => {
            Ok(BridgeCommand::RetryAttachment { attachment_id_hex: text("attachmentIdHex")? })
        }
        "attachment.cancel" => {
            Ok(BridgeCommand::CancelAttachment { attachment_id_hex: text("attachmentIdHex")? })
        }
        "attachment.export" => Ok(BridgeCommand::ExportAttachment {
            attachment_id_hex: text("attachmentIdHex")?,
            destination_path: text("destinationPath")?,
        }),
        _ => Err(("CONTRACT_OPERATION_UNKNOWN", "contract.operation.unknown")),
    }
}

pub(crate) fn secure_id_hex() -> Result<String, ()> {
    let mut provider = RustCryptoProvider;
    let mut bytes = [0_u8; 16];
    provider.fill_random(&mut bytes).map_err(|_| ())?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn now_ms() -> Result<i64, ()> {
    let value = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| ())?.as_millis();
    i64::try_from(value).map_err(|_| ())
}

fn is_idempotent_command(kind: &str) -> bool {
    kind == "command"
}

fn operation_counts_for_revision(kind: &str, name: &str) -> bool {
    kind == "command" || kind == "lifecycle" || name == "snapshot.get"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_has_compatibility_fields() {
        let value: Value = serde_json::from_slice(metadata()).expect("valid metadata");
        assert_eq!(value["nativeAbi"], NATIVE_ABI);
        assert_eq!(value["storageEpoch"], STORAGE_EPOCH);
        assert_eq!(value["contractSchema"], CONTRACT_VERSION);
        assert_eq!(
            value["capabilities"]["maxAttachmentBytes"],
            torca_attachments::MAX_ATTACHMENT_BYTES
        );
        assert_eq!(value["capabilities"]["maxQueuedAttachments"], 5);
        assert!(value["buildId"].is_string());
        assert!(value["sourceFingerprint"].is_string());
    }

    #[test]
    fn notification_snapshot_carries_the_process_runtime_identifier() {
        let response = br#"{
            "runtimeId":"runtime-a",
            "snapshot":{"afterCursor":4,"events":[]}
        }"#;
        let snapshot = extract_notification_snapshot(response).expect("notification snapshot");
        assert_eq!(snapshot["runtimeId"], "runtime-a");
        assert_eq!(snapshot["afterCursor"], 4);
    }

    #[test]
    fn command_ledger_is_bounded_and_expires_entries() {
        let now = Instant::now();
        let mut ledger = IdempotencyLedger::with_limits(2, Duration::from_secs(10));
        ledger.insert("a".into(), b"a".to_vec(), now);
        ledger.insert("b".into(), b"b".to_vec(), now);
        ledger.insert("c".into(), b"c".to_vec(), now);
        assert!(ledger.get("a", now).is_none());
        assert_eq!(ledger.get("b", now), Some(b"b".to_vec()));
        assert_eq!(ledger.get("c", now), Some(b"c".to_vec()));
        assert!(ledger.get("b", now + Duration::from_secs(11)).is_none());
    }

    #[test]
    fn query_request_ids_are_not_command_ledger_entries() {
        let now = Instant::now();
        let mut ledger = IdempotencyLedger::default();
        assert!(is_idempotent_command("command"));
        assert!(!is_idempotent_command("query"));
        assert!(!is_idempotent_command("lifecycle"));
        // Query paths intentionally never call insert, even when they reuse a
        // transport request id. A later poll must execute against new state.
        assert!(ledger.get("notifications-poll-10", now).is_none());
        ledger.insert("command-1".into(), b"command".to_vec(), now);
        assert!(ledger.get("notifications-poll-10", now).is_none());
    }

    #[test]
    fn queries_do_not_count_as_revision_transitions() {
        // snapshot.get may publish a bootstrap/transport transition observed
        // while refreshing the projection, but a stable snapshot does not.
        assert!(operation_counts_for_revision("query", "snapshot.get"));
        assert!(!operation_counts_for_revision("query", "notifications.poll"));
        assert!(!operation_counts_for_revision("query", "conversation.page"));
        assert!(operation_counts_for_revision("command", "profile.set"));
        assert!(operation_counts_for_revision("lifecycle", "foregrounded"));
    }
}
