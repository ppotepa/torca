use core::{slice, str};

use torca_bridge::BridgeCommand;

use crate::native_runtime::{ABI_ERROR, NativeEngineRuntime};
use crate::process_runtime::NativeEngineHandle;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_mark_conversation_read_intent(
    handle: *mut NativeEngineHandle,
    conversation_id: *const u8,
    conversation_id_length: usize,
    send_receipt: u8,
) -> i32 {
    with_runtime_mut(handle, |runtime| {
        let conversation_id = match unsafe { utf8_argument(conversation_id, conversation_id_length) } {
            Ok(value) => value,
            Err(error) => return runtime.reject_argument(error),
        };
        runtime.execute(BridgeCommand::MarkConversationReadWithPolicy {
            conversation_id_hex: conversation_id,
            send_receipt: send_receipt != 0,
        })
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
