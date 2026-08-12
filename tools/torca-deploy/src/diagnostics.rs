use crate::paths::RuntimePaths;
use crate::process::{CommandRunner, CommandSpec};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct DiagnosticsReport {
    pub root: PathBuf,
    pub files: Vec<PathBuf>,
}

pub fn collect_runtime(
    paths: &RuntimePaths,
    runner: &dyn CommandRunner,
    android_devices: &[String],
) -> DiagnosticsReport {
    let _ = std::fs::create_dir_all(&paths.relay_logs);
    if paths.docker_compose.is_file() {
        if let Ok(output) = runner.run(&CommandSpec {
            program: "docker".into(),
            arguments: vec![
                "compose".into(),
                "-f".into(),
                paths.docker_compose.display().to_string(),
                "logs".into(),
                "--no-color".into(),
                "--tail".into(),
                "300".into(),
                "relay".into(),
            ],
            working_directory: paths.repo_root.clone(),
            timeout: Duration::from_secs(60),
            environment: std::collections::BTreeMap::new(),
        }) {
            let _ = std::fs::write(paths.relay_logs.join("relay.log"), output.text);
        }
    }
    collect_windows_native_logs(paths);
    for device in android_devices {
        collect_android_native_logs(paths, runner, device);
        let mut arguments = vec!["-s".into(), device.clone(), "logcat".into(), "-d".into()];
        if let Some(process_id) = android_process_id(runner, paths, device) {
            arguments.extend(["--pid".into(), process_id]);
        }
        arguments.extend(["-t".into(), "500".into()]);
        if let Ok(output) = runner.run(&CommandSpec {
            program: "adb".into(),
            arguments,
            working_directory: paths.repo_root.clone(),
            timeout: Duration::from_secs(60),
            environment: std::collections::BTreeMap::new(),
        }) {
            let _ =
                std::fs::write(paths.relay_logs.join(format!("android-{device}.log")), output.text);
        }
    }
    DiagnosticsReport::collect(&paths.relay_logs)
}

const ANDROID_PACKAGE: &str = "com.torca.torca_app";
const ANDROID_LOG_ROOT: &str = "/sdcard/Android/data/com.torca.torca_app/files/torca/logs";

fn collect_windows_native_logs(paths: &RuntimePaths) {
    let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") else { return };
    let source = PathBuf::from(local_app_data).join("Torca").join("logs");
    copy_tree(&source, &paths.relay_logs.join("windows-native"));
}

fn collect_android_native_logs(paths: &RuntimePaths, runner: &dyn CommandRunner, device: &str) {
    let target = paths.relay_logs.join(format!("android-{device}")).join("native");
    let listing = runner.run(&CommandSpec {
        program: "adb".into(),
        arguments: vec![
            "-s".into(),
            device.into(),
            "shell".into(),
            "find".into(),
            ANDROID_LOG_ROOT.into(),
            "-type".into(),
            "f".into(),
            "\\(".into(),
            "-name".into(),
            "*.log".into(),
            "-o".into(),
            "-name".into(),
            "*.json".into(),
            "\\)".into(),
        ],
        working_directory: paths.repo_root.clone(),
        timeout: Duration::from_secs(30),
        environment: std::collections::BTreeMap::new(),
    });
    let Ok(listing) = listing else { return };
    for source in listing.text.lines().filter(|line| line.starts_with(ANDROID_LOG_ROOT)) {
        let relative =
            source.strip_prefix(ANDROID_LOG_ROOT).unwrap_or_default().trim_start_matches('/');
        let destination = target.join(relative);
        if let Some(parent) = destination.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(output) = runner.run(&CommandSpec {
            program: "adb".into(),
            arguments: vec![
                "-s".into(),
                device.into(),
                "exec-out".into(),
                "cat".into(),
                source.into(),
            ],
            working_directory: paths.repo_root.clone(),
            timeout: Duration::from_secs(30),
            environment: std::collections::BTreeMap::new(),
        }) {
            let _ = fs::write(destination, output.text);
        }
    }
}

fn android_process_id(
    runner: &dyn CommandRunner,
    paths: &RuntimePaths,
    device: &str,
) -> Option<String> {
    runner
        .run(&CommandSpec {
            program: "adb".into(),
            arguments: vec![
                "-s".into(),
                device.into(),
                "shell".into(),
                "pidof".into(),
                ANDROID_PACKAGE.into(),
            ],
            working_directory: paths.repo_root.clone(),
            timeout: Duration::from_secs(10),
            environment: std::collections::BTreeMap::new(),
        })
        .ok()
        .and_then(|output| output.text.split_whitespace().next().map(str::to_owned))
}

impl DiagnosticsReport {
    pub fn collect(root: impl AsRef<Path>) -> Self {
        let root = root.as_ref().to_path_buf();
        let mut files = Vec::new();
        collect_files(&root, &mut files);
        files.sort();
        Self { root, files }
    }

    pub fn summary(&self) -> String {
        format!("{} diagnostic files under {}", self.files.len(), self.root.display())
    }
}

fn copy_tree(source: &Path, destination: &Path) {
    let Ok(entries) = fs::read_dir(source) else { return };
    for entry in entries.flatten() {
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if source_path.is_dir() {
            copy_tree(&source_path, &destination_path);
        } else if source_path.is_file() {
            if let Some(parent) = destination_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::copy(source_path, destination_path);
        }
    }
}

fn collect_files(path: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(path) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, files);
        } else if path.is_file() {
            files.push(path);
        }
    }
}
