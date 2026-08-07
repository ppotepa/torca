use std::path::PathBuf;

use torca_crypto::ProtectedSecretStoreError;

/// Resolves only the Tor binary shipped beside the Windows application bundle.
pub fn discover_packaged_tor() -> Result<PathBuf, ProtectedSecretStoreError> {
    let executable = std::env::current_exe().map_err(|_| {
        ProtectedSecretStoreError("unable to resolve Torca executable directory".into())
    })?;
    let root = executable.parent().ok_or_else(|| {
        ProtectedSecretStoreError("Torca executable has no parent directory".into())
    })?;
    let tor = root.join("tor").join("tor.exe");
    if !tor.is_file() {
        return Err(ProtectedSecretStoreError(
            "packaged Tor executable is missing".into(),
        ));
    }
    Ok(tor)
}
