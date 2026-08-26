use crate::devices::Device;
use crate::domain::{CommunicationProvider, Configuration};
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
        communication_provider: CommunicationProvider,
        provider_profile: Option<&str>,
    ) -> Result<(), InstallError> {
        match device.target {
            crate::domain::Target::Windows => Ok(()),
            crate::domain::Target::Android => {
                let abi = device
                    .android_abi
                    .ok_or_else(|| InstallError::MissingAndroidAbi(device.id.clone()))?
                    .package_name();
                let apk = crate::build::android_apk_path(&self.paths.repo_root, abi, configuration);
                if !apk.is_file() {
                    return Err(InstallError::Artifact(apk));
                }
                crate::build::verify_artifact_manifest(
                    self.paths,
                    crate::domain::Target::Android,
                    configuration,
                    &apk,
                    communication_provider,
                    provider_profile,
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
                if !output.success {
                    return Err(InstallError::Command(output.text));
                }
                self.verify_installed(&device.id)?;
                Ok(())
            }
        }
    }

    pub fn verify_installed(&self, device: &str) -> Result<(), InstallError> {
        self.verify_package(device)?;
        self.verify_launchable_activity(device)
    }

    fn verify_package(&self, device: &str) -> Result<(), InstallError> {
        let output = self.runner.run(&CommandSpec {
            program: "adb".into(),
            arguments: vec![
                "-s".into(),
                device.into(),
                "shell".into(),
                "pm".into(),
                "path".into(),
                crate::android_target::package().into(),
            ],
            working_directory: self.paths.repo_root.clone(),
            timeout: Duration::from_secs(30),
            environment: std::collections::BTreeMap::new(),
        })?;
        if output.success && package_is_installed(&output.text) {
            Ok(())
        } else {
            Err(InstallError::PackageVerification {
                package: crate::android_target::package().into(),
                details: output.text,
            })
        }
    }

    fn verify_launchable_activity(&self, device: &str) -> Result<(), InstallError> {
        let output = self.runner.run(&CommandSpec {
            program: "adb".into(),
            arguments: vec![
                "-s".into(),
                device.into(),
                "shell".into(),
                "cmd".into(),
                "package".into(),
                "resolve-activity".into(),
                "--brief".into(),
                "-a".into(),
                "android.intent.action.MAIN".into(),
                "-c".into(),
                "android.intent.category.LAUNCHER".into(),
                crate::android_target::package().into(),
            ],
            working_directory: self.paths.repo_root.clone(),
            timeout: Duration::from_secs(30),
            environment: std::collections::BTreeMap::new(),
        })?;
        if output.success && activity_is_resolvable(&output.text, crate::android_target::package())
        {
            Ok(())
        } else {
            Err(InstallError::ActivityVerification {
                package: crate::android_target::package().into(),
                details: output.text,
            })
        }
    }
}

fn package_is_installed(output: &str) -> bool {
    output.lines().any(|line| line.trim_start().starts_with("package:"))
}

fn activity_is_resolvable(output: &str, package: &str) -> bool {
    output.lines().any(|line| {
        let value = line.trim();
        value.starts_with(package) && value.contains('/')
    })
}
#[derive(Debug, Error)]
pub enum InstallError {
    #[error("Android artifact missing: {0}")]
    Artifact(std::path::PathBuf),
    #[error("install failed: {0}")]
    Command(String),
    #[error("artifact verification failed: {0}")]
    ArtifactVerification(String),
    #[error("Android package verification failed for {package}: {details}")]
    PackageVerification { package: String, details: String },
    #[error("Android package {package} has no MAIN/LAUNCHER activity: {details}")]
    ActivityVerification { package: String, details: String },
    #[error("install process error: {0}")]
    Process(#[from] ProcessError),
    #[error("Android ABI was not discovered for device {0}")]
    MissingAndroidAbi(String),
}

#[cfg(test)]
mod tests {
    use super::{activity_is_resolvable, package_is_installed};

    #[test]
    fn package_verification_requires_pm_path_output() {
        assert!(package_is_installed("package:/data/app/example/base.apk\n"));
        assert!(!package_is_installed(""));
        assert!(!package_is_installed("Unable to find package example"));
    }

    #[test]
    fn activity_verification_rejects_other_packages_and_errors() {
        assert!(activity_is_resolvable(
            "priority=0 preferredOrder=0 match=0x108000\ncom.torca.torca_app/com.torca.app.MainActivity\n",
            "com.torca.torca_app",
        ));
        assert!(!activity_is_resolvable("No activity found\n", "com.torca.torca_app",));
        assert!(!activity_is_resolvable(
            "com.other.app/com.example.MainActivity\n",
            "com.torca.torca_app",
        ));
    }
}
