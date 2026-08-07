use torca_bridge::BridgeCommand;

use crate::{ABI_ERROR, NativeEngineRuntime, utf8_argument};

/// Joins a pairing invitation without exposing any remote peer proposal to Flutter.
///
/// # Safety
///
/// Pointer/length pairs must refer to readable UTF-8 bytes for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_join_pairing(
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
    runtime.execute(BridgeCommand::JoinPairing { session_id_hex: session_id, code, expires_at_ms })
}

/// Explicitly approves a verified pairing peer.
///
/// # Safety
///
/// `session_id` must refer to readable UTF-8 bytes for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_approve_pairing(
    handle: *mut NativeEngineRuntime,
    session_id: *const u8,
    session_id_length: usize,
    at_ms: i64,
) -> i32 {
    let Some(runtime) = (unsafe { handle.as_mut() }) else {
        return ABI_ERROR;
    };
    let session_id = match unsafe { utf8_argument(session_id, session_id_length) } {
        Ok(value) => value,
        Err(error) => return runtime.reject_argument(error),
    };
    runtime.execute(BridgeCommand::ApprovePairing { session_id_hex: session_id, at_ms })
}

/// Explicitly rejects the current pairing peer.
///
/// # Safety
///
/// `session_id` must refer to readable UTF-8 bytes for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_reject_pairing(
    handle: *mut NativeEngineRuntime,
    session_id: *const u8,
    session_id_length: usize,
) -> i32 {
    unsafe { pairing_terminal(handle, session_id, session_id_length, true) }
}

/// Cancels a local pairing workflow.
///
/// # Safety
///
/// `session_id` must refer to readable UTF-8 bytes for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_cancel_pairing(
    handle: *mut NativeEngineRuntime,
    session_id: *const u8,
    session_id_length: usize,
) -> i32 {
    unsafe { pairing_terminal(handle, session_id, session_id_length, false) }
}

unsafe fn pairing_terminal(
    handle: *mut NativeEngineRuntime,
    session_id: *const u8,
    session_id_length: usize,
    reject: bool,
) -> i32 {
    let Some(runtime) = (unsafe { handle.as_mut() }) else {
        return ABI_ERROR;
    };
    let session_id = match unsafe { utf8_argument(session_id, session_id_length) } {
        Ok(value) => value,
        Err(error) => return runtime.reject_argument(error),
    };
    let command = if reject {
        BridgeCommand::RejectPairing { session_id_hex: session_id }
    } else {
        BridgeCommand::CancelPairing { session_id_hex: session_id }
    };
    runtime.execute(command)
}
