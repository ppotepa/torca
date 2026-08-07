use core::{slice, str};

use torca_bridge::BridgeCommand;

use crate::native_runtime::ABI_ERROR;
use crate::process_runtime::NativeEngineHandle;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_retry_message(
    handle: *mut NativeEngineHandle,
    message_id: *const u8,
    message_id_length: usize,
    at_ms: i64,
) -> i32 {
    let Some(handle) = (unsafe { handle.as_ref() }) else { return ABI_ERROR; };
    let message_id = match unsafe { utf8_argument(message_id, message_id_length) } {
        Ok(value) if !value.is_empty() => value,
        Ok(_) => return ABI_ERROR,
        Err(_) => return ABI_ERROR,
    };
    match handle.runtime.lock() {
        Ok(mut runtime) => runtime.execute(BridgeCommand::RetryMessage {
            message_id_hex: message_id,
            at_ms,
        }),
        Err(_) => ABI_ERROR,
    }
}

unsafe fn utf8_argument(data: *const u8, length: usize) -> Result<String, ()> {
    if data.is_null() || length == 0 { return Err(()); }
    let bytes = unsafe { slice::from_raw_parts(data, length) };
    str::from_utf8(bytes).map(str::to_owned).map_err(|_| ())
}
