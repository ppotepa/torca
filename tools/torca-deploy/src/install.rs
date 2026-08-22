use crate::devices::Device;
use crate::domain::Configuration;
use crate::paths::RuntimePaths;
use crate::process::{CommandRunner, CommandSpec, ProcessError};
use std::time::Duration;
use thiserror::Error;

pub struct InstallController<'a> {
    paths: &'a RuntimePaths,
    runner: &'a dyn CommandRunner,
}
impl<'a> InstallController<'a> {
    pub fn new(paths: &'a RuntimePaths, runner: &'a dyn CommandRunner) -> Self {
        Self { paths, runner }
    }
    pub fn install(
        &self,
        device: &Device,
        configuration: Configuration,
    ) -> Result<(), InstallError> {
        match device.target {
            crate::domain::Target::Windows => Ok(()),
            crate::domain::Target::Android => {
                let mode = match configuration {
                    Configuration::Debug => "debug",
                    Configuration::Release => "release",
                };
                let abi = device
                    .android_abi
                    .ok_or_else(|| InstallError::MissingAndroidAbi(device.id.clone()))?
                    .package_name();
                let apk = if crate::build::soak_flavor_enabled()
                    && matches!(configuration, Configuration::Debug)
                {
                    self.paths.repo_root.join(format!(
                        "apps/client/flutter/build/app/outputs/flutter-apk/app-{abi}-soak-debug.apk"
                    ))
                } else {
                    self.paths.repo_root.join(format!(
                        "apps/client/flutter/build/app/outputs/flutter-apk/app-{abi}-normal-{mode}.apk"
                    ))
                };
                if !apk.is_file() {
                    return Err(InstallError::Artifact(apk));
                }
                crate::build::verify_artifact_manifest(
                    self.paths,
                    crate::domain::Target::Android,
                    configuration,
                    &apk,
                )
                .map_err(InstallError::ArtifactVerification)?;
                let args = vec![
                    "-s".into(),
                    device.id.clone(),
                    "install".into(),
                    "-r".into(),
                    apk.display().to_string(),
                ];
                let output = self.runner.run(&CommandSpec {
                    program: "adb".into(),
                    arguments: args,
                    working_directory: self.paths.repo_root.clone(),
                    timeout: Duration::from_secs(120),
                    environment: std::collections::BTreeMap::new(),
                })?;
                if output.success { Ok(()) } else { Err(InstallError::Command(output.text)) }
            }
        }
    }
}
#[derive(Debug, Error)]
pub enum InstallError {
    #[error("Android artifact missing: {0}")]
    Artifact(std::path::PathBuf),
    #[error("install failed: {0}")]
    Command(String),
    #[error("artifact verification failed: {0}")]
    ArtifactVerification(String),
    #[error("install process error: {0}")]
    Process(#[from] ProcessError),
    #[error("Android ABI was not discovered for device {0}")]
    MissingAndroidAbi(String),
}
