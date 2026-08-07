//! Intent-oriented v11 ABI. Flutter supplies user intent; native Rust supplies IDs and timestamps.

use core::{slice, str};
use std::time::{SystemTime, UNIX_EPOCH};

use torca_bridge::BridgeCommand;
use torca_crypto::{CryptoProvider, RustCryptoProvider};

use crate::native_runtime::{ABI_ERROR, NativeEngineRuntime};
use crate::process_runtime::NativeEngineHandle;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_create_identity_intent(
    handle: *mut NativeEngineHandle,
    display_name: *const u8,
    display_name_length: usize,
) -> i32 {
    with_runtime_mut(handle, |runtime| {
        let display_name = match unsafe { utf8_argument(display_name, display_name_length) } {
            Ok(value) => value,
            Err(error) => return runtime.reject_argument(error),
        };
        let identity_id_hex = match secure_id_hex() {
            Ok(value) => value,
            Err(error) => return runtime.reject_argument(error),
        };
        let at_ms = match now_ms() {
            Ok(value) => value,
            Err(error) => return runtime.reject_argument(error),
        };
        runtime.execute(BridgeCommand::CreateIdentity {
            identity_id_hex,
            display_name,
            at_ms,
        })
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_create_pairing_intent(
    handle: *mut NativeEngineHandle,
) -> i32 {
    with_runtime_mut(handle, |runtime| {
        let session_id_hex = match secure_id_hex() {
            Ok(value) => value,
            Err(error) => return runtime.reject_argument(error),
        };
        runtime.execute(BridgeCommand::CreatePairing { session_id_hex })
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_join_pairing_intent(
    handle: *mut NativeEngineHandle,
    code: *const u8,
    code_length: usize,
) -> i32 {
    with_runtime_mut(handle, |runtime| {
        let code = match unsafe { utf8_argument(code, code_length) } {
            Ok(value) => value,
            Err(error) => return runtime.reject_argument(error),
        };
        let session_id_hex = match secure_id_hex() {
            Ok(value) => value,
            Err(error) => return runtime.reject_argument(error),
        };
        runtime.execute(BridgeCommand::JoinPairing { session_id_hex, code })
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_queue_message_intent(
    handle: *mut NativeEngineHandle,
    conversation_id: *const u8,
    conversation_id_length: usize,
    body: *const u8,
    body_length: usize,
    reply_to_message_id: *const u8,
    reply_to_message_id_length: usize,
) -> i32 {
    with_runtime_mut(handle, |runtime| {
        let conversation_id_hex = match unsafe { utf8_argument(conversation_id, conversation_id_length) } {
            Ok(value) => value,
            Err(error) => return runtime.reject_argument(error),
        };
        let body = match unsafe { utf8_argument(body, body_length) } {
            Ok(value) => value,
            Err(error) => return runtime.reject_argument(error),
        };
        let reply_to_message_id_hex = if reply_to_message_id_length == 0 {
            None
        } else {
            match unsafe { utf8_argument(reply_to_message_id, reply_to_message_id_length) } {
                Ok(value) => Some(value),
                Err(error) => return runtime.reject_argument(error),
            }
        };
        let message_id_hex = match secure_id_hex() {
            Ok(value) => value,
            Err(error) => return runtime.reject_argument(error),
        };
        let at_ms = match now_ms() {
            Ok(value) => value,
            Err(error) => return runtime.reject_argument(error),
        };
        runtime.execute(BridgeCommand::QueueMessage {
            message_id_hex,
            conversation_id_hex,
            body,
            reply_to_message_id_hex,
            at_ms,
        })
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_retry_message_intent(
    handle: *mut NativeEngineHandle,
    message_id: *const u8,
    message_id_length: usize,
) -> i32 {
    with_runtime_mut(handle, |runtime| {
        let message_id_hex = match unsafe { utf8_argument(message_id, message_id_length) } {
            Ok(value) => value,
            Err(error) => return runtime.reject_argument(error),
        };
        let at_ms = match now_ms() {
            Ok(value) => value,
            Err(error) => return runtime.reject_argument(error),
        };
        runtime.execute(BridgeCommand::RetryMessage { message_id_hex, at_ms })
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_queue_attachment_intent(
    handle: *mut NativeEngineHandle,
    conversation_id: *const u8,
    conversation_id_length: usize,
    source_path: *const u8,
    source_path_length: usize,
    name: *const u8,
    name_length: usize,
    media_type: *const u8,
    media_type_length: usize,
    size: u64,
) -> i32 {
    with_runtime_mut(handle, |runtime| {
        let conversation_id_hex = match unsafe { utf8_argument(conversation_id, conversation_id_length) } {
            Ok(value) => value,
            Err(error) => return runtime.reject_argument(error),
        };
        let source_path = match unsafe { utf8_argument(source_path, source_path_length) } {
            Ok(value) => value,
            Err(error) => return runtime.reject_argument(error),
        };
        let name = match unsafe { utf8_argument(name, name_length) } {
            Ok(value) => value,
            Err(error) => return runtime.reject_argument(error),
        };
        let media_type = match unsafe { utf8_argument(media_type, media_type_length) } {
            Ok(value) => value,
            Err(error) => return runtime.reject_argument(error),
        };
        let attachment_id_hex = match secure_id_hex() {
            Ok(value) => value,
            Err(error) => return runtime.reject_argument(error),
        };
        let message_id_hex = match secure_id_hex() {
            Ok(value) => value,
            Err(error) => return runtime.reject_argument(error),
        };
        runtime.execute(BridgeCommand::QueueAttachment {
            attachment_id_hex,
            message_id_hex,
            conversation_id_hex,
            source_path,
            name,
            media_type,
            size,
        })
    })
}

fn with_runtime_mut(
    handle: *mut NativeEngineHandle,
    operation: impl FnOnce(&mut NativeEngineRuntime) -> i32,
) -> i32 {
    let Some(handle) = (unsafe { handle.as_ref() }) else {
        return ABI_ERROR;
    };
    match handle.runtime.lock() {
        Ok(mut runtime) => operation(&mut runtime),
        Err(_) => ABI_ERROR,
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

fn secure_id_hex() -> Result<String, &'static str> {
    let mut provider = RustCryptoProvider;
    let mut bytes = [0_u8; 16];
    provider
        .fill_random(&mut bytes)
        .map_err(|_| "secure id generation failed")?;
    if bytes.iter().all(|byte| *byte == 0) {
        bytes[15] = 1;
    }
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn now_ms() -> Result<i64, &'static str> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "system clock is before Unix epoch")?;
    i64::try_from(duration.as_millis()).map_err(|_| "system timestamp is out of range")
}
