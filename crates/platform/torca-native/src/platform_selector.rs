//! The single host-level platform selector.
//!
//! Domain/runtime composition receives only [`PlatformServices`].  The small
//! target branches below are the one permitted place where the native host
//! chooses its Windows or Android adapter.

#[cfg(windows)]
use crate::app_paths::windows_app_root;
use crate::composition::NativeCompositionError;
use torca_platform::PlatformServices;
#[cfg(target_os = "android")]
use torca_platform::SecretNamespace;

pub(crate) fn platform_services() -> Result<Box<dyn PlatformServices>, NativeCompositionError> {
    #[cfg(windows)]
    {
        use torca_platform_windows::WindowsPlatformServices;

        let root = windows_app_root()?;
        Ok(Box::new(WindowsPlatformServices::new(
            root.join("data"),
            root.join("cache"),
            root.join("logs"),
        )))
    }

    #[cfg(target_os = "android")]
    {
        use crate::composition::android::{
            AndroidProtectedSecretStore, database_path, log_root_path,
        };
        use torca_platform_android::AndroidPlatformServices;

        let database = database_path()
            .map_err(|_| NativeCompositionError::new("resolve Android database path failed"))?;
        let data = database.parent().map_or_else(|| database.clone(), std::path::Path::to_path_buf);
        let platform = AndroidPlatformServices::new(
            data.clone(),
            data.join("cache"),
            log_root_path().unwrap_or_else(|_| data.join("logs")),
        )
        .with_secret_store_factory(|namespace| {
            let name = match namespace {
                SecretNamespace::Identity => "identity",
                SecretNamespace::Storage => "database",
                SecretNamespace::Runtime => "peer",
            };
            Box::new(AndroidProtectedSecretStore::new(name))
        });
        return Ok(Box::new(platform));
    }

    #[cfg(not(any(windows, target_os = "android")))]
    {
        Err(NativeCompositionError::new(
            "production native composition is not implemented for this platform",
        ))
    }
}
