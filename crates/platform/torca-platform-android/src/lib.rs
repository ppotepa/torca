//! Android-only system adapter. Runtime composition remains platform-neutral.

use std::path::PathBuf;
use std::sync::Arc;
use torca_platform::{
    AppPaths, DeviceDescriptor, FileSecretStore, LifecycleCapabilities, PlatformServices,
    ProtectedSecretStore, SecretNamespace,
};

type SecretStoreFactory =
    Arc<dyn Fn(SecretNamespace) -> Box<dyn ProtectedSecretStore> + Send + Sync>;

/// Android application services supplied by the host/service bridge.
pub struct AndroidPlatformServices {
    pub paths: AppPaths,
    pub device_id: String,
    pub installation_id: String,
    secret_store_factory: Option<SecretStoreFactory>,
}

/// Rust-side handle for the Android Keystore bridge. The Android overlay owns
/// encryption and persistence; the file-backed delegate is used only when the
/// crate is exercised on a non-Android host (unit tests and tooling).
pub struct AndroidKeystoreSecretStore {
    delegate: FileSecretStore,
}

impl AndroidKeystoreSecretStore {
    fn new(root: PathBuf) -> Self {
        Self { delegate: FileSecretStore::new(root) }
    }
}

impl ProtectedSecretStore for AndroidKeystoreSecretStore {
    fn insert(
        &mut self,
        key_id: torca_identity::KeyId,
        secret: &[u8],
    ) -> Result<(), torca_platform::ProtectedSecretStoreError> {
        self.delegate.insert(key_id, secret)
    }
    fn load(
        &self,
        key_id: torca_identity::KeyId,
    ) -> Result<Option<Vec<u8>>, torca_platform::ProtectedSecretStoreError> {
        self.delegate.load(key_id)
    }
    fn delete(
        &mut self,
        key_id: torca_identity::KeyId,
    ) -> Result<bool, torca_platform::ProtectedSecretStoreError> {
        self.delegate.delete(key_id)
    }
}

impl AndroidPlatformServices {
    pub fn new(data: PathBuf, cache: PathBuf, logs: PathBuf) -> Self {
        Self {
            paths: AppPaths { data, cache, logs },
            device_id: "android-device".into(),
            installation_id: "android-install".into(),
            secret_store_factory: None,
        }
    }

    pub fn with_secret_store_factory(
        mut self,
        factory: impl Fn(SecretNamespace) -> Box<dyn ProtectedSecretStore> + Send + Sync + 'static,
    ) -> Self {
        self.secret_store_factory = Some(Arc::new(factory));
        self
    }
}

impl PlatformServices for AndroidPlatformServices {
    fn app_paths(&self) -> AppPaths {
        self.paths.clone()
    }
    fn open_secret_store(&self, namespace: SecretNamespace) -> Box<dyn ProtectedSecretStore> {
        if let Some(factory) = &self.secret_store_factory {
            return factory(namespace);
        }
        let name = format!("android-keystore/{namespace:?}");
        Box::new(AndroidKeystoreSecretStore::new(self.paths.data.join(name)))
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
