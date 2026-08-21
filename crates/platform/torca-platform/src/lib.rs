//! Platform lifecycle policy shared by Windows and Android hosts.

use std::path::PathBuf;
use torca_diagnostics::PlatformEnergySample;

pub use torca_crypto::{ProtectedSecretStore, ProtectedSecretStoreError};
use torca_identity::KeyId;

/// Application-private filesystem locations supplied by the platform adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppPaths {
    pub data: PathBuf,
    pub cache: PathBuf,
    pub logs: PathBuf,
}

/// Namespace used to isolate protected secrets.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SecretNamespace {
    Identity,
    Storage,
    Runtime,
}

/// Configured relay endpoint, without platform-specific transport details.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelayEndpoint {
    pub host: String,
    pub port: u16,
}

/// Stable device/install descriptor used for logging and manifest matching.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceDescriptor {
    pub device_id: String,
    pub installation_id: String,
}

/// OS capabilities relevant to lifecycle ownership.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifecycleCapabilities {
    pub background_runtime: bool,
    pub notifications: bool,
}

/// Platform boundary consumed by the shared runtime composition.
pub trait PlatformServices: Send + Sync {
    fn app_paths(&self) -> AppPaths;
    fn open_secret_store(&self, namespace: SecretNamespace) -> Box<dyn ProtectedSecretStore>;
    fn relay_endpoint(&self) -> Result<RelayEndpoint, String>;
    fn device_descriptor(&self) -> DeviceDescriptor;
    fn lifecycle_capabilities(&self) -> LifecycleCapabilities;
    /// Returns an event-triggered energy sample. Platform hosts may override
    /// this at lifecycle/diagnostics boundaries; the default is intentionally
    /// empty rather than a polling fallback.
    fn energy_sample(&self) -> PlatformEnergySample {
        PlatformEnergySample::default()
    }
}

/// Filesystem-backed secret store useful for tests and development adapters.
pub struct FileSecretStore {
    root: PathBuf,
}

impl FileSecretStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
}

impl ProtectedSecretStore for FileSecretStore {
    fn insert(&mut self, key_id: KeyId, secret: &[u8]) -> Result<(), ProtectedSecretStoreError> {
        std::fs::create_dir_all(&self.root).map_err(|error| {
            ProtectedSecretStoreError(format!("secret store unavailable: {error:?}"))
        })?;
        let path = self.root.join(key_id.to_string());
        if path.exists() {
            return Err(ProtectedSecretStoreError("protected key handle already exists".into()));
        }
        std::fs::write(path, secret).map_err(|error| {
            ProtectedSecretStoreError(format!("secret store write failed: {error:?}"))
        })
    }

    fn load(&self, key_id: KeyId) -> Result<Option<Vec<u8>>, ProtectedSecretStoreError> {
        match std::fs::read(self.root.join(key_id.to_string())) {
            Ok(value) => Ok(Some(value)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => {
                Err(ProtectedSecretStoreError(format!("secret store read failed: {error:?}")))
            }
        }
    }

    fn delete(&mut self, key_id: KeyId) -> Result<bool, ProtectedSecretStoreError> {
        match std::fs::remove_file(self.root.join(key_id.to_string())) {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => {
                Err(ProtectedSecretStoreError(format!("secret store delete failed: {error:?}")))
            }
        }
    }
}

/// Host lifecycle event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleEvent {
    Started,
    Foregrounded,
    Backgrounded,
    CloseRequested,
    Terminating,
}
/// Engine lifecycle action chosen without platform APIs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleAction {
    StartEngine,
    ResumeEngine,
    KeepEngineAlive,
    MinimizeToTray,
    FlushAndStop,
    NoOp,
}
/// Platform class.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlatformClass {
    WindowsDesktop,
    AndroidMobile,
}
/// Deterministic lifecycle policy.
pub struct LifecyclePolicy;
impl LifecyclePolicy {
    /// Maps host lifecycle to engine ownership action.
    pub const fn action(platform: PlatformClass, event: LifecycleEvent) -> LifecycleAction {
        match (platform, event) {
            (_, LifecycleEvent::Started) => LifecycleAction::StartEngine,
            (_, LifecycleEvent::Foregrounded) => LifecycleAction::ResumeEngine,
            (PlatformClass::WindowsDesktop, LifecycleEvent::CloseRequested) => {
                LifecycleAction::MinimizeToTray
            }
            (PlatformClass::AndroidMobile, LifecycleEvent::Backgrounded) => {
                LifecycleAction::KeepEngineAlive
            }
            (_, LifecycleEvent::Terminating) => LifecycleAction::FlushAndStop,
            _ => LifecycleAction::NoOp,
        }
    }
}
