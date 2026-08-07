use core::{ptr, slice, str};

use torca_bridge::{BridgeCommand, CONTRACT_VERSION};

use crate::native_runtime::{ABI_ERROR, NativeEngineRuntime};
use crate::process_runtime::{
    NativeEngineHandle, acquire_process_runtime, shutdown_process_runtime,
};

#[unsafe(no_mangle)]
pub extern "C" fn torca_contract_version() -> u16 { CONTRACT_VERSION }

#[unsafe(no_mangle)]
pub extern "C" fn torca_alloc(length: usize) -> *mut u8 {
    if length == 0 { return ptr::null_mut(); }
    Box::into_raw(vec![0_u8; length].into_boxed_slice()).cast::<u8>()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_free(data: *mut u8, length: usize) {
    if data.is_null() || length == 0 { return; }
    let raw_slice = ptr::slice_from_raw_parts_mut(data, length);
    unsafe { drop(Box::from_raw(raw_slice)); }
}

#[unsafe(no_mangle)]
pub extern "C" fn torca_engine_new() -> *mut NativeEngineHandle {
    acquire_process_runtime()
        .map(|handle| Box::into_raw(Box::new(handle)))
        .unwrap_or(ptr::null_mut())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_destroy(handle: *mut NativeEngineHandle) {
    if !handle.is_null() { unsafe { drop(Box::from_raw(handle)); } }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_create_identity(
    handle: *mut NativeEngineHandle,
    identity_id: *const u8,
    identity_id_length: usize,
    display_name: *const u8,
    display_name_length: usize,
    at_ms: i64,
) -> i32 {
    with_runtime_mut(handle, |runtime| {
        let identity_id = match unsafe { utf8_argument(identity_id, identity_id_length) } {
            Ok(value) => value, Err(error) => return runtime.reject_argument(error),
        };
        let display_name = match unsafe { utf8_argument(display_name, display_name_length) } {
            Ok(value) => value, Err(error) => return runtime.reject_argument(error),
        };
        runtime.execute(BridgeCommand::CreateIdentity { identity_id_hex: identity_id, display_name, at_ms })
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_create_pairing(
    handle: *mut NativeEngineHandle,
    session_id: *const u8,
    session_id_length: usize,
) -> i32 {
    with_runtime_mut(handle, |runtime| {
        let session_id = match unsafe { utf8_argument(session_id, session_id_length) } {
            Ok(value) => value, Err(error) => return runtime.reject_argument(error),
        };
        runtime.execute(BridgeCommand::CreatePairing { session_id_hex: session_id })
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_join_pairing(
    handle: *mut NativeEngineHandle,
    session_id: *const u8,
    session_id_length: usize,
    code: *const u8,
    code_length: usize,
) -> i32 {
    with_runtime_mut(handle, |runtime| {
        let session_id = match unsafe { utf8_argument(session_id, session_id_length) } {
            Ok(value) => value, Err(error) => return runtime.reject_argument(error),
        };
        let code = match unsafe { utf8_argument(code, code_length) } {
            Ok(value) => value, Err(error) => return runtime.reject_argument(error),
        };
        runtime.execute(BridgeCommand::JoinPairing { session_id_hex: session_id, code })
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_approve_pairing(
    handle: *mut NativeEngineHandle,
    session_id: *const u8,
    session_id_length: usize,
) -> i32 {
    unsafe { pairing_id_command(handle, session_id, session_id_length, |session_id_hex| {
        BridgeCommand::ApprovePairing { session_id_hex }
    }) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_reject_pairing(
    handle: *mut NativeEngineHandle,
    session_id: *const u8,
    session_id_length: usize,
) -> i32 {
    unsafe { pairing_id_command(handle, session_id, session_id_length, |session_id_hex| {
        BridgeCommand::RejectPairing { session_id_hex }
    }) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_cancel_pairing(
    handle: *mut NativeEngineHandle,
    session_id: *const u8,
    session_id_length: usize,
) -> i32 {
    unsafe { pairing_id_command(handle, session_id, session_id_length, |session_id_hex| {
        BridgeCommand::CancelPairing { session_id_hex }
    }) }
}

unsafe fn pairing_id_command(
    handle: *mut NativeEngineHandle,
    session_id: *const u8,
    session_id_length: usize,
    make: impl FnOnce(String) -> BridgeCommand,
) -> i32 {
    with_runtime_mut(handle, |runtime| {
        let session_id = match unsafe { utf8_argument(session_id, session_id_length) } {
            Ok(value) => value, Err(error) => return runtime.reject_argument(error),
        };
        runtime.execute(make(session_id))
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_queue_message(
    handle: *mut NativeEngineHandle,
    message_id: *const u8,
    message_id_length: usize,
    conversation_id: *const u8,
    conversation_id_length: usize,
    body: *const u8,
    body_length: usize,
    at_ms: i64,
) -> i32 {
    unsafe {
        queue_message_command(
            handle,
            message_id,
            message_id_length,
            conversation_id,
            conversation_id_length,
            body,
            body_length,
            ptr::null(),
            0,
            at_ms,
        )
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_queue_message_reply(
    handle: *mut NativeEngineHandle,
    message_id: *const u8,
    message_id_length: usize,
    conversation_id: *const u8,
    conversation_id_length: usize,
    body: *const u8,
    body_length: usize,
    reply_to_message_id: *const u8,
    reply_to_message_id_length: usize,
    at_ms: i64,
) -> i32 {
    if reply_to_message_id_length == 0 {
        return with_runtime_mut(handle, |runtime| runtime.reject_argument("reply message id is empty"));
    }
    unsafe {
        queue_message_command(
            handle,
            message_id,
            message_id_length,
            conversation_id,
            conversation_id_length,
            body,
            body_length,
            reply_to_message_id,
            reply_to_message_id_length,
            at_ms,
        )
    }
}

unsafe fn queue_message_command(
    handle: *mut NativeEngineHandle,
    message_id: *const u8,
    message_id_length: usize,
    conversation_id: *const u8,
    conversation_id_length: usize,
    body: *const u8,
    body_length: usize,
    reply_to_message_id: *const u8,
    reply_to_message_id_length: usize,
    at_ms: i64,
) -> i32 {
    with_runtime_mut(handle, |runtime| {
        let message_id = match unsafe { utf8_argument(message_id, message_id_length) } {
            Ok(value) => value, Err(error) => return runtime.reject_argument(error),
        };
        let conversation_id = match unsafe { utf8_argument(conversation_id, conversation_id_length) } {
            Ok(value) => value, Err(error) => return runtime.reject_argument(error),
        };
        let body = match unsafe { utf8_argument(body, body_length) } {
            Ok(value) => value, Err(error) => return runtime.reject_argument(error),
        };
        let reply_to_message_id_hex = if reply_to_message_id_length == 0 {
            None
        } else {
            match unsafe { utf8_argument(reply_to_message_id, reply_to_message_id_length) } {
                Ok(value) => Some(value), Err(error) => return runtime.reject_argument(error),
            }
        };
        runtime.execute(BridgeCommand::QueueMessage {
            message_id_hex: message_id,
            conversation_id_hex: conversation_id,
            body,
            reply_to_message_id_hex,
            at_ms,
        })
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_mark_conversation_read(
    handle: *mut NativeEngineHandle,
    conversation_id: *const u8,
    conversation_id_length: usize,
) -> i32 {
    with_runtime_mut(handle, |runtime| {
        let conversation_id = match unsafe { utf8_argument(conversation_id, conversation_id_length) } {
            Ok(value) => value, Err(error) => return runtime.reject_argument(error),
        };
        runtime.execute(BridgeCommand::MarkConversationRead { conversation_id_hex: conversation_id })
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_refresh_snapshot(handle: *mut NativeEngineHandle) -> i32 {
    with_runtime_mut(handle, NativeEngineRuntime::refresh_snapshot)
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_refresh_diagnostics(handle: *mut NativeEngineHandle) -> i32 {
    with_runtime_mut(handle, NativeEngineRuntime::refresh_diagnostics)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_result_ptr(handle: *const NativeEngineHandle) -> *const u8 {
    with_runtime(handle, |runtime| runtime.last_result_json.as_ptr()).unwrap_or(ptr::null())
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_result_len(handle: *const NativeEngineHandle) -> usize {
    with_runtime(handle, |runtime| runtime.last_result_json.len()).unwrap_or(0)
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_snapshot_ptr(handle: *const NativeEngineHandle) -> *const u8 {
    with_runtime(handle, |runtime| runtime.snapshot_json.as_ptr()).unwrap_or(ptr::null())
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_snapshot_len(handle: *const NativeEngineHandle) -> usize {
    with_runtime(handle, |runtime| runtime.snapshot_json.len()).unwrap_or(0)
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_diagnostics_ptr(handle: *const NativeEngineHandle) -> *const u8 {
    with_runtime(handle, |runtime| runtime.diagnostics_json.as_ptr()).unwrap_or(ptr::null())
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_diagnostics_len(handle: *const NativeEngineHandle) -> usize {
    with_runtime(handle, |runtime| runtime.diagnostics_json.len()).unwrap_or(0)
}

/// Explicit process shutdown. Destroying/releasing an individual FFI handle does not stop Torca.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_close(_handle: *mut NativeEngineHandle) -> i32 {
    shutdown_process_runtime()
}

fn with_runtime_mut(
    handle: *mut NativeEngineHandle,
    operation: impl FnOnce(&mut NativeEngineRuntime) -> i32,
) -> i32 {
    let Some(handle) = (unsafe { handle.as_ref() }) else { return ABI_ERROR; };
    match handle.runtime.lock() {
        Ok(mut runtime) => operation(&mut runtime),
        Err(_) => ABI_ERROR,
    }
}
fn with_runtime<T>(
    handle: *const NativeEngineHandle,
    operation: impl FnOnce(&NativeEngineRuntime) -> T,
) -> Option<T> {
    let handle = unsafe { handle.as_ref() }?;
    handle.runtime.lock().ok().map(|runtime| operation(&runtime))
}

unsafe fn utf8_argument(data: *const u8, length: usize) -> Result<String, &'static str> {
    if length == 0 { return Ok(String::new()); }
    if data.is_null() { return Err("native argument pointer is null"); }
    let bytes = unsafe { slice::from_raw_parts(data, length) };
    str::from_utf8(bytes).map(str::to_owned).map_err(|_| "native argument is not valid UTF-8")
}