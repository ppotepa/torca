use core::ptr;

use torca_bridge::CONTRACT_VERSION;

use crate::native_runtime::{ABI_ERROR, NativeEngineRuntime};
use crate::process_runtime::{NativeEngineHandle, acquire_process_runtime};

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

/// Handle-level close is presentation-local. Process-owned resources stop only on explicit Quit.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_close(_handle: *mut NativeEngineHandle) -> i32 { 0 }

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
