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
            // A reset must never race the process which owns SQLite, the
            // protected-secret files, or Arti's state directory.  Stop the
            // client first; launch happens as a later, separately
            // checkpointed stage.
            self.stop_client(device)?;
            match device.target {
                crate::domain::Target::Windows => {
                    let root = std::env::var_os("LOCALAPPDATA")
                        .map(std::path::PathBuf::from)
                        .ok_or(DataError::MissingLocalAppData)?
                        .join("Torca");
                    self.reset_windows(&root, &device.id, policy)?;
                }
                crate::domain::Target::Android => {
                    self.reset_android(device, policy)?;
                }
            }
        }
        Ok(())
    }

    fn stop_client(&self, device: &Device) -> Result<(), DataError> {
        match device.target {
            crate::domain::Target::Windows => {
                crate::windows_client::WorkspaceWindowsClient::new(self.paths, self.runner)
                    .stop()
                    .map_err(DataError::WindowsClient)
            }
            crate::domain::Target::Android => {
                let output = self
                    .command(&device.id, &["shell", "am", "force-stop", "com.torca.torca_app"])?;
                if output.success { Ok(()) } else { Err(DataError::Command(output.text)) }
            }
        }
    }

    fn reset_windows(
        &self,
        root: &std::path::Path,
        device_id: &str,
        policy: ClientDataPolicy,
    ) -> Result<(), DataError> {
        if !root.is_dir() {
            return Ok(());
        }
        match policy {
            // A profile reset intentionally retains `runtime/`: it contains
            // Arti directory cache and onion state, so a client-data reset
            // does not turn into an unnecessary cold Tor bootstrap.
            ClientDataPolicy::ResetProfile => {
                for name in ["data", "protected-secrets"] {
                    self.move_to_backup(&root.join(name), device_id)?;
                }
            }
            ClientDataPolicy::ResetAll => self.move_to_backup(root, device_id)?,
            ClientDataPolicy::Preserve => {}
        }
        Ok(())
    }

    fn reset_android(&self, device: &Device, policy: ClientDataPolicy) -> Result<(), DataError> {
        // Resets run before installation in a fresh deployment.  In that
        // case the package legitimately does not exist yet; there is no
        // profile to clear and `run-as` would fail with "unknown package".
        // Probe package presence once and treat an absent package as an
        // already-reset profile.  Other adb/package-manager failures remain
        // hard errors so a disconnected device is never reported as reset.
        let package =
            self.command(&device.id, &["shell", "pm", "list", "packages", "com.torca.torca_app"])?;
        if !package.success {
            return Err(DataError::Command(package.text));
        }
        // `pm list packages` returns success with no output when the package
        // is not installed. This is the normal state before a fresh install.
        if package.text.trim().is_empty() {
            return Ok(());
        }
        match policy {
            // The Android host keeps profile data below no_backup/torca while
            // Arti's reusable state is below no_backup/torca/runtime.  `pm
            // clear` removes both; use the app UID for the narrow reset.
            ClientDataPolicy::ResetProfile => {
                let output = self.command(
                    &device.id,
                    &[
                        "shell",
                        "run-as",
                        "com.torca.torca_app",
                        "rm",
                        "-rf",
                        "no_backup/torca/data",
                        "no_backup/torca/protected-secrets",
                    ],
                )?;
                if output.success { Ok(()) } else { Err(DataError::Command(output.text)) }
            }
            ClientDataPolicy::ResetAll => {
                let output =
                    self.command(&device.id, &["shell", "pm", "clear", "com.torca.torca_app"])?;
                if output.success { Ok(()) } else { Err(DataError::Command(output.text)) }
            }
            ClientDataPolicy::Preserve => Ok(()),
        }
    }

    fn move_to_backup(&self, source: &std::path::Path, device_id: &str) -> Result<(), DataError> {
        if !source.exists() {
            return Ok(());
        }
        let parent = source.parent().unwrap_or(&self.paths.runtime_root);
        let base = if source.file_name().is_some_and(|name| name == std::ffi::OsStr::new("Torca")) {
            parent
        } else {
            parent.parent().unwrap_or(parent)
        };
        let backup_root = base.join("Torca-backups");
        std::fs::create_dir_all(&backup_root).map_err(DataError::Io)?;
        let backup = backup_root.join(format!(
            "{}-{}-{}",
            chrono_stamp(),
            device_id,
            source.file_name().and_then(|name| name.to_str()).unwrap_or("data")
        ));
        std::fs::rename(source, backup).map_err(DataError::Io)
    }

    fn command(
        &self,
        device: &str,
        arguments: &[&str],
    ) -> Result<crate::process::CommandOutput, DataError> {
        self.runner
            .run(&CommandSpec {
                program: "adb".into(),
                arguments: std::iter::once("-s".into())
                    .chain(std::iter::once(device.into()))
                    .chain(arguments.iter().map(|argument| (*argument).into()))
                    .collect(),
                working_directory: self.paths.repo_root.clone(),
                timeout: Duration::from_secs(60),
                environment: std::collections::BTreeMap::new(),
            })
            .map_err(DataError::Process)
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
    #[error("Windows client stop failed: {0}")]
    WindowsClient(#[from] crate::windows_client::WindowsClientError),
}

fn chrono_stamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devices::AndroidAbi;
    use crate::domain::Target;
    use crate::process::{CommandOutput, CommandSpec};
    use std::sync::Mutex;

    #[derive(Default)]
    struct RecordingRunner {
        commands: Mutex<Vec<CommandSpec>>,
    }

    impl CommandRunner for RecordingRunner {
        fn run(&self, command: &CommandSpec) -> Result<CommandOutput, ProcessError> {
            self.commands.lock().expect("commands").push(command.clone());
            let text = if command.arguments.ends_with(&[
                "shell".into(),
                "pm".into(),
                "list".into(),
                "packages".into(),
                "com.torca.torca_app".into(),
            ]) {
                "package:/data/app/com.torca.torca_app/base.apk\n".into()
            } else {
                String::new()
            };
            Ok(CommandOutput { success: true, status: Some(0), text })
        }
    }

    #[test]
    fn android_profile_reset_passes_rm_operands_directly() {
        let root = std::env::temp_dir().join(format!("torca-data-test-{}", std::process::id()));
        let paths = RuntimePaths::from_repo(root.clone());
        let runner = RecordingRunner::default();
        let device = Device {
            target: Target::Android,
            id: "phone".into(),
            android_abi: Some(AndroidAbi::Arm64),
        };

        DataController::new(&paths, &runner)
            .reset(&[device], ClientDataPolicy::ResetProfile)
            .expect("profile reset");

        let commands = runner.commands.lock().expect("commands");
        assert_eq!(commands.len(), 3);
        assert_eq!(
            commands[2].arguments,
            [
                "-s",
                "phone",
                "shell",
                "run-as",
                "com.torca.torca_app",
                "rm",
                "-rf",
                "no_backup/torca/data",
                "no_backup/torca/protected-secrets",
            ]
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
