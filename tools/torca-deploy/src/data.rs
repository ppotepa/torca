use crate::devices::Device;
use crate::domain::ClientDataPolicy;
use crate::paths::RuntimePaths;
use crate::process::{CommandRunner, CommandSpec, ProcessError};
use std::time::Duration;
use thiserror::Error;

pub struct DataController<'a> {
    paths: &'a RuntimePaths,
    runner: &'a dyn CommandRunner,
}
impl<'a> DataController<'a> {
    pub fn new(paths: &'a RuntimePaths, runner: &'a dyn CommandRunner) -> Self {
        Self { paths, runner }
    }
    pub fn reset(&self, devices: &[Device], policy: ClientDataPolicy) -> Result<(), DataError> {
        if matches!(policy, ClientDataPolicy::Preserve) {
            return Ok(());
        }
        for device in devices {
            match device.target {
                crate::domain::Target::Windows => {
                    let root = std::env::var_os("LOCALAPPDATA")
                        .map(std::path::PathBuf::from)
                        .ok_or(DataError::MissingLocalAppData)?
                        .join("Torca");
                    if root.is_dir() {
                        let backup_root =
                            root.parent().unwrap_or(&self.paths.runtime_root).join("Torca-backups");
                        std::fs::create_dir_all(&backup_root).map_err(DataError::Io)?;
                        let backup = backup_root.join(format!("{}-{}", chrono_stamp(), device.id));
                        std::fs::rename(&root, backup).map_err(DataError::Io)?;
                    }
                }
                crate::domain::Target::Android => {
                    let output = self.runner.run(&CommandSpec {
                        program: "adb".into(),
                        arguments: vec![
                            "-s".into(),
                            device.id.clone(),
                            "shell".into(),
                            "pm".into(),
                            "clear".into(),
                            "com.torca.torca_app".into(),
                        ],
                        working_directory: self.paths.repo_root.clone(),
                        timeout: Duration::from_secs(60),
                        environment: std::collections::BTreeMap::new(),
                    })?;
                    if !output.success {
                        return Err(DataError::Command(output.text));
                    }
                }
            }
        }
        Ok(())
    }
}
#[derive(Debug, Error)]
pub enum DataError {
    #[error("data reset failed: {0}")]
    Command(String),
    #[error("data reset process error: {0}")]
    Process(#[from] ProcessError),
    #[error("data reset I/O failed: {0}")]
    Io(std::io::Error),
    #[error("LOCALAPPDATA is not available for Windows data reset")]
    MissingLocalAppData,
}

fn chrono_stamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs().to_string()
}
