use std::sync::{Arc, Mutex, OnceLock};

use crate::native_runtime::{ABI_ERROR, ABI_OK, NativeEngineRuntime};

static PROCESS_RUNTIME: OnceLock<Mutex<Option<Arc<Mutex<NativeEngineRuntime>>>>> = OnceLock::new();

pub struct NativeEngineHandle {
    pub(crate) runtime: Arc<Mutex<NativeEngineRuntime>>,
}

pub(crate) fn acquire_process_runtime() -> Result<NativeEngineHandle, ()> {
    let registry = PROCESS_RUNTIME.get_or_init(|| Mutex::new(None));
    let mut guard = registry.lock().map_err(|_| ())?;
    let runtime = match guard.as_ref() {
        Some(runtime) => Arc::clone(runtime),
        None => {
            let runtime = Arc::new(Mutex::new(NativeEngineRuntime::new()?));
            *guard = Some(Arc::clone(&runtime));
            runtime
        }
    };
    Ok(NativeEngineHandle { runtime })
}

pub(crate) fn ensure_process_runtime() -> bool {
    acquire_process_runtime().is_ok()
}

/// Historical handle-level close remains presentation-local.
/// Releasing a Flutter/desktop handle must never implicitly stop the process runtime.
pub(crate) const fn shutdown_process_runtime() -> i32 {
    ABI_OK
}

pub(crate) fn force_shutdown_process_runtime() -> i32 {
    let registry = PROCESS_RUNTIME.get_or_init(|| Mutex::new(None));
    let runtime = match registry.lock() {
        Ok(mut guard) => guard.take(),
        Err(_) => return ABI_ERROR,
    };
    let Some(runtime) = runtime else { return ABI_OK; };
    match runtime.lock() {
        Ok(mut runtime) => runtime.close(),
        Err(_) => ABI_ERROR,
    }
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_torca_host_NativeRuntimeBridge_nativeEnsureRuntime(
    _env: *mut jni::sys::JNIEnv,
    _class: jni::sys::jclass,
) -> jni::sys::jboolean {
    if ensure_process_runtime() { 1 } else { 0 }
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_torca_host_NativeRuntimeBridge_nativeShutdownRuntime(
    _env: *mut jni::sys::JNIEnv,
    _class: jni::sys::jclass,
) {
    let _ = force_shutdown_process_runtime();
}
