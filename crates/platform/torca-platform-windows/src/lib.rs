#![deny(unsafe_op_in_unsafe_fn)]
//! Windows-specific Torca platform integrations.
//!
//! Unsafe Win32 calls are isolated in this crate. Domain and application crates remain safe Rust.

#[cfg(windows)]
mod dpapi;

#[cfg(windows)]
pub use dpapi::DpapiFileSecretStore;

use std::path::PathBuf;
use torca_platform::{
    AppPaths, DeviceDescriptor, LifecycleCapabilities, PlatformServices, ProtectedSecretStore,
    RelayEndpoint, SecretNamespace,
};

/// Windows system services; runtime composition is shared with Android.
#[derive(Clone, Debug)]
pub struct WindowsPlatformServices {
    pub paths: AppPaths,
    pub device_id: String,
    pub installation_id: String,
    pub relay: RelayEndpoint,
}

impl WindowsPlatformServices {
    pub fn new(data: PathBuf, cache: PathBuf, logs: PathBuf, relay: RelayEndpoint) -> Self {
        Self {
            paths: AppPaths { data, cache, logs },
            device_id: "windows-device".into(),
            installation_id: "windows-install".into(),
            relay,
        }
    }
}

impl PlatformServices for WindowsPlatformServices {
    fn app_paths(&self) -> AppPaths {
        self.paths.clone()
    }
    fn open_secret_store(&self, namespace: SecretNamespace) -> Box<dyn ProtectedSecretStore> {
        let root = self.paths.data.join(format!("dpapi/{namespace:?}"));
        #[cfg(windows)]
        {
            Box::new(
                DpapiFileSecretStore::new(root)
                    .expect("Windows DPAPI secret store must be constructible"),
            )
        }
        #[cfg(not(windows))]
        {
            Box::new(torca_platform::FileSecretStore::new(root))
        }
    }
    fn relay_endpoint(&self) -> Result<RelayEndpoint, String> {
        Ok(self.relay.clone())
    }
    fn device_descriptor(&self) -> DeviceDescriptor {
        DeviceDescriptor {
            device_id: self.device_id.clone(),
            installation_id: self.installation_id.clone(),
        }
    }
    fn lifecycle_capabilities(&self) -> LifecycleCapabilities {
        LifecycleCapabilities { background_runtime: true, notifications: true }
    }
}

/// Marker available on non-Windows build hosts so the workspace remains cross-platform checkable.
#[cfg(not(windows))]
#[derive(Clone, Copy, Debug, Default)]
pub struct WindowsPlatformUnavailable;
