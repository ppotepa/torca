#![deny(unsafe_op_in_unsafe_fn)]
//! Windows-specific Torca platform integrations.
//!
//! Unsafe Win32 calls are isolated in this crate. Domain and application crates remain safe Rust.

#[cfg(windows)]
mod dpapi;

#[cfg(windows)]
pub use dpapi::DpapiFileSecretStore;

use std::path::PathBuf;
use torca_diagnostics::{PlatformEnergyProvider, PlatformEnergySample};
use torca_platform::{
    AppPaths, DeviceDescriptor, LifecycleCapabilities, PlatformServices, ProtectedSecretStore,
    SecretNamespace,
};

/// Windows system services; runtime composition is shared with Android.
#[derive(Clone)]
pub struct WindowsPlatformServices {
    pub paths: AppPaths,
    pub device_id: String,
    pub installation_id: String,
}

/// Event-triggered Windows energy sample. The caller chooses when to sample
/// (power notification, diagnostics, or incident collection); this provider
/// never starts a polling thread.
#[derive(Clone, Copy, Debug, Default)]
pub struct WindowsEnergyProvider;

impl PlatformEnergyProvider for WindowsEnergyProvider {
    fn sample(&self) -> PlatformEnergySample {
        sample_energy()
    }
}

#[cfg(windows)]
fn sample_energy() -> PlatformEnergySample {
    use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
    let mut status = SYSTEM_POWER_STATUS::default();
    // SAFETY: Windows fills this caller-owned POD structure synchronously.
    let ok = unsafe { GetSystemPowerStatus(&mut status) } != 0;
    if !ok {
        return PlatformEnergySample::default();
    }
    let battery_percent = (status.BatteryLifePercent != 255).then_some(status.BatteryLifePercent);
    let charging = match status.ACLineStatus {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    };
    PlatformEnergySample {
        battery_percent,
        charging,
        power_saver: None,
        metered_network: None,
        process_cpu_ms: None,
        uid_tx_bytes: None,
        uid_rx_bytes: None,
    }
}

#[cfg(not(windows))]
fn sample_energy() -> PlatformEnergySample {
    PlatformEnergySample::default()
}

impl WindowsPlatformServices {
    pub fn new(data: PathBuf, cache: PathBuf, logs: PathBuf) -> Self {
        Self {
            paths: AppPaths { data, cache, logs },
            device_id: "windows-device".into(),
            installation_id: "windows-install".into(),
        }
    }

    pub fn energy_provider(&self) -> WindowsEnergyProvider {
        WindowsEnergyProvider
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
    fn device_descriptor(&self) -> DeviceDescriptor {
        DeviceDescriptor {
            device_id: self.device_id.clone(),
            installation_id: self.installation_id.clone(),
        }
    }
    fn lifecycle_capabilities(&self) -> LifecycleCapabilities {
        LifecycleCapabilities { background_runtime: true, notifications: true }
    }
    fn energy_sample(&self) -> PlatformEnergySample {
        self.energy_provider().sample()
    }
}

/// Marker available on non-Windows build hosts so the workspace remains cross-platform checkable.
#[cfg(not(windows))]
#[derive(Clone, Copy, Debug, Default)]
pub struct WindowsPlatformUnavailable;
