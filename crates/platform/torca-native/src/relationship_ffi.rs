use core::{slice, str};

use torca_bridge::BridgeCommand;

use crate::native_runtime::{ABI_ERROR, NativeEngineRuntime};
use crate::process_runtime::NativeEngineHandle;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_rename_contact(
    handle: *mut NativeEngineHandle,
    contact_id: *const u8,
    contact_id_length: usize,
    display_name: *const u8,
    display_name_length: usize,
) -> i32 {
    with_runtime_mut(handle, |runtime| {
        let contact_id = match unsafe { utf8_argument(contact_id, contact_id_length) } {
            Ok(value) => value, Err(error) => return runtime.reject_argument(error),
        };
        let display_name = match unsafe { utf8_argument(display_name, display_name_length) } {
            Ok(value) => value, Err(error) => return runtime.reject_argument(error),
        };
        runtime.execute(BridgeCommand::RenameContact { contact_id_hex: contact_id, display_name })
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_block_contact(
    handle: *mut NativeEngineHandle,
    contact_id: *const u8,
    contact_id_length: usize,
) -> i32 {
    unsafe { id_command(handle, contact_id, contact_id_length, |contact_id_hex| BridgeCommand::BlockContact { contact_id_hex }) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_unblock_contact(
    handle: *mut NativeEngineHandle,
    contact_id: *const u8,
    contact_id_length: usize,
) -> i32 {
    unsafe { id_command(handle, contact_id, contact_id_length, |contact_id_hex| BridgeCommand::UnblockContact { contact_id_hex }) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_remove_contact(
    handle: *mut NativeEngineHandle,
    contact_id: *const u8,
    contact_id_length: usize,
) -> i32 {
    unsafe { id_command(handle, contact_id, contact_id_length, |contact_id_hex| BridgeCommand::RemoveContact { contact_id_hex }) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_clear_conversation_history(
    handle: *mut NativeEngineHandle,
    conversation_id: *const u8,
    conversation_id_length: usize,
) -> i32 {
    unsafe { id_command(handle, conversation_id, conversation_id_length, |conversation_id_hex| BridgeCommand::ClearConversationHistory { conversation_id_hex }) }
}

unsafe fn id_command(
    handle: *mut NativeEngineHandle,
    data: *const u8,
    length: usize,
    make: impl FnOnce(String) -> BridgeCommand,
) -> i32 {
    with_runtime_mut(handle, |runtime| {
        let value = match unsafe { utf8_argument(data, length) } {
            Ok(value) => value, Err(error) => return runtime.reject_argument(error),
        };
        runtime.execute(make(value))
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
    str::from_utf8(bytes).map(str::to_owned).map_err(|_| "native argument is not valid UTF-8")
}
