use crate::process_runtime::force_shutdown_process_runtime;

/// Explicitly stops the process-owned Torca runtime.
///
/// This is intentionally separate from handle destruction so Android activity recreation and
/// other presentation lifecycles cannot accidentally stop Tor, peer sessions or delivery workers.
#[unsafe(no_mangle)]
pub extern "C" fn torca_process_shutdown() -> i32 {
    force_shutdown_process_runtime()
}
