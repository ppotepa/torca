use crate::domain::Target;
use crate::paths::RuntimePaths;
use crate::process::{CommandRunner, CommandSpec, ProcessError};
use std::time::Duration;
use thiserror::Error;

#[derive(Clone, Debug)]
pub struct Device {
    pub target: Target,
    pub id: String,
    pub android_abi: Option<AndroidAbi>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AndroidAbi {
    Arm64,
    X86_64,
}

impl AndroidAbi {
    pub const fn package_name(self) -> &'static str {
        match self {
            Self::Arm64 => "arm64-v8a",
            Self::X86_64 => "x86_64",
        }
    }
}
pub struct DeviceController<'a> {
    paths: &'a RuntimePaths,
    runner: &'a dyn CommandRunner,
}
impl<'a> DeviceController<'a> {
    pub fn new(paths: &'a RuntimePaths, runner: &'a dyn CommandRunner) -> Self {
        Self { paths, runner }
    }
    pub fn discover(&self, targets: &[Target]) -> Result<Vec<Device>, DeviceError> {
        let mut result = Vec::new();
        if targets.contains(&Target::Windows) {
            result.push(Device {
                target: Target::Windows,
                id: std::env::var("COMPUTERNAME").unwrap_or_else(|_| "windows".into()),
                android_abi: None,
            });
        }
        if targets.contains(&Target::Android) {
            let output = self.runner.run(&CommandSpec {
                program: "adb".into(),
                arguments: vec!["devices".into()],
                working_directory: self.paths.repo_root.clone(),
                timeout: Duration::from_secs(30),
                environment: std::collections::BTreeMap::new(),
            })?;
            if !output.success {
                return Err(DeviceError::Command(output.text));
            }
            for line in output.text.lines().skip(1) {
                let id = line.split_whitespace().next().unwrap_or_default();
                if !id.is_empty() && line.contains("device") {
                    let abi = self.android_abi(id)?;
                    result.push(Device {
                        target: Target::Android,
                        id: id.into(),
                        android_abi: Some(abi),
                    });
                }
            }
        }
        for target in targets {
            if !result.iter().any(|device| device.target == *target) {
                return Err(DeviceError::RequestedTargetUnavailable(*target));
            }
        }
        Ok(result)
    }

    fn android_abi(&self, device: &str) -> Result<AndroidAbi, DeviceError> {
        let output = self.runner.run(&CommandSpec {
            program: "adb".into(),
            arguments: vec![
                "-s".into(),
                device.into(),
                "shell".into(),
                "getprop".into(),
                "ro.product.cpu.abi".into(),
            ],
            working_directory: self.paths.repo_root.clone(),
            timeout: Duration::from_secs(30),
            environment: std::collections::BTreeMap::new(),
        })?;
        if !output.success {
            return Err(DeviceError::Command(output.text));
        }
        let value = output.text.trim();
        match value {
            value if value.contains("arm64-v8a") => Ok(AndroidAbi::Arm64),
            value if value.contains("x86_64") => Ok(AndroidAbi::X86_64),
            _ => {
                Err(DeviceError::UnsupportedAndroidAbi { device: device.into(), abi: value.into() })
            }
        }
    }
}
#[derive(Debug, Error)]
pub enum DeviceError {
    #[error("device command failed: {0}")]
    Command(String),
    #[error("device process error: {0}")]
    Process(#[from] ProcessError),
    #[error("requested {0} deployment target is unavailable")]
    RequestedTargetUnavailable(Target),
    #[error("Android device {device} uses unsupported ABI `{abi}`")]
    UnsupportedAndroidAbi { device: String, abi: String },
}
