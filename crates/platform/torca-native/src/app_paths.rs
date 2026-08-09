#[cfg(windows)]
use std::path::PathBuf;

#[cfg(windows)]
use crate::composition::NativeCompositionError;

/// Stable Windows application root for the current Torca storage epoch.
#[cfg(windows)]
pub(crate) fn windows_app_root() -> Result<PathBuf, NativeCompositionError> {
    let root = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .ok_or_else(|| {
            NativeCompositionError::new("Windows local application data is unavailable")
        })?
        .join("Torca");
    std::fs::create_dir_all(&root).map_err(|error| {
        NativeCompositionError::new(format!("create application root failed ({:?})", error.kind()))
    })?;
    Ok(root)
}
