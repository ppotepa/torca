#[cfg(windows)]
use std::path::PathBuf;

#[cfg(windows)]
use crate::composition::NativeCompositionError;

/// Stable Windows application root for the current Torca storage epoch.
#[cfg(windows)]
pub(crate) fn windows_app_root() -> Result<PathBuf, NativeCompositionError> {
    // A laboratory peer runs in a separate process and must never share a
    // profile, identity, SQLite database or Tor cache with the desktop host.
    // Normal clients do not set this value and retain the stable LocalAppData
    // location. The override is deliberately process-scoped, never persisted.
    let root = if let Some(root) = std::env::var_os("TORCA_APP_ROOT") {
        PathBuf::from(root)
    } else {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .ok_or_else(|| {
                NativeCompositionError::new("Windows local application data is unavailable")
            })?
            .join("Torca")
    };
    std::fs::create_dir_all(&root).map_err(|error| {
        NativeCompositionError::new(format!("create application root failed ({:?})", error.kind()))
    })?;
    Ok(root)
}
