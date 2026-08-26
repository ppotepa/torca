use crate::devices::Device;
use crate::domain::{CommunicationProvider, Configuration, PrivacyPolicy, ValidationLevel};
use crate::paths::RuntimePaths;
use crate::process::{CommandRunner, CommandSpec, ProcessError};
use crate::windows_client::{WindowsClientError, WorkspaceWindowsClient};
use serde_json::Value;
use std::time::{Duration, SystemTime};
use thiserror::Error;

fn android_package() -> &'static str {
    crate::android_target::package()
}

fn android_activity() -> &'static str {
    crate::android_target::activity()
}

fn android_logs_root() -> &'static str {
    crate::android_target::logs_root()
}

pub struct LaunchController<'a> {
    paths: &'a RuntimePaths,
    runner: &'a dyn CommandRunner,
}

#[derive(Clone, Copy, Debug)]
pub struct LaunchReceipt {
    started_at: SystemTime,
}

impl LaunchReceipt {
    pub(crate) const fn from_started_at(started_at: SystemTime) -> Self {
        Self { started_at }
    }
}

impl<'a> LaunchController<'a> {
    pub fn new(paths: &'a RuntimePaths, runner: &'a dyn CommandRunner) -> Self {
        Self { paths, runner }
    }
    pub fn launch(
        &self,
        device: &Device,
        configuration: Configuration,
        privacy: PrivacyPolicy,
        communication_provider: CommunicationProvider,
        provider_profile: Option<&str>,
        restart: bool,
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
                    communication_provider,
                    provider_profile,
                )
                .map_err(LaunchError::ArtifactVerification)?;
                WorkspaceWindowsClient::new(self.paths, self.runner).stop()?;
                spawn_windows_detached(&exe, self.paths, self.runner)?;
                Ok(LaunchReceipt { started_at })
            }
            crate::domain::Target::Android => {
                if restart {
                    let stopped = self.runner.run(&CommandSpec {
                        program: "adb".into(),
                        arguments: vec![
                            "-s".into(),
                            device.id.clone(),
                            "shell".into(),
                            "am".into(),
                            "force-stop".into(),
                            android_package().into(),
                        ],
                        working_directory: self.paths.repo_root.clone(),
                        timeout: Duration::from_secs(15),
                        environment: std::collections::BTreeMap::new(),
                    })?;
                    if !stopped.success {
                        return Err(LaunchError::Command(stopped.text));
                    }
                }
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
                        android_activity().into(),
                        "-a".into(),
                        "android.intent.action.MAIN".into(),
                        "-c".into(),
                        "android.intent.category.LAUNCHER".into(),
                        "--ez".into(),
                        "torca.allow_screen_capture".into(),
                        matches!(privacy, PrivacyPolicy::AllowCapture).to_string(),
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
                        android_package().into(),
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
        Err(LaunchError::HealthTimeout {
            device: device.id.clone(),
            reason: "process did not become running".into(),
        })
    }

    pub fn wait_network_ready(
        &self,
        device: &Device,
        receipt: LaunchReceipt,
        validation: ValidationLevel,
        communication_provider: CommunicationProvider,
    ) -> Result<(), LaunchError> {
        if matches!(validation, ValidationLevel::Skip) {
            return Ok(());
        }
        // Quick validation proves that the newly launched app has a live
        // local runtime. It never waits for provider-specific remote work.
        // Full validation waits for provider-owned commissioning only when
        // the profile actually requires an external endpoint/service. Direct
        // providers (for example Iroh) are complete once their local runtime
        // is ready and must never inherit a Tor-style service gate.
        let require_service = matches!(validation, ValidationLevel::Full)
            && communication_provider.deployment_profile().requires_service_readiness();
        // The selected provider owns its commissioning budget. Tor may need
        // a longer managed-directory window; Iroh/WebRTC must fail fast so a
        // transient route problem cannot leave the deploy wizard waiting for
        // minutes on a local runtime that is already usable.
        let timeout = if require_service {
            communication_provider.deployment_profile().service_validation_timeout.as_secs()
        } else {
            45
        };
        let deadline = std::time::Instant::now() + Duration::from_secs(timeout);
        let started = std::time::Instant::now();
        let mut next_heartbeat = started;
        let mut consecutive_ready = 0_u8;
        while std::time::Instant::now() < deadline {
            if !self.process_is_running(device)? {
                return Err(LaunchError::ProcessExited(device.id.clone()));
            }
            let ready = match device.target {
                crate::domain::Target::Windows => Self::windows_network_ready(
                    receipt.started_at,
                    communication_provider,
                    require_service,
                ),
                crate::domain::Target::Android => self.android_network_ready(
                    &device.id,
                    receipt.started_at,
                    communication_provider,
                    require_service,
                ),
            }?;
            if ready {
                consecutive_ready += 1;
                // Two observations avoid accepting one stale/transient log
                // line, while keeping the quick path bounded and responsive.
                if consecutive_ready >= 2 {
                    return Ok(());
                }
            } else {
                consecutive_ready = 0;
            }
            if std::time::Instant::now() >= next_heartbeat {
                eprintln!(
                    "torca-deploy: waiting for {} {} elapsed_s={}",
                    device.id,
                    if require_service {
                        communication_provider.protocol_label()
                    } else {
                        "LOCAL_READY"
                    },
                    started.elapsed().as_secs()
                );
                next_heartbeat = std::time::Instant::now() + Duration::from_secs(10);
            }
            std::thread::sleep(Duration::from_secs(1));
        }
        let reason = match device.target {
            crate::domain::Target::Android => {
                self.android_readiness_diagnostic(&device.id).unwrap_or_else(|_| {
                    format!(
                        "missing fresh Flutter gateway handshake (provider={}, required={})",
                        communication_provider,
                        if require_service { "provider service" } else { "local runtime" },
                    )
                })
            }
            crate::domain::Target::Windows => format!(
                "missing fresh Flutter gateway handshake (provider={}, required={})",
                communication_provider,
                if require_service { "provider service" } else { "local runtime" },
            ),
        };
        Err(LaunchError::HealthTimeout { device: device.id.clone(), reason })
    }

    /// Proves that launch made a user-visible surface, rather than merely
    /// leaving a background process alive. This is deliberately checked
    /// before runtime health: a stale process or log must never make deploy
    /// report a successful launch when the user is still looking at Home,
    /// Recents, or no Torca window.
    pub fn wait_visible_surface(&self, device: &Device) -> Result<(), LaunchError> {
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        // Android may acknowledge `am start` while another task still owns
        // the foreground (Maps, a permission dialog, or a stale launcher
        // task).  Re-issuing the idempotent start once makes the launch
        // contract deterministic without force-stopping the app or touching
        // its data.  This is especially common with wireless ADB transports.
        if matches!(device.target, crate::domain::Target::Android) {
            self.bring_android_to_front(&device.id)?;
        }
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

    fn bring_android_to_front(&self, device: &str) -> Result<(), LaunchError> {
        let output = self.command(
            "adb",
            &[
                "-s",
                device,
                "shell",
                "am",
                "start",
                "-W",
                "-n",
                android_activity(),
                "-a",
                "android.intent.action.MAIN",
                "-c",
                "android.intent.category.LAUNCHER",
            ],
        )?;
        if output.success { Ok(()) } else { Err(LaunchError::Command(output.text)) }
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
                    android_package().into(),
                ],
            ),
        };
        let output = self.runner.run_quiet(&CommandSpec {
            program: program.into(),
            arguments,
            working_directory: self.paths.repo_root.clone(),
            timeout: Duration::from_secs(15),
            environment: std::collections::BTreeMap::new(),
        })?;
        Ok(output.success && !output.text.trim().is_empty() && !output.text.contains("No tasks"))
    }

    fn windows_network_ready(
        launched_at: SystemTime,
        provider: CommunicationProvider,
        require_service: bool,
    ) -> Result<bool, LaunchError> {
        let Some(local) = std::env::var_os("LOCALAPPDATA") else {
            return Ok(false);
        };
        let root = std::path::PathBuf::from(local).join("Torca/logs/devices");
        let mut logs = Vec::new();
        collect_named(&root, "bootstrap.log", &mut logs);
        // Keep Tor as a backward-compatible readiness source for clients
        // built before LOCAL_READY was introduced.
        collect_named(&root, "tor.log", &mut logs);
        logs.sort_by_key(|path| std::fs::metadata(path).and_then(|m| m.modified()).ok());
        Ok(logs.iter().rev().any(|path| {
            file_is_fresh(path, launched_at)
                && read_readiness_evidence(path, provider, require_service)
        }))
    }

    fn android_network_ready(
        &self,
        device: &str,
        launched_at: SystemTime,
        provider: CommunicationProvider,
        require_service: bool,
    ) -> Result<bool, LaunchError> {
        let files = self
            .command("adb", &["-s", device, "shell", "find", android_logs_root(), "-type", "f"])?;
        let paths = latest_android_health_logs(&files.text);
        if paths.is_empty() {
            return Ok(false);
        }
        let mut arguments = vec!["-s", device, "shell", "tail", "-n", "120"];
        arguments.extend(paths);
        let output = self.command("adb", &arguments)?;
        Ok(file_is_fresh_from_log(&output.text, launched_at)
            && output.success
            && read_readiness_evidence_from_content(&output.text, provider, require_service))
    }

    fn android_readiness_diagnostic(&self, device: &str) -> Result<String, LaunchError> {
        let files = self
            .command("adb", &["-s", device, "shell", "find", android_logs_root(), "-type", "f"])?;
        let paths = latest_android_health_logs(&files.text);
        if paths.is_empty() {
            return Ok("no current Android runtime logs were found".into());
        }
        let mut arguments = vec!["-s", device, "shell", "tail", "-n", "160"];
        arguments.extend(paths);
        let output = self.command("adb", &arguments)?;
        let codes = [
            "RUNTIME_ACTOR_PANICKED",
            "RUNTIME_START_FAILED",
            "SNAPSHOT_REFRESH_FAILED",
            "RUNTIME_SHUTDOWN_REQUESTED",
            "TORCA_NATIVE_STARTUP_FAILED",
        ];
        for line in output.text.lines().rev() {
            if let Ok(value) = serde_json::from_str::<Value>(line)
                && value
                    .get("code")
                    .and_then(Value::as_str)
                    .is_some_and(|code| codes.contains(&code))
            {
                let code = value.get("code").and_then(Value::as_str).unwrap_or("unknown");
                let message = value.get("message").and_then(Value::as_str).unwrap_or("");
                return Ok(format!("{code}: {message}"));
            }
        }
        Ok("fresh readiness evidence was not emitted; inspect the current runtime/ffi logs".into())
    }

    fn command(
        &self,
        program: &str,
        arguments: &[&str],
    ) -> Result<crate::process::CommandOutput, LaunchError> {
        self.runner
            .run_quiet(&CommandSpec {
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
                    && line.contains(android_package())
            }))
    }
}

fn file_is_fresh(path: &std::path::Path, launched_at: SystemTime) -> bool {
    std::fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .is_ok_and(|modified| modified >= launched_at)
}

fn latest_android_health_logs(content: &str) -> Vec<&str> {
    let latest =
        |suffix: &str| content.lines().map(str::trim).filter(|path| path.ends_with(suffix)).max();
    let mut paths = latest("/bootstrap.log").into_iter().collect::<Vec<_>>();
    for suffix in ["/tor.log", "/runtime.log", "/ffi.log", "/communication.log"] {
        if let Some(path) = latest(suffix) {
            paths.push(path);
        }
    }
    paths
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

/// The deployer used to accept LOCAL_READY as the complete client health
/// contract. That event is intentionally emitted before Flutter starts, so a
/// crashed/mismatched gateway could still produce a green deployment. Require
/// the application handshake as a separate piece of evidence; provider
/// service readiness remains an additional condition for full validation.
fn read_readiness_evidence(
    path: &std::path::Path,
    provider: CommunicationProvider,
    require_service: bool,
) -> bool {
    std::fs::read_to_string(path).ok().is_some_and(|content| {
        read_readiness_evidence_from_content(&content, provider, require_service)
    })
}

fn read_readiness_evidence_from_content(
    content: &str,
    provider: CommunicationProvider,
    require_service: bool,
) -> bool {
    let mut gateway_ready = false;
    let mut local_ready = false;
    let mut service_ready = false;
    for value in
        content.lines().rev().take(256).filter_map(|line| serde_json::from_str::<Value>(line).ok())
    {
        let code = value.get("code").and_then(Value::as_str);
        gateway_ready |= code == Some("FLUTTER_GATEWAY_READY");
        local_ready |= ready_code(code, provider, false);
        service_ready |= ready_code(code, provider, true);
    }
    gateway_ready && local_ready && (!require_service || service_ready)
}

fn ready_code(code: Option<&str>, provider: CommunicationProvider, require_service: bool) -> bool {
    let profile = provider.deployment_profile();
    let accepted =
        if require_service { profile.service_ready_codes } else { profile.local_ready_codes };
    code.is_some_and(|code| accepted.contains(&code))
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
        .map_err(LaunchError::Io)
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
    #[error("client process did not become healthy on {device}: {reason}")]
    HealthTimeout { device: String, reason: String },
    #[error("client process exited before its fresh provider-readiness event on {0}")]
    ProcessExited(String),
    #[error("workspace Windows client operation failed: {0}")]
    WindowsClient(#[from] WindowsClientError),
    #[error("client started but did not expose a visible application surface on {0}")]
    VisibleSurfaceTimeout(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn android_health_uses_only_latest_relevant_run_logs() {
        let files = concat!(
            "/logs/2026-08-13/run-000001/bootstrap.log\n",
            "/logs/2026-08-13/run-000001/tor.log\n",
            "/logs/2026-08-14/run-000002/bootstrap.log\n",
            "/logs/2026-08-14/run-000002/tor.log\n",
            "/logs/2026-08-14/run-000002/runtime.log\n",
        );
        assert_eq!(
            latest_android_health_logs(files),
            vec![
                "/logs/2026-08-14/run-000002/bootstrap.log",
                "/logs/2026-08-14/run-000002/tor.log",
                "/logs/2026-08-14/run-000002/runtime.log",
            ]
        );
    }

    #[test]
    fn quick_and_full_validation_have_distinct_readiness_contracts() {
        assert!(ready_code(Some("LOCAL_READY"), CommunicationProvider::Tor, false));
        assert!(!ready_code(Some("TOR_STARTING"), CommunicationProvider::Tor, false));
        assert!(!ready_code(Some("COMMUNICATION_STARTING"), CommunicationProvider::Iroh, false));
        assert!(!ready_code(Some("TOR_STARTING"), CommunicationProvider::Tor, true));
        assert!(!ready_code(Some("LOCAL_READY"), CommunicationProvider::Tor, true));
        assert!(ready_code(Some("TOR_BOOTSTRAP_READY"), CommunicationProvider::Tor, true));
        assert!(ready_code(Some("COMMUNICATION_READY"), CommunicationProvider::Iroh, true));
        assert!(ready_code(Some("COMMUNICATION_READY"), CommunicationProvider::WebRtc, true));
        assert!(!ready_code(Some("TOR_BOOTSTRAP_READY"), CommunicationProvider::WebRtc, true));
    }

    #[test]
    fn client_readiness_requires_flutter_gateway_handshake() {
        let local_only = r#"
            {"code":"LOCAL_READY"}
        "#;
        assert!(!read_readiness_evidence_from_content(
            local_only,
            CommunicationProvider::Iroh,
            false,
        ));
        let ready = r#"
            {"code":"LOCAL_READY"}
            {"code":"COMMUNICATION_READY"}
            {"code":"FLUTTER_GATEWAY_READY"}
        "#;
        assert!(read_readiness_evidence_from_content(ready, CommunicationProvider::Iroh, false,));
        assert!(read_readiness_evidence_from_content(ready, CommunicationProvider::Iroh, true,));
    }

    #[test]
    fn android_readiness_rejects_stale_or_malformed_log_entries() {
        let launched_at = SystemTime::UNIX_EPOCH + Duration::from_millis(2_000);
        let stale = r#"{"ts_ms":1999,"code":"LOCAL_READY"}"#;
        let malformed = "not-json\n{\"code\":\"LOCAL_READY\"}";
        let fresh = r#"{"ts_ms":2000,"code":"LOCAL_READY"}"#;

        assert!(!file_is_fresh_from_log(stale, launched_at));
        assert!(!file_is_fresh_from_log(malformed, launched_at));
        assert!(file_is_fresh_from_log(fresh, launched_at));
    }
}
