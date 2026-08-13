use crate::domain::Target;
use crate::paths::RuntimePaths;
use crate::process::{CommandRunner, CommandSpec, ProcessError};
use std::time::Duration;
use thiserror::Error;

#[derive(Clone, Debug)]
pub struct Device {
    pub target: Target,
    pub id: String,
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
                    result.push(Device { target: Target::Android, id: id.into() });
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
}
#[derive(Debug, Error)]
pub enum DeviceError {
    #[error("device command failed: {0}")]
    Command(String),
    #[error("device process error: {0}")]
    Process(#[from] ProcessError),
    #[error("requested {0} deployment target is unavailable")]
    RequestedTargetUnavailable(Target),
}
