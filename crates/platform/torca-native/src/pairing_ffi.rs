use core::{slice, str};

use torca_bridge::BridgeCommand;

use crate::native_runtime::{ABI_ERROR, NativeEngineRuntime};
use crate::process_runtime::NativeEngineHandle;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_approve_pairing(
    handle: *mut NativeEngineHandle,
    session_id: *const u8,
    session_id_length: usize,
) -> i32 {
    unsafe { pairing_command(handle, session_id, session_id_length, |session_id_hex| {
        BridgeCommand::ApprovePairing { session_id_hex }
    }) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_reject_pairing(
    handle: *mut NativeEngineHandle,
    session_id: *const u8,
    session_id_length: usize,
) -> i32 {
    unsafe { pairing_command(handle, session_id, session_id_length, |session_id_hex| {
        BridgeCommand::RejectPairing { session_id_hex }
    }) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_cancel_pairing(
    handle: *mut NativeEngineHandle,
    session_id: *const u8,
    session_id_length: usize,
) -> i32 {
    unsafe { pairing_command(handle, session_id, session_id_length, |session_id_hex| {
        BridgeCommand::CancelPairing { session_id_hex }
    }) }
}

unsafe fn pairing_command(
    handle: *mut NativeEngineHandle,
    session_id: *const u8,
    session_id_length: usize,
    make: impl FnOnce(String) -> BridgeCommand,
) -> i32 {
    with_runtime_mut(handle, |runtime| {
        let session_id = match unsafe { utf8_argument(session_id, session_id_length) } {
            Ok(value) if !value.is_empty() => value,
            Ok(_) => return runtime.reject_argument("pairing session id is empty"),
            Err(error) => return runtime.reject_argument(error),
        };
        runtime.execute(make(session_id))
    })
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

unsafe fn utf8_argument(data: *const u8, length: usize) -> Result<String, &'static str> {
    if length == 0 { return Ok(String::new()); }
    if data.is_null() { return Err("native argument pointer is null"); }
    let bytes = unsafe { slice::from_raw_parts(data, length) };
    str::from_utf8(bytes)
        .map(str::to_owned)
        .map_err(|_| "native argument is not valid UTF-8")
}
