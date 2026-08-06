//! Narrow C ABI owned by the shared Torca client runtime.
//!
//! The ABI intentionally exposes application commands and presentation snapshots rather than
//! domain objects. Flutter owns no workflow state; it only sends commands and renders snapshots.

mod composition;

use core::fmt::Write as _;
use core::{ptr, slice, str};
use torca_bridge::{
    BridgeCommand, BridgeResult, BridgeSnapshot, EngineBridge, CONTRACT_VERSION,
};
use torca_client_engine::ClientEngineActor;

use composition::spawn_production_engine;

const ABI_OK: i32 = 0;
const ABI_ERROR: i32 = -1;
const ABI_CLOSED: i32 = -2;

/// Native engine instance owned by exactly one Flutter application process.
pub struct NativeEngineRuntime {
    bridge: EngineBridge,
    actor: Option<ClientEngineActor>,
    last_result_json: String,
    snapshot_json: String,
}

impl NativeEngineRuntime {
    fn new() -> Result<Self, ()> {
        let (handle, actor) = spawn_production_engine().map_err(|_| ())?;
        let mut runtime = Self {
            bridge: EngineBridge::new(handle),
            actor: Some(actor),
            last_result_json: success_result("initialized"),
            snapshot_json: empty_snapshot_json(),
        };
        if runtime.refresh_snapshot() != ABI_OK {
            return Err(());
        }
        Ok(runtime)
    }

    fn is_closed(&self) -> bool {
        self.actor.is_none()
    }

    fn execute(&mut self, command: BridgeCommand) -> i32 {
        if self.is_closed() {
            self.last_result_json = error_result("native engine is closed");
            return ABI_CLOSED;
        }

        let result = self.bridge.execute(command);
        let ok = result.ok;
        self.last_result_json = bridge_result_json(&result);
        if ok {
            let _ = self.refresh_snapshot();
            ABI_OK
        } else {
            ABI_ERROR
        }
    }

    fn refresh_snapshot(&mut self) -> i32 {
        if self.is_closed() {
            return ABI_CLOSED;
        }
        match self.bridge.snapshot() {
            Ok(snapshot) => {
                self.snapshot_json = bridge_snapshot_json(&snapshot);
                ABI_OK
            }
            Err(error) => {
                self.last_result_json = error_result(&error.to_string());
                ABI_ERROR
            }
        }
    }

    fn close(&mut self) -> i32 {
        let Some(actor) = self.actor.take() else {
            return ABI_OK;
        };
        match actor.shutdown() {
            Ok(()) => ABI_OK,
            Err(error) => {
                self.last_result_json = error_result(&error.to_string());
                ABI_ERROR
            }
        }
    }
}

impl Drop for NativeEngineRuntime {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

/// Returns the cross-language contract version implemented by this library.
#[unsafe(no_mangle)]
pub extern "C" fn torca_contract_version() -> u16 {
    CONTRACT_VERSION
}

/// Allocates a zeroed byte buffer for a caller-owned UTF-8 argument.
#[unsafe(no_mangle)]
pub extern "C" fn torca_alloc(length: usize) -> *mut u8 {
    if length == 0 {
        return ptr::null_mut();
    }
    let bytes = vec![0_u8; length].into_boxed_slice();
    Box::into_raw(bytes).cast::<u8>()
}

/// Releases a byte buffer previously returned by [`torca_alloc`].
///
/// # Safety
///
/// `data` must be a pointer returned by `torca_alloc(length)` that has not already been freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_free(data: *mut u8, length: usize) {
    if data.is_null() || length == 0 {
        return;
    }
    let raw_slice = ptr::slice_from_raw_parts_mut(data, length);
    unsafe {
        drop(Box::from_raw(raw_slice));
    }
}

/// Creates one shared native engine runtime.
///
/// Returns null when secure production composition cannot be created. There is deliberately no
/// fallback to the in-memory engine on a production FFI path.
#[unsafe(no_mangle)]
pub extern "C" fn torca_engine_new() -> *mut NativeEngineRuntime {
    match NativeEngineRuntime::new() {
        Ok(runtime) => Box::into_raw(Box::new(runtime)),
        Err(()) => ptr::null_mut(),
    }
}

/// Stops and releases one shared native engine runtime.
///
/// # Safety
///
/// `handle` must be a live pointer returned by [`torca_engine_new`] and must be destroyed once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_destroy(handle: *mut NativeEngineRuntime) {
    if handle.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(handle));
    }
}

/// Creates the local identity through the Rust application engine.
///
/// # Safety
///
/// Every non-empty pointer/length pair must refer to readable UTF-8 bytes for the duration of the
/// call. `handle` must be a live engine pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_create_identity(
    handle: *mut NativeEngineRuntime,
    identity_id: *const u8,
    identity_id_length: usize,
    display_name: *const u8,
    display_name_length: usize,
    at_ms: i64,
) -> i32 {
    let Some(runtime) = (unsafe { handle.as_mut() }) else {
        return ABI_ERROR;
    };
    let identity_id = match unsafe { utf8_argument(identity_id, identity_id_length) } {
        Ok(value) => value,
        Err(error) => return runtime.reject_argument(error),
    };
    let display_name = match unsafe { utf8_argument(display_name, display_name_length) } {
        Ok(value) => value,
        Err(error) => return runtime.reject_argument(error),
    };
    runtime.execute(BridgeCommand::CreateIdentity { identity_id_hex: identity_id, display_name, at_ms })
}

/// Starts a pairing session through the Rust application engine.
///
/// # Safety
///
/// Every pointer/length pair must refer to readable UTF-8 bytes for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_start_pairing(
    handle: *mut NativeEngineRuntime,
    session_id: *const u8,
    session_id_length: usize,
    code: *const u8,
    code_length: usize,
    expires_at_ms: i64,
) -> i32 {
    let Some(runtime) = (unsafe { handle.as_mut() }) else {
        return ABI_ERROR;
    };
    let session_id = match unsafe { utf8_argument(session_id, session_id_length) } {
        Ok(value) => value,
        Err(error) => return runtime.reject_argument(error),
    };
    let code = match unsafe { utf8_argument(code, code_length) } {
        Ok(value) => value,
        Err(error) => return runtime.reject_argument(error),
    };
    runtime.execute(BridgeCommand::StartPairing {
        session_id_hex: session_id,
        code,
        expires_at_ms,
    })
}

/// Queues one outbound text message through the Rust application engine.
///
/// # Safety
///
/// Every pointer/length pair must refer to readable UTF-8 bytes for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_queue_message(
    handle: *mut NativeEngineRuntime,
    message_id: *const u8,
    message_id_length: usize,
    conversation_id: *const u8,
    conversation_id_length: usize,
    body: *const u8,
    body_length: usize,
    at_ms: i64,
) -> i32 {
    let Some(runtime) = (unsafe { handle.as_mut() }) else {
        return ABI_ERROR;
    };
    let message_id = match unsafe { utf8_argument(message_id, message_id_length) } {
        Ok(value) => value,
        Err(error) => return runtime.reject_argument(error),
    };
    let conversation_id = match unsafe { utf8_argument(conversation_id, conversation_id_length) } {
        Ok(value) => value,
        Err(error) => return runtime.reject_argument(error),
    };
    let body = match unsafe { utf8_argument(body, body_length) } {
        Ok(value) => value,
        Err(error) => return runtime.reject_argument(error),
    };
    runtime.execute(BridgeCommand::QueueMessage {
        message_id_hex: message_id,
        conversation_id_hex: conversation_id,
        body,
        at_ms,
    })
}

/// Refreshes the cached presentation snapshot.
///
/// # Safety
///
/// `handle` must be a live engine pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_refresh_snapshot(handle: *mut NativeEngineRuntime) -> i32 {
    let Some(runtime) = (unsafe { handle.as_mut() }) else {
        return ABI_ERROR;
    };
    runtime.refresh_snapshot()
}

/// Returns the pointer to the last command-result JSON bytes.
///
/// The pointer remains valid until the next mutable engine call.
///
/// # Safety
///
/// `handle` must be a live engine pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_result_ptr(
    handle: *const NativeEngineRuntime,
) -> *const u8 {
    let Some(runtime) = (unsafe { handle.as_ref() }) else {
        return ptr::null();
    };
    runtime.last_result_json.as_ptr()
}

/// Returns the byte length of the last command-result JSON.
///
/// # Safety
///
/// `handle` must be a live engine pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_result_len(handle: *const NativeEngineRuntime) -> usize {
    unsafe { handle.as_ref() }.map_or(0, |runtime| runtime.last_result_json.len())
}

/// Returns the pointer to the cached snapshot JSON bytes.
///
/// The pointer remains valid until the next snapshot refresh or successful command.
///
/// # Safety
///
/// `handle` must be a live engine pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_snapshot_ptr(
    handle: *const NativeEngineRuntime,
) -> *const u8 {
    let Some(runtime) = (unsafe { handle.as_ref() }) else {
        return ptr::null();
    };
    runtime.snapshot_json.as_ptr()
}

/// Returns the byte length of the cached snapshot JSON.
///
/// # Safety
///
/// `handle` must be a live engine pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_snapshot_len(handle: *const NativeEngineRuntime) -> usize {
    unsafe { handle.as_ref() }.map_or(0, |runtime| runtime.snapshot_json.len())
}

/// Flushes and stops the engine actor without releasing the outer handle.
///
/// # Safety
///
/// `handle` must be a live engine pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_close(handle: *mut NativeEngineRuntime) -> i32 {
    let Some(runtime) = (unsafe { handle.as_mut() }) else {
        return ABI_ERROR;
    };
    runtime.close()
}

impl NativeEngineRuntime {
    fn reject_argument(&mut self, error: &'static str) -> i32 {
        self.last_result_json = error_result(error);
        ABI_ERROR
    }
}

unsafe fn utf8_argument(data: *const u8, length: usize) -> Result<String, &'static str> {
    if length == 0 {
        return Ok(String::new());
    }
    if data.is_null() {
        return Err("native argument pointer is null");
    }
    let bytes = unsafe { slice::from_raw_parts(data, length) };
    str::from_utf8(bytes)
        .map(str::to_owned)
        .map_err(|_| "native argument is not valid UTF-8")
}

fn success_result(kind: &str) -> String {
    bridge_result_json(&BridgeResult { ok: true, kind: kind.to_owned(), error: None })
}

fn error_result(error: &str) -> String {
    bridge_result_json(&BridgeResult {
        ok: false,
        kind: "error".into(),
        error: Some(error.to_owned()),
    })
}

fn bridge_result_json(result: &BridgeResult) -> String {
    let mut output = String::from("{\"ok\":");
    output.push_str(if result.ok { "true" } else { "false" });
    output.push_str(",\"kind\":\"");
    push_json_string(&result.kind, &mut output);
    output.push_str("\",\"error\":");
    match &result.error {
        Some(error) => {
            output.push('"');
            push_json_string(error, &mut output);
            output.push('"');
        }
        None => output.push_str("null"),
    }
    output.push('}');
    output
}

fn empty_snapshot_json() -> String {
    format!(
        "{{\"contractVersion\":{CONTRACT_VERSION},\"identity\":null,\"contacts\":[],\"conversations\":[],\"messages\":[]}}"
    )
}

fn bridge_snapshot_json(snapshot: &BridgeSnapshot) -> String {
    let mut output = String::new();
    let _ = write!(output, "{{\"contractVersion\":{}", snapshot.contract_version);
    output.push_str(",\"identity\":");
    match &snapshot.identity_name {
        Some(name) => {
            output.push_str("{\"displayName\":\"");
            push_json_string(name, &mut output);
            output.push_str("\"}");
        }
        None => output.push_str("null"),
    }

    output.push_str(",\"contacts\":[");
    for (index, contact) in snapshot.contacts.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        output.push_str("{\"id\":\"");
        push_json_string(&contact.id, &mut output);
        output.push_str("\",\"onionAddress\":\"");
        push_json_string(&contact.onion_address, &mut output);
        output.push_str("\",\"status\":\"");
        push_json_string(&contact.status, &mut output);
        output.push_str("\"}");
    }
    output.push(']');

    output.push_str(",\"conversations\":[");
    for (index, conversation) in snapshot.conversations.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        output.push_str("{\"id\":\"");
        push_json_string(&conversation.id, &mut output);
        output.push_str("\",\"contactId\":\"");
        push_json_string(&conversation.contact_id, &mut output);
        output.push_str("\",\"status\":\"");
        push_json_string(&conversation.status, &mut output);
        output.push_str("\"}");
    }
    output.push(']');

    output.push_str(",\"messages\":[");
    for (index, message) in snapshot.messages.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        output.push_str("{\"id\":\"");
        push_json_string(&message.id, &mut output);
        output.push_str("\",\"conversationId\":\"");
        push_json_string(&message.conversation_id, &mut output);
        output.push_str("\",\"body\":\"");
        push_json_string(&message.body, &mut output);
        output.push_str("\",\"direction\":\"");
        push_json_string(&message.direction, &mut output);
        output.push_str("\",\"status\":\"");
        push_json_string(&message.status, &mut output);
        output.push_str("\"}");
    }
    output.push_str("]}");
    output
}

fn push_json_string(value: &str, output: &mut String) {
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character.is_control() => {
                let _ = write!(output, "\\u{:04x}", character as u32);
            }
            character => output.push(character),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::empty_snapshot_json;

    #[test]
    fn empty_snapshot_json_is_parseable_shape_without_secret_material() {
        let json = empty_snapshot_json();
        assert!(json.contains("\"contractVersion\":1"));
        assert!(json.contains("\"contacts\":[]"));
        assert!(!json.contains("private_key"));
        assert!(!json.contains("secret="));
    }
}
