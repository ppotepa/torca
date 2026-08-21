/// Safe, process-local adapter around the native ABI for integration runners.
///
/// Flutter and JNI continue to use the ABI directly. This adapter exists so a
/// Rust laboratory process can exercise the same production runtime without
/// duplicating pointer ownership or leaking unsafe operations into a tool.
pub struct NativeRuntimeClient {
    handle: *mut TorcaRuntimeHandle,
}

impl NativeRuntimeClient {
    /// Acquires the one production runtime for this process using an isolated
    /// application root. Call this before spawning any other runtime threads.
    pub fn acquire_at(root: &std::path::Path) -> Result<Self, String> {
        std::fs::create_dir_all(root).map_err(|error| format!("create runtime root failed: {error}"))?;
        // This process is a dedicated lab peer. No other thread is started
        // before this call, so mutating its process environment is safe.
        unsafe { std::env::set_var("TORCA_APP_ROOT", root) };
        let handle = torca_runtime_acquire();
        if handle.is_null() {
            return Err("native runtime acquisition failed".into());
        }
        Ok(Self { handle })
    }

    /// Executes one contract request through the production actor.
    pub fn invoke_json(&mut self, request: &str, timeout: Duration) -> Result<String, String> {
        let timeout_ms = timeout.as_millis().min(u128::from(u32::MAX)) as u32;
        let status = unsafe {
            torca_runtime_invoke(self.handle, request.as_ptr(), request.len(), timeout_ms)
        };
        if status != ABI_OK {
            return Err(format!("native runtime invoke failed ({status})"));
        }
        let length = unsafe { torca_runtime_response_len(self.handle) };
        let pointer = unsafe { torca_runtime_response_ptr(self.handle) };
        if pointer.is_null() || length == 0 {
            return Err("native runtime returned an empty response".into());
        }
        let bytes = unsafe { slice::from_raw_parts(pointer, length) };
        String::from_utf8(bytes.to_vec()).map_err(|error| format!("native response was not UTF-8: {error}"))
    }
}

impl Drop for NativeRuntimeClient {
    fn drop(&mut self) {
        unsafe { torca_runtime_release(self.handle) };
        let _ = torca_runtime_shutdown(5_000);
    }
}
