use core::{slice, str};

use torca_bridge::BridgeCommand;
use crate::native_runtime::ABI_ERROR;
use crate::process_runtime::NativeEngineHandle;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_queue_attachment(handle:*mut NativeEngineHandle,attachment_id:*const u8,attachment_id_length:usize,message_id:*const u8,message_id_length:usize,conversation_id:*const u8,conversation_id_length:usize,source_path:*const u8,source_path_length:usize,name:*const u8,name_length:usize,media_type:*const u8,media_type_length:usize,size:u64)->i32{
 let Some(handle)=(unsafe{handle.as_ref()})else{return ABI_ERROR};let Ok(mut runtime)=handle.runtime.lock()else{return ABI_ERROR};
 let attachment_id=match unsafe{utf8(attachment_id,attachment_id_length)}{Ok(v)=>v,Err(e)=>return runtime.reject_argument(e)};let message_id=match unsafe{utf8(message_id,message_id_length)}{Ok(v)=>v,Err(e)=>return runtime.reject_argument(e)};let conversation_id=match unsafe{utf8(conversation_id,conversation_id_length)}{Ok(v)=>v,Err(e)=>return runtime.reject_argument(e)};let source_path=match unsafe{utf8(source_path,source_path_length)}{Ok(v)=>v,Err(e)=>return runtime.reject_argument(e)};let name=match unsafe{utf8(name,name_length)}{Ok(v)=>v,Err(e)=>return runtime.reject_argument(e)};let media_type=match unsafe{utf8(media_type,media_type_length)}{Ok(v)=>v,Err(e)=>return runtime.reject_argument(e)};
 runtime.execute(BridgeCommand::QueueAttachment{attachment_id_hex:attachment_id,message_id_hex:message_id,conversation_id_hex:conversation_id,source_path,name,media_type,size})}

#[unsafe(no_mangle)] pub unsafe extern "C" fn torca_engine_retry_attachment(handle:*mut NativeEngineHandle,id:*const u8,len:usize)->i32{unsafe{id_command(handle,id,len,|attachment_id_hex|BridgeCommand::RetryAttachment{attachment_id_hex})}}
#[unsafe(no_mangle)] pub unsafe extern "C" fn torca_engine_cancel_attachment(handle:*mut NativeEngineHandle,id:*const u8,len:usize)->i32{unsafe{id_command(handle,id,len,|attachment_id_hex|BridgeCommand::CancelAttachment{attachment_id_hex})}}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn torca_engine_export_attachment(handle:*mut NativeEngineHandle,id:*const u8,id_len:usize,destination:*const u8,destination_len:usize)->i32{
 let Some(handle)=(unsafe{handle.as_ref()})else{return ABI_ERROR};let Ok(mut runtime)=handle.runtime.lock()else{return ABI_ERROR};let attachment_id=match unsafe{utf8(id,id_len)}{Ok(v)=>v,Err(e)=>return runtime.reject_argument(e)};let destination_path=match unsafe{utf8(destination,destination_len)}{Ok(v)=>v,Err(e)=>return runtime.reject_argument(e)};if destination_path.is_empty(){return runtime.reject_argument("attachment export destination is empty")};runtime.execute(BridgeCommand::ExportAttachment{attachment_id_hex:attachment_id,destination_path})}
unsafe fn id_command(handle:*mut NativeEngineHandle,id:*const u8,len:usize,make:impl FnOnce(String)->BridgeCommand)->i32{let Some(handle)=(unsafe{handle.as_ref()})else{return ABI_ERROR};let Ok(mut runtime)=handle.runtime.lock()else{return ABI_ERROR};let value=match unsafe{utf8(id,len)}{Ok(v)=>v,Err(e)=>return runtime.reject_argument(e)};runtime.execute(make(value))}
unsafe fn utf8(data:*const u8,length:usize)->Result<String,&'static str>{if length==0{return Ok(String::new())}if data.is_null(){return Err("native argument pointer is null")}let bytes=unsafe{slice::from_raw_parts(data,length)};str::from_utf8(bytes).map(str::to_owned).map_err(|_|"native argument is not valid UTF-8")}
