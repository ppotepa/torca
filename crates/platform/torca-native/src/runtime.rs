//! Shared native Torca runtime implementation.

mod composition;
mod ffi;
mod json;
mod native_runtime;
mod process_runtime;
mod runtime_composition;

pub use ffi::*;
pub use native_runtime::NativeEngineRuntime;
pub use process_runtime::NativeEngineHandle;
