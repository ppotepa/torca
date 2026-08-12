use crate::devices::Device;
use crate::domain::Configuration;
use crate::paths::RuntimePaths;
use crate::process::{CommandRunner, CommandSpec, ProcessError};
use crate::windows_client::{WindowsClientError, WorkspaceWindowsClient};
use serde_json::Value;
use std::time::{Duration, SystemTime};
use thiserror::Error;

const ANDROID_PACKAGE: &str = "com.torca.torca_app";
const ANDROID_ACTIVITY: &str = "com.torca.torca_app/com.torca.app.MainActivity";

pub struct LaunchController<'a> {
    paths: &'a RuntimePaths,
    runner: &'a dyn CommandRunner,
}

#[derive(Clone, Copy, Debug)]
pub struct LaunchReceipt {
    started_at: SystemTime,
}

impl<'a> LaunchController<'a> {
    pub fn new(paths: &'a RuntimePaths, runner: &'a dyn CommandRunner) -> Self {
        Self { paths, runner }
    }
    pub fn launch(
        &self,
        device: &Device,
        configuration: Configuration,
    ) -> Result<LaunchReceipt, LaunchError> {
        let started_at = SystemTime::now();
        match device.target {
            crate::domain::Target::Windows => {
                let mode = match configuration {
                    Configuration::Debug => "Debug",
                    Configuration::Release => "Release",
                };
                let exe = self.paths.repo_root.join(format!(
                    "apps/client/flutter/build/windows/x64/runner/{mode}/torca_app.exe"
                ));
                if !exe.is_file() {
                    return Err(LaunchError::Artifact(exe));
                }
                crate::build::verify_artifact_manifest(
                    self.paths,
                    crate::domain::Target::Windows,
                    configuration,
                    &exe,
                )
                .map_err(LaunchError::ArtifactVerification)?;
                WorkspaceWindowsClient::new(self.paths, self.runner).stop()?;
                spawn_windows_detached(&exe, self.paths, self.runner)?;
                Ok(LaunchReceipt { started_at })
            }
            crate::domain::Target::Android => {
                let output = self.runner.run(&CommandSpec {
                    program: "adb".into(),
                    arguments: vec![
                        "-s".into(),
                        device.id.clone(),
                        "shell".into(),
                        "am".into(),
                        "start".into(),
                        "-W".into(),
                        "-n".into(),
                        ANDROID_ACTIVITY.into(),
                        "-a".into(),
                        "android.intent.action.MAIN".into(),
                        "-c".into(),
                        "android.intent.category.LAUNCHER".into(),
                    ],
                    working_directory: self.paths.repo_root.clone(),
                    timeout: Duration::from_secs(30),
                    environment: std::collections::BTreeMap::new(),
                })?;
                if output.success {
                    Ok(LaunchReceipt { started_at })
                } else {
                    Err(LaunchError::Command(output.text))
                }
            }
        }
    }

    pub fn wait_process(&self, device: &Device) -> Result<(), LaunchError> {
        let deadline = std::time::Instant::now() + Duration::from_secs(90);
        while std::time::Instant::now() < deadline {
            if matches!(device.target, crate::domain::Target::Windows) {
                if WorkspaceWindowsClient::new(self.paths, self.runner).is_running()? {
                    return Ok(());
                }
                std::thread::sleep(Duration::from_secs(1));
                continue;
            }
            let (program, arguments) = match device.target {
                crate::domain::Target::Windows => {
                    unreachable!("workspace Windows branch handled above")
                }
                crate::domain::Target::Android => (
                    "adb",
                    vec![
                        "-s".into(),
                        device.id.clone(),
                        "shell".into(),
                        "pidof".into(),
                        "com.torca.torca_app".into(),
                    ],
                ),
            };
            let output = self.runner.run(&CommandSpec {
                program: program.into(),
                arguments,
                working_directory: self.paths.repo_root.clone(),
                timeout: Duration::from_secs(15),
                environment: std::collections::BTreeMap::new(),
            })?;
            if output.success && !output.text.trim().is_empty() && !output.text.contains("No tasks")
            {
                return Ok(());
            }
            std::thread::sleep(Duration::from_secs(1));
        }
        Err(LaunchError::HealthTimeout(device.id.clone()))
    }

    pub fn wait_network_ready(
        &self,
        device: &Device,
        receipt: LaunchReceipt,
    ) -> Result<(), LaunchError> {
        let deadline = std::time::Instant::now() + Duration::from_secs(180);
        let started = std::time::Instant::now();
        let mut next_heartbeat = started;
        while std::time::Instant::now() < deadline {
            if !self.process_is_running(device)? {
                return Err(LaunchError::ProcessExited(device.id.clone()));
            }
            let ready = match device.target {
                crate::domain::Target::Windows => Self::windows_network_ready(receipt.started_at),
                crate::domain::Target::Android => {
                    self.android_network_ready(&device.id, receipt.started_at)
                }
            }?;
            if ready {
                return Ok(());
            }
            if std::time::Instant::now() >= next_heartbeat {
                eprintln!(
                    "torca-deploy: waiting for {} NETWORK_READY elapsed_s={}",
                    device.id,
                    started.elapsed().as_secs()
                );
                next_heartbeat = std::time::Instant::now() + Duration::from_secs(10);
            }
            std::thread::sleep(Duration::from_millis(500));
        }
        Err(LaunchError::HealthTimeout(device.id.clone()))
    }

    /// Proves that launch made a user-visible surface, rather than merely
    /// leaving a background process alive. This is deliberately checked
    /// before runtime health: a stale process or log must never make deploy
    /// report a successful launch when the user is still looking at Home,
    /// Recents, or no Torca window.
    pub fn wait_visible_surface(&self, device: &Device) -> Result<(), LaunchError> {
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        while std::time::Instant::now() < deadline {
            let visible = match device.target {
                crate::domain::Target::Windows => {
                    WorkspaceWindowsClient::new(self.paths, self.runner)
                        .activate_visible_window()?
                }
                crate::domain::Target::Android => self.android_activity_visible(&device.id)?,
            };
            if visible {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(500));
        }
        Err(LaunchError::VisibleSurfaceTimeout(device.id.clone()))
    }

    fn process_is_running(&self, device: &Device) -> Result<bool, LaunchError> {
        let (program, arguments) = match device.target {
            crate::domain::Target::Windows => {
                return WorkspaceWindowsClient::new(self.paths, self.runner)
                    .is_running()
                    .map_err(LaunchError::WindowsClient);
            }
            crate::domain::Target::Android => (
                "adb",
                vec![
                    "-s".into(),
                    device.id.clone(),
                    "shell".into(),
                    "pidof".into(),
                    ANDROID_PACKAGE.into(),
                ],
            ),
        };
        let output = self.runner.run(&CommandSpec {
            program: program.into(),
            arguments,
            working_directory: self.paths.repo_root.clone(),
            timeout: Duration::from_secs(15),
            environment: std::collections::BTreeMap::new(),
        })?;
        Ok(output.success && !output.text.trim().is_empty() && !output.text.contains("No tasks"))
    }

    fn windows_network_ready(launched_at: SystemTime) -> Result<bool, LaunchError> {
        let Some(local) = std::env::var_os("LOCALAPPDATA") else {
            return Ok(false);
        };
        let root = std::path::PathBuf::from(local).join("Torca/logs/devices");
        let mut logs = Vec::new();
        collect_named(&root, "bootstrap.log", &mut logs);
        logs.sort_by_key(|path| std::fs::metadata(path).and_then(|m| m.modified()).ok());
        let Some(path) = logs.last() else {
            return Ok(false);
        };
        Ok(file_is_fresh(path, launched_at) && read_network_ready(path))
    }

    fn android_network_ready(
        &self,
        device: &str,
        launched_at: SystemTime,
    ) -> Result<bool, LaunchError> {
        let files = self.command(
            "adb",
            &[
                "-s",
                device,
                "shell",
                "find",
                "/sdcard/Android/data/com.torca.torca_app/files/torca/logs",
                "-type",
                "f",
                "-name",
                "bootstrap.log",
            ],
        )?;
        let path = files.text.lines().last().unwrap_or_default().trim();
        if path.is_empty() {
            return Ok(false);
        }
        let output = self.command("adb", &["-s", device, "shell", "tail", "-n", "120", path])?;
        Ok(file_is_fresh_from_log(&output.text, launched_at)
            && output.success
            && output.text.lines().filter_map(|line| serde_json::from_str::<Value>(line).ok()).any(
                |value| {
                    matches!(
                        value.get("code").and_then(Value::as_str),
                        Some("LOCAL_READY") | Some("TOR_BOOTSTRAP_READY") | Some("NETWORK_READY")
                    )
                },
            ))
    }

    fn command(
        &self,
        program: &str,
        arguments: &[&str],
    ) -> Result<crate::process::CommandOutput, LaunchError> {
        self.runner
            .run(&CommandSpec {
                program: program.into(),
                arguments: arguments.iter().map(|x| (*x).into()).collect(),
                working_directory: self.paths.repo_root.clone(),
                timeout: Duration::from_secs(30),
                environment: std::collections::BTreeMap::new(),
            })
            .map_err(LaunchError::Process)
    }

    fn android_activity_visible(&self, device: &str) -> Result<bool, LaunchError> {
        let output =
            self.command("adb", &["-s", device, "shell", "dumpsys", "activity", "activities"])?;
        // `dumpsys activity activities` contains history too. Accept only the
        // current focus/resumed records, never a historical Torca activity.
        Ok(output.success
            && output.text.lines().any(|line| {
                (line.contains("mResumedActivity")
                    || line.contains("ResumedActivity:")
                    || line.contains("mCurrentFocus"))
                    && line.contains(ANDROID_PACKAGE)
            }))
    }
}

fn file_is_fresh(path: &std::path::Path, launched_at: SystemTime) -> bool {
    std::fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .is_ok_and(|modified| modified >= launched_at)
}

fn file_is_fresh_from_log(content: &str, launched_at: SystemTime) -> bool {
    let launched_ms = launched_at
        .duration_since(SystemTime::UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_millis()).ok());
    let Some(launched_ms) = launched_ms else { return false };
    content
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter_map(|value| value.get("ts_ms").and_then(Value::as_i64))
        .any(|timestamp| timestamp >= launched_ms)
}

fn collect_named(root: &std::path::Path, name: &str, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_named(&path, name, out);
        } else if path.file_name().is_some_and(|value| value == name) {
            out.push(path);
        }
    }
}

fn read_network_ready(path: &std::path::Path) -> bool {
    let Ok(content) = std::fs::read_to_string(path) else {
        return false;
    };
    content.lines().rev().take(120).filter_map(|line| serde_json::from_str::<Value>(line).ok()).any(
        |value| {
            matches!(
                value.get("code").and_then(Value::as_str),
                Some("LOCAL_READY") | Some("TOR_BOOTSTRAP_READY") | Some("NETWORK_READY")
            )
        },
    )
}

#[cfg(windows)]
fn spawn_windows_detached(
    executable: &std::path::Path,
    paths: &RuntimePaths,
    _runner: &dyn CommandRunner,
) -> Result<(), LaunchError> {
    use std::process::{Command, Stdio};
    Command::new(executable)
        .current_dir(executable.parent().unwrap_or(&paths.repo_root))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|error| LaunchError::Io(error))
}

#[cfg(not(windows))]
fn spawn_windows_detached(
    executable: &std::path::Path,
    paths: &RuntimePaths,
    runner: &dyn CommandRunner,
) -> Result<(), LaunchError> {
    let output = runner.run(&CommandSpec {
        program: executable.display().to_string(),
        arguments: Vec::new(),
        working_directory: executable.parent().unwrap_or(&paths.repo_root).to_path_buf(),
        timeout: Duration::from_secs(30),
        environment: std::collections::BTreeMap::new(),
    })?;
    if output.success { Ok(()) } else { Err(LaunchError::Command(output.text)) }
}
#[derive(Debug, Error)]
pub enum LaunchError {
    #[error("launch artifact missing: {0}")]
    Artifact(std::path::PathBuf),
    #[error("launch failed: {0}")]
    Command(String),
    #[error("artifact verification failed: {0}")]
    ArtifactVerification(String),
    #[error("launch I/O failed: {0}")]
    Io(std::io::Error),
    #[error("launch process error: {0}")]
    Process(#[from] ProcessError),
    #[error("client process did not become healthy on {0}")]
    HealthTimeout(String),
    #[error("client process exited before its fresh NETWORK_READY event on {0}")]
    ProcessExited(String),
    #[error("workspace Windows client operation failed: {0}")]
    WindowsClient(#[from] WindowsClientError),
    #[error("client started but did not expose a visible application surface on {0}")]
    VisibleSurfaceTimeout(String),
}
