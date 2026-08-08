#[cfg(windows)]
use std::path::{Path, PathBuf};

use crate::composition::NativeCompositionError;

/// Version-neutral Windows application root. Existing 0.1 installations are migrated once by
/// moving all owned runtime children into `%LOCALAPPDATA%/Torca` before stores are opened.
#[cfg(windows)]
pub(crate) fn windows_app_root() -> Result<PathBuf, NativeCompositionError> {
    let root = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .ok_or_else(|| NativeCompositionError::new("Windows local application data is unavailable"))?
        .join("Torca");
    migrate_windows_legacy_root(&root)?;
    std::fs::create_dir_all(&root)
        .map_err(|error| NativeCompositionError::new(format!("create application root failed ({:?})", error.kind())))?;
    Ok(root)
}

#[cfg(windows)]
fn migrate_windows_legacy_root(root: &Path) -> Result<(), NativeCompositionError> {
    let legacy = root.join("0.1");
    if !legacy.is_dir() { return Ok(()); }
    std::fs::create_dir_all(root)
        .map_err(|error| NativeCompositionError::new(format!("create migration root failed ({:?})", error.kind())))?;
    let entries = std::fs::read_dir(&legacy)
        .map_err(|error| NativeCompositionError::new(format!("read legacy application root failed ({:?})", error.kind())))?;
    for entry in entries {
        let entry = entry
            .map_err(|error| NativeCompositionError::new(format!("read legacy application entry failed ({:?})", error.kind())))?;
        let destination = root.join(entry.file_name());
        if destination.exists() {
            return Err(NativeCompositionError::new(
                "legacy and version-neutral Torca data both exist; refusing to merge automatically",
            ));
        }
        std::fs::rename(entry.path(), &destination)
            .map_err(|error| NativeCompositionError::new(format!("migrate legacy application data failed ({:?})", error.kind())))?;
    }
    std::fs::remove_dir(&legacy)
        .map_err(|error| NativeCompositionError::new(format!("remove legacy application root failed ({:?})", error.kind())))?;
    Ok(())
}
