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
                let abi = self
                    .runner
                    .run(&CommandSpec {
                        program: "adb".into(),
                        arguments: vec![
                            "-s".into(),
                            device.id.clone(),
                            "shell".into(),
                            "getprop".into(),
                            "ro.product.cpu.abi".into(),
                        ],
                        working_directory: self.paths.repo_root.clone(),
                        timeout: Duration::from_secs(30),
                        environment: std::collections::BTreeMap::new(),
                    })?
                    .text;
                let abi = if abi.contains("x86_64") { "x86_64" } else { "arm64-v8a" };
                let apk = self.paths.repo_root.join(format!(
                    "apps/client/flutter/build/app/outputs/flutter-apk/app-{abi}-{mode}.apk"
                ));
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
}
