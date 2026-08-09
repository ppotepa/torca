use core::{ptr, slice, str};
use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use torca_contract::{BridgeCommand, CONTRACT_VERSION};
use torca_crypto::{CryptoProvider, RustCryptoProvider};

use crate::native_runtime::{ABI_OK, TorcaRuntime};

const NATIVE_ABI: u16 = 1;
const STORAGE_EPOCH: u16 = 2;
const MAILBOX_CAPACITY: usize = 256;
const DEFAULT_QUERY_TIMEOUT: Duration = Duration::from_secs(5);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(15);
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

static REGISTRY: OnceLock<Mutex<Option<Arc<RuntimeHandleInner>>>> = OnceLock::new();
static METADATA: OnceLock<Vec<u8>> = OnceLock::new();
static INITIALIZATION_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

enum ActorMessage {
    Invoke { request: String, response: SyncSender<Vec<u8>> },
    Shutdown { response: SyncSender<()> },
}

struct RuntimeHandleInner {
    sender: SyncSender<ActorMessage>,
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
    completed: HashMap<String, Vec<u8>>,
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
    let request = serde_json::json!({
        "schema": 1,
        "requestId": format!("android-notifications-{after_cursor}"),
        "kind": "query",
        "name": "notifications.poll",
        "payload": { "afterCursor": after_cursor },
    })
    .to_string();
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
    serde_json::from_slice::<Value>(&response)
        .ok()
        .and_then(|value| value.get("snapshot").cloned())
        .map_or_else(
            || crate::notification_json::notification_events_json(after_cursor),
            |value| value.to_string(),
        )
}

#[unsafe(no_mangle)]
pub extern "C" fn torca_alloc(length: usize) -> *mut u8 {
    if length == 0 {
        return ptr::null_mut();
    }
    Box::into_raw(vec![0_u8; length].into_boxed_slice()).cast::<u8>()
}

#[unsafe(no_mangle)]
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
    let (ready_tx, ready_rx) = mpsc::sync_channel(1);
    thread::Builder::new()
        .name("torca-runtime".into())
        .spawn(move || match TorcaRuntime::new() {
            Ok(runtime) => {
                let runtime_id = secure_id_hex().unwrap_or_else(|_| "runtime-unavailable".into());
                let _ = ready_tx.send(true);
                actor_loop(
                    receiver,
                    ActorState { runtime, runtime_id, revision: 1, completed: HashMap::new() },
                );
            }
            Err(error) => {
                eprintln!("Torca runtime startup failed: {error}");
                let _ = ready_tx.send(false);
            }
        })
        .map_err(|_| ())?;
    if ready_rx.recv_timeout(Duration::from_secs(10)).ok() != Some(true) {
        return Err(());
    }
    Ok(Arc::new(RuntimeHandleInner { sender }))
}

fn actor_loop(receiver: Receiver<ActorMessage>, mut state: ActorState) {
    loop {
        let message = match receiver.recv_timeout(Duration::from_secs(1)) {
            Ok(message) => message,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        };
        match message {
            ActorMessage::Invoke { request, response } => {
                let _ = response.send(state.invoke(&request));
            }
            ActorMessage::Shutdown { response } => {
                let _ = state.runtime.close();
                let _ = response.send(());
                break;
            }
        }
    }
}

impl ActorState {
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
        if let Some(response) = self.completed.get(request_id) {
            return response.clone();
        }
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
        let code = match (kind, name) {
            ("query", "snapshot.get") => self.runtime.refresh_snapshot(),
            ("query", "conversation.page") => {
                let conversation =
                    payload.get("conversationId").and_then(Value::as_str).unwrap_or_default();
                let before =
                    payload.get("beforeMessageId").and_then(Value::as_str).unwrap_or_default();
                let limit =
                    payload.get("limit").and_then(Value::as_u64).unwrap_or(100).clamp(1, 200)
                        as u32;
                let cursor = if before.is_empty() { None } else { Some(before) };
                self.runtime.conversation_page(conversation, None, cursor, limit as usize)
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
            ("query", "pairing.parse") => {
                let uri = payload.get("uri").and_then(Value::as_str).unwrap_or_default();
                self.runtime.parse_pairing_uri(uri)
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
        self.revision = self.revision.saturating_add(1);
        let mut snapshot: Value = if name == "conversation.page"
            || name == "conversation.search"
            || name == "notifications.poll"
            || name == "pairing.parse"
        {
            serde_json::from_str(&self.runtime.query_json).unwrap_or(Value::Null)
        } else {
            serde_json::from_str(&self.runtime.snapshot_json).unwrap_or(Value::Null)
        };
        if name != "conversation.page"
            && name != "conversation.search"
            && name != "notifications.poll"
        {
            if let Value::Object(object) = &mut snapshot {
                object.insert("runtimeId".into(), Value::String(self.runtime_id.clone()));
                object.insert("revision".into(), Value::from(self.revision));
                object.insert(
                    "notificationCursor".into(),
                    Value::from(self.runtime.notification_cursor),
                );
            }
        }
        let response = serde_json::to_vec(&json!({
            "schema": 1, "requestId": request_id, "status": "succeeded",
            "resultKind": if name == "profile.set" { "profile_updated" } else { "snapshot" },
            "runtimeId": self.runtime_id, "revision": self.revision, "snapshot": snapshot,
            "error": Value::Null, "timing": { "queuedMs": 0, "executionMs": started.elapsed().as_millis() }
        })).expect("runtime response is serializable");
        if !request_id.is_empty() {
            self.completed.insert(request_id.to_owned(), response.clone());
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
pub unsafe extern "C" fn torca_runtime_release(handle: *mut TorcaRuntimeHandle) {
    if !handle.is_null() {
        unsafe {
            drop(Box::from_raw(handle));
        }
    }
}

#[unsafe(no_mangle)]
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
pub unsafe extern "C" fn torca_runtime_response_ptr(
    handle: *const TorcaRuntimeHandle,
) -> *const u8 {
    let Some(handle) = (unsafe { handle.as_ref() }) else {
        return ptr::null();
    };
    handle.response.lock().map_or(ptr::null(), |value| value.as_ptr())
}

#[unsafe(no_mangle)]
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
        unsafe {
            torca_runtime_release(handle);
        }
        1
    }
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_torca_host_NativeRuntimeBridge_nativeRuntimeAvailable(
    _env: *mut jni::sys::JNIEnv,
    _class: jni::sys::jclass,
) -> jni::sys::jboolean {
    let registry = REGISTRY.get_or_init(|| Mutex::new(None));
    registry.lock().map_or(0, |guard| u8::from(guard.is_some()))
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
        "profile.set" => {
            Ok(BridgeCommand::UpdateProfile { display_name: text("displayName")?, at_ms: now()? })
        }
        "pairing.create" => Ok(BridgeCommand::CreatePairing { session_id_hex: generated()? }),
        "pairing.join" => {
            Ok(BridgeCommand::JoinPairing { session_id_hex: generated()?, code: text("code")? })
        }
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
            if payload.get("sendReceipt").and_then(Value::as_bool).unwrap_or(true) {
                Ok(BridgeCommand::MarkConversationReadWithPolicy {
                    conversation_id_hex,
                    send_receipt: true,
                })
            } else {
                Ok(BridgeCommand::MarkConversationRead { conversation_id_hex })
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_has_compatibility_fields() {
        let value: Value = serde_json::from_slice(metadata()).expect("valid metadata");
        assert_eq!(value["nativeAbi"], NATIVE_ABI);
        assert_eq!(value["storageEpoch"], STORAGE_EPOCH);
        assert_eq!(value["contractSchema"], CONTRACT_VERSION);
        assert!(value["buildId"].is_string());
        assert!(value["sourceFingerprint"].is_string());
    }
}
