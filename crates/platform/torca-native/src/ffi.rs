use core::{ptr, slice, str};

use torca_bridge::{BridgeCommand, CONTRACT_VERSION};

use crate::native_runtime::{ABI_ERROR, NativeEngineRuntime};

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
pub extern "C" fn torca_engine_new() -> *mut NativeEngineRuntime {
    NativeEngineRuntime::new()
        .map(|runtime| Box::into_raw(Box::new(runtime)))
        .unwrap_or(ptr::null_mut())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_destroy(handle: *mut NativeEngineRuntime) {
    if !handle.is_null() { unsafe { drop(Box::from_raw(handle)); } }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_create_identity(
    handle: *mut NativeEngineRuntime,
    identity_id: *const u8,
    identity_id_length: usize,
    display_name: *const u8,
    display_name_length: usize,
    at_ms: i64,
) -> i32 {
    let Some(runtime) = (unsafe { handle.as_mut() }) else { return ABI_ERROR; };
    let identity_id = match unsafe { utf8_argument(identity_id, identity_id_length) } {
        Ok(value) => value, Err(error) => return runtime.reject_argument(error),
    };
    let display_name = match unsafe { utf8_argument(display_name, display_name_length) } {
        Ok(value) => value, Err(error) => return runtime.reject_argument(error),
    };
    runtime.execute(BridgeCommand::CreateIdentity { identity_id_hex: identity_id, display_name, at_ms })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_create_pairing(
    handle: *mut NativeEngineRuntime,
    session_id: *const u8,
    session_id_length: usize,
) -> i32 {
    let Some(runtime) = (unsafe { handle.as_mut() }) else { return ABI_ERROR; };
    let session_id = match unsafe { utf8_argument(session_id, session_id_length) } {
        Ok(value) => value, Err(error) => return runtime.reject_argument(error),
    };
    runtime.execute(BridgeCommand::CreatePairing { session_id_hex: session_id })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_join_pairing(
    handle: *mut NativeEngineRuntime,
    session_id: *const u8,
    session_id_length: usize,
    code: *const u8,
    code_length: usize,
) -> i32 {
    let Some(runtime) = (unsafe { handle.as_mut() }) else { return ABI_ERROR; };
    let session_id = match unsafe { utf8_argument(session_id, session_id_length) } {
        Ok(value) => value, Err(error) => return runtime.reject_argument(error),
    };
    let code = match unsafe { utf8_argument(code, code_length) } {
        Ok(value) => value, Err(error) => return runtime.reject_argument(error),
    };
    runtime.execute(BridgeCommand::JoinPairing { session_id_hex: session_id, code })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_approve_pairing(
    handle: *mut NativeEngineRuntime,
    session_id: *const u8,
    session_id_length: usize,
) -> i32 {
    unsafe { pairing_id_command(handle, session_id, session_id_length, |session_id_hex| {
        BridgeCommand::ApprovePairing { session_id_hex }
    }) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_reject_pairing(
    handle: *mut NativeEngineRuntime,
    session_id: *const u8,
    session_id_length: usize,
) -> i32 {
    unsafe { pairing_id_command(handle, session_id, session_id_length, |session_id_hex| {
        BridgeCommand::RejectPairing { session_id_hex }
    }) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_cancel_pairing(
    handle: *mut NativeEngineRuntime,
    session_id: *const u8,
    session_id_length: usize,
) -> i32 {
    unsafe { pairing_id_command(handle, session_id, session_id_length, |session_id_hex| {
        BridgeCommand::CancelPairing { session_id_hex }
    }) }
}

unsafe fn pairing_id_command(
    handle: *mut NativeEngineRuntime,
    session_id: *const u8,
    session_id_length: usize,
    make: impl FnOnce(String) -> BridgeCommand,
) -> i32 {
    let Some(runtime) = (unsafe { handle.as_mut() }) else { return ABI_ERROR; };
    let session_id = match unsafe { utf8_argument(session_id, session_id_length) } {
        Ok(value) => value, Err(error) => return runtime.reject_argument(error),
    };
    runtime.execute(make(session_id))
}

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
    let Some(runtime) = (unsafe { handle.as_mut() }) else { return ABI_ERROR; };
    let message_id = match unsafe { utf8_argument(message_id, message_id_length) } {
        Ok(value) => value, Err(error) => return runtime.reject_argument(error),
    };
    let conversation_id = match unsafe { utf8_argument(conversation_id, conversation_id_length) } {
        Ok(value) => value, Err(error) => return runtime.reject_argument(error),
    };
    let body = match unsafe { utf8_argument(body, body_length) } {
        Ok(value) => value, Err(error) => return runtime.reject_argument(error),
    };
    runtime.execute(BridgeCommand::QueueMessage {
        message_id_hex: message_id, conversation_id_hex: conversation_id, body, at_ms,
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_mark_conversation_read(
    handle: *mut NativeEngineRuntime,
    conversation_id: *const u8,
    conversation_id_length: usize,
) -> i32 {
    let Some(runtime) = (unsafe { handle.as_mut() }) else { return ABI_ERROR; };
    let conversation_id = match unsafe { utf8_argument(conversation_id, conversation_id_length) } {
        Ok(value) => value, Err(error) => return runtime.reject_argument(error),
    };
    runtime.execute(BridgeCommand::MarkConversationRead { conversation_id_hex: conversation_id })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_refresh_snapshot(handle: *mut NativeEngineRuntime) -> i32 {
    unsafe { handle.as_mut() }.map_or(ABI_ERROR, NativeEngineRuntime::refresh_snapshot)
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_refresh_diagnostics(handle: *mut NativeEngineRuntime) -> i32 {
    unsafe { handle.as_mut() }.map_or(ABI_ERROR, NativeEngineRuntime::refresh_diagnostics)
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_result_ptr(handle: *const NativeEngineRuntime) -> *const u8 {
    unsafe { handle.as_ref() }.map_or(ptr::null(), |runtime| runtime.last_result_json.as_ptr())
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_result_len(handle: *const NativeEngineRuntime) -> usize {
    unsafe { handle.as_ref() }.map_or(0, |runtime| runtime.last_result_json.len())
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_snapshot_ptr(handle: *const NativeEngineRuntime) -> *const u8 {
    unsafe { handle.as_ref() }.map_or(ptr::null(), |runtime| runtime.snapshot_json.as_ptr())
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_snapshot_len(handle: *const NativeEngineRuntime) -> usize {
    unsafe { handle.as_ref() }.map_or(0, |runtime| runtime.snapshot_json.len())
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_diagnostics_ptr(handle: *const NativeEngineRuntime) -> *const u8 {
    unsafe { handle.as_ref() }.map_or(ptr::null(), |runtime| runtime.diagnostics_json.as_ptr())
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_diagnostics_len(handle: *const NativeEngineRuntime) -> usize {
    unsafe { handle.as_ref() }.map_or(0, |runtime| runtime.diagnostics_json.len())
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_close(handle: *mut NativeEngineRuntime) -> i32 {
    unsafe { handle.as_mut() }.map_or(ABI_ERROR, NativeEngineRuntime::close)
}

unsafe fn utf8_argument(data: *const u8, length: usize) -> Result<String, &'static str> {
    if length == 0 { return Ok(String::new()); }
    if data.is_null() { return Err("native argument pointer is null"); }
    let bytes = unsafe { slice::from_raw_parts(data, length) };
    str::from_utf8(bytes).map(str::to_owned).map_err(|_| "native argument is not valid UTF-8")
}
