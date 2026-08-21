use crate::domain::Target;
use crate::paths::RuntimePaths;
use crate::process::{CommandRunner, CommandSpec, ProcessError};
use std::io::{self, IsTerminal, Write};
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

/// Restrict discovered devices to an exact user-requested id.
pub fn select_device(
    devices: Vec<Device>,
    requested: Option<&str>,
) -> Result<Vec<Device>, DeviceError> {
    let Some(requested) = requested else {
        return Ok(devices);
    };
    let selected = devices.into_iter().filter(|device| device.id == requested).collect::<Vec<_>>();
    if selected.is_empty() {
        return Err(DeviceError::RequestedDeviceUnavailable(requested.to_owned()));
    }
    Ok(selected)
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
            for id in parse_adb_devices(&output.text) {
                {
                    let abi = self.android_abi(&id)?;
                    result.push(Device { target: Target::Android, id, android_abi: Some(abi) });
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

    /// Discover requested devices, offering an interactive retry when an
    /// Android target is temporarily unavailable (for example while the
    /// phone is locked or the USB authorization prompt is pending).
    pub fn discover_with_retry(&self, targets: &[Target]) -> Result<Vec<Device>, DeviceError> {
        const MAX_RETRIES: usize = 5;
        let interactive = io::stdin().is_terminal() && io::stdout().is_terminal();
        for attempt in 0..=MAX_RETRIES {
            match self.discover(targets) {
                Ok(devices) => return Ok(devices),
                Err(DeviceError::RequestedTargetUnavailable(Target::Android))
                    if interactive && attempt < MAX_RETRIES =>
                {
                    eprintln!(
                        "torca-deploy: no ADB Android devices found. Unlock the phone, accept USB debugging, then press Enter to retry (attempt {}/{}) or type q to abort.",
                        attempt + 1,
                        MAX_RETRIES
                    );
                    print!("torca-deploy: ");
                    let _ = io::stdout().flush();
                    let mut input = String::new();
                    if io::stdin().read_line(&mut input).is_err()
                        || input.trim().eq_ignore_ascii_case("q")
                    {
                        return Err(DeviceError::RequestedTargetUnavailable(Target::Android));
                    }
                }
                Err(error) => return Err(error),
            }
        }
        Err(DeviceError::RequestedTargetUnavailable(Target::Android))
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

/// Returns only ADB transports that are ready for commands.  `adb devices`
/// also lists transports in `offline` and `unauthorized` states; treating
/// those as deployable makes installation fail later with a misleading error.
fn parse_adb_devices(output: &str) -> Vec<String> {
    output
        .lines()
        .skip_while(|line| line.trim() != "List of devices attached")
        .skip(1)
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let id = fields.next()?;
            let state = fields.next()?;
            (state == "device").then(|| id.to_owned())
        })
        .collect()
}

#[derive(Debug, Error)]
pub enum DeviceError {
    #[error("device command failed: {0}")]
    Command(String),
    #[error("device process error: {0}")]
    Process(#[from] ProcessError),
    #[error("requested {0} deployment target is unavailable")]
    RequestedTargetUnavailable(Target),
    #[error("requested device `{0}` is unavailable")]
    RequestedDeviceUnavailable(String),
    #[error("Android device {device} uses unsupported ABI `{abi}`")]
    UnsupportedAndroidAbi { device: String, abi: String },
}

#[cfg(test)]
mod tests {
    use super::{AndroidAbi, Device, parse_adb_devices, select_device};
    use crate::domain::Target;

    #[test]
    fn parser_accepts_ready_transports_only() {
        let output = "List of devices attached\n".to_owned()
            + "offline-device\toffline\n"
            + "authorized-device\tunauthorized\n"
            + "ready-device\tdevice product:pixel\n";

        assert_eq!(parse_adb_devices(&output), vec!["ready-device"]);
    }

    #[test]
    fn parser_handles_adb_noise_and_missing_header() {
        assert!(parse_adb_devices("adb server version mismatch\n").is_empty());
        assert_eq!(
            parse_adb_devices("prefix\nList of devices attached\nserial\tdevice\n"),
            vec!["serial"]
        );
    }

    #[test]
    fn exact_device_selection_is_opt_in() {
        let devices = vec![
            Device {
                target: Target::Android,
                id: "phone-a".into(),
                android_abi: Some(AndroidAbi::Arm64),
            },
            Device {
                target: Target::Android,
                id: "phone-b".into(),
                android_abi: Some(AndroidAbi::X86_64),
            },
        ];
        assert_eq!(select_device(devices.clone(), None).expect("all devices").len(), 2);
        assert_eq!(
            select_device(devices, Some("phone-b")).expect("selected device")[0].id,
            "phone-b"
        );
    }
}
