//! Shared native Torca runtime implementation.

mod attachment_ffi;
mod composition;
mod ffi;
mod json;
mod native_runtime;
mod process_runtime;
mod process_shutdown_ffi;
mod runtime_composition;

pub use attachment_ffi::*;
pub use ffi::*;
pub use native_runtime::NativeEngineRuntime;
pub use process_runtime::NativeEngineHandle;
pub use process_shutdown_ffi::*;
