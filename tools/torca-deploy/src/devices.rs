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

/// Restrict discovered devices to a user-requested id. Wireless ADB mDNS can
/// append a collision counter such as ` (2)` to the service instance between
/// sessions. Prefer an exact match, then accept one unambiguous match after
/// removing only that volatile counter from an Android mDNS serial.
pub fn select_device(
    devices: Vec<Device>,
    requested: Option<&str>,
) -> Result<Vec<Device>, DeviceError> {
    let Some(requested) = requested else {
        return Ok(devices);
    };
    if let Some(exact) = devices.iter().find(|device| device.id == requested) {
        return Ok(vec![exact.clone()]);
    }
    let requested_stable = stable_android_device_id(requested);
    let selected = devices
        .into_iter()
        .filter(|device| {
            device.target == Target::Android
                && stable_android_device_id(&device.id) == requested_stable
        })
        .collect::<Vec<_>>();
    match selected.len() {
        0 => Err(DeviceError::RequestedDeviceUnavailable(requested.to_owned())),
        1 => Ok(selected),
        _ => Err(DeviceError::RequestedDeviceAmbiguous(requested.to_owned())),
    }
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
        .filter_map(parse_adb_device_line)
        .collect()
}

fn parse_adb_device_line(line: &str) -> Option<String> {
    const STATES: [&str; 6] =
        ["device", "offline", "unauthorized", "recovery", "sideload", "bootloader"];
    let mut offset = 0;
    for field in line.split_whitespace() {
        let relative = line[offset..].find(field)?;
        let start = offset + relative;
        offset = start + field.len();
        if STATES.contains(&field) {
            let id = line[..start].trim_end();
            return (field == "device" && !id.is_empty()).then(|| id.to_owned());
        }
    }
    None
}

fn stable_android_device_id(id: &str) -> String {
    const MDNS_SUFFIX: &str = "._adb-tls-connect._tcp";
    let Some(prefix) = id.strip_suffix(MDNS_SUFFIX) else {
        return id.to_owned();
    };
    let Some((base, counter)) = prefix.rsplit_once(" (") else {
        return id.to_owned();
    };
    let Some(counter) = counter.strip_suffix(')') else {
        return id.to_owned();
    };
    if counter.is_empty() || !counter.bytes().all(|byte| byte.is_ascii_digit()) {
        return id.to_owned();
    }
    format!("{base}{MDNS_SUFFIX}")
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
    #[error("requested device `{0}` matches more than one available Android transport")]
    RequestedDeviceAmbiguous(String),
    #[error("Android device {device} uses unsupported ABI `{abi}`")]
    UnsupportedAndroidAbi { device: String, abi: String },
}

#[cfg(test)]
mod tests {
    use super::{AndroidAbi, Device, parse_adb_devices, select_device, stable_android_device_id};
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
    fn parser_preserves_windows_mdns_collision_suffix_with_spaces() {
        let id = "adb-85Z5AIGU79XSLZMZ-RUuyXh (2)._adb-tls-connect._tcp";
        let output = format!("List of devices attached\n{id}   device\n");

        assert_eq!(parse_adb_devices(&output), vec![id]);
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

    #[test]
    fn wireless_adb_selection_accepts_a_changed_mdns_collision_counter() {
        let requested = "adb-85Z5AIGU79XSLZMZ-RUuyXh._adb-tls-connect._tcp";
        let current = "adb-85Z5AIGU79XSLZMZ-RUuyXh (2)._adb-tls-connect._tcp";
        let devices = vec![Device {
            target: Target::Android,
            id: current.into(),
            android_abi: Some(AndroidAbi::Arm64),
        }];

        let selected = select_device(devices, Some(requested)).expect("stable mDNS match");
        assert_eq!(selected[0].id, current);
        assert_eq!(stable_android_device_id(current), requested);
    }

    #[test]
    fn wireless_adb_fallback_rejects_ambiguous_matches() {
        let requested = "adb-phone._adb-tls-connect._tcp";
        let devices = [1, 2]
            .into_iter()
            .map(|counter| Device {
                target: Target::Android,
                id: format!("adb-phone ({counter})._adb-tls-connect._tcp"),
                android_abi: Some(AndroidAbi::Arm64),
            })
            .collect();

        assert!(select_device(devices, Some(requested)).is_err());
    }
}
