use core::{ptr, slice, str};

use crate::native_runtime::{ABI_ERROR, NativeEngineRuntime};
use crate::process_runtime::NativeEngineHandle;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_conversation_page(
    handle: *mut NativeEngineHandle,
    conversation_id: *const u8,
    conversation_id_length: usize,
    before_at_ms: i64,
    before_message_id: *const u8,
    before_message_id_length: usize,
    limit: u32,
) -> i32 {
    with_runtime_mut(handle, |runtime| {
        let conversation_id = match unsafe { utf8_argument(conversation_id, conversation_id_length) } {
            Ok(value) => value,
            Err(error) => return runtime.reject_argument(error),
        };
        let before = if before_at_ms < 0 && before_message_id_length == 0 {
            None
        } else {
            let message_id = match unsafe { utf8_argument(before_message_id, before_message_id_length) } {
                Ok(value) if !value.is_empty() => value,
                _ => return runtime.reject_argument("invalid page cursor"),
            };
            Some((before_at_ms, message_id))
        };
        runtime.conversation_page(
            &conversation_id,
            before.as_ref().map(|value| value.0),
            before.as_ref().map(|value| value.1.as_str()),
            usize::try_from(limit).unwrap_or(100),
        )
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_search_messages(
    handle: *mut NativeEngineHandle,
    conversation_id: *const u8,
    conversation_id_length: usize,
    query: *const u8,
    query_length: usize,
    limit: u32,
) -> i32 {
    with_runtime_mut(handle, |runtime| {
        let conversation_id = match unsafe { utf8_argument(conversation_id, conversation_id_length) } {
            Ok(value) => value,
            Err(error) => return runtime.reject_argument(error),
        };
        let query = match unsafe { utf8_argument(query, query_length) } {
            Ok(value) => value,
            Err(error) => return runtime.reject_argument(error),
        };
        runtime.search_messages(
            &conversation_id,
            &query,
            usize::try_from(limit).unwrap_or(100),
        )
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_query_ptr(
    handle: *const NativeEngineHandle,
) -> *const u8 {
    with_runtime(handle, |runtime| runtime.query_json.as_ptr()).unwrap_or(ptr::null())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_query_len(
    handle: *const NativeEngineHandle,
) -> usize {
    with_runtime(handle, |runtime| runtime.query_json.len()).unwrap_or(0)
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
    str::from_utf8(bytes)
        .map(str::to_owned)
        .map_err(|_| "native argument is not valid UTF-8")
}
