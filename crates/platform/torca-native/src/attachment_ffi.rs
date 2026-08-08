use core::{slice, str};

use torca_bridge::BridgeCommand;

use crate::native_runtime::ABI_ERROR;
use crate::process_runtime::NativeEngineHandle;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_retry_attachment(
    handle: *mut NativeEngineHandle,
    id: *const u8,
    length: usize,
) -> i32 {
    unsafe { id_command(handle, id, length, |attachment_id_hex| {
        BridgeCommand::RetryAttachment { attachment_id_hex }
    }) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_cancel_attachment(
    handle: *mut NativeEngineHandle,
    id: *const u8,
    length: usize,
) -> i32 {
    unsafe { id_command(handle, id, length, |attachment_id_hex| {
        BridgeCommand::CancelAttachment { attachment_id_hex }
    }) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_export_attachment(
    handle: *mut NativeEngineHandle,
    id: *const u8,
    id_length: usize,
    destination: *const u8,
    destination_length: usize,
) -> i32 {
    let Some(handle) = (unsafe { handle.as_ref() }) else { return ABI_ERROR; };
    let Ok(mut runtime) = handle.runtime.lock() else { return ABI_ERROR; };
    let attachment_id = match unsafe { utf8(id, id_length) } {
        Ok(value) => value,
        Err(error) => return runtime.reject_argument(error),
    };
    let destination_path = match unsafe { utf8(destination, destination_length) } {
        Ok(value) => value,
        Err(error) => return runtime.reject_argument(error),
    };
    if destination_path.is_empty() {
        return runtime.reject_argument("attachment export destination is empty");
    }
    runtime.execute(BridgeCommand::ExportAttachment { attachment_id_hex: attachment_id, destination_path })
}

unsafe fn id_command(
    handle: *mut NativeEngineHandle,
    id: *const u8,
    length: usize,
    make: impl FnOnce(String) -> BridgeCommand,
) -> i32 {
    let Some(handle) = (unsafe { handle.as_ref() }) else { return ABI_ERROR; };
    let Ok(mut runtime) = handle.runtime.lock() else { return ABI_ERROR; };
    let value = match unsafe { utf8(id, length) } {
        Ok(value) if !value.is_empty() => value,
        Ok(_) => return runtime.reject_argument("attachment id is empty"),
        Err(error) => return runtime.reject_argument(error),
    };
    runtime.execute(make(value))
}

unsafe fn utf8(data: *const u8, length: usize) -> Result<String, &'static str> {
    if length == 0 { return Ok(String::new()); }
    if data.is_null() { return Err("native argument pointer is null"); }
    let bytes = unsafe { slice::from_raw_parts(data, length) };
    str::from_utf8(bytes)
        .map(str::to_owned)
        .map_err(|_| "native argument is not valid UTF-8")
}
