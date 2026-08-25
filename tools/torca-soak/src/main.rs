// Multi-process production-runtime soak orchestrator.
//
// The orchestrator deliberately talks to `torca-lab-peer` over JSONL instead
// of linking a second copy of the runtime into this process. Each peer is a
// real process with an isolated profile, which exercises lifecycle, storage,
// logging and Tor ownership boundaries.

use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, IsTerminal, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Arc;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use clap::{Parser, ValueEnum};
use serde::Serialize;

mod events;
mod report;
mod tui;
mod wizard;
mod workload;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, ValueEnum)]
enum Scenario {
    /// Android plus fake peers exchanging real messages through the production runtime.
    ActiveMessaging,
    /// Physical Android idle battery measurement.
    IdleBattery,
    /// Android network loss and recovery loop.
    Connectivity,
    /// Multi-process fake-peer runtime laboratory.
    #[default]
    RuntimeLab,
    /// Repeated deterministic Rust test suite.
    Deterministic,
}

#[derive(Clone, Copy, Debug, Serialize, ValueEnum)]
enum RelayMode {
    Managed,
    External,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
enum CommunicationProvider {
    Tor,
    Iroh,
}

impl CommunicationProvider {
    const fn wire(self) -> &'static str {
        match self {
            Self::Tor => "tor",
            Self::Iroh => "iroh",
        }
    }

    const fn requires_managed_relay(self) -> bool {
        matches!(self, Self::Tor)
    }
}

#[derive(Clone, Copy, Debug, Serialize, ValueEnum)]
enum Workload {
    /// Balanced battery profile: one message roughly every two minutes.
    Balanced,
    Moderate,
    Minimal,
}

#[derive(Clone, Copy, Debug, Serialize, ValueEnum)]
enum FaultProfile {
    Controlled,
    RelayOnly,
    None,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, ValueEnum)]
enum FixtureMode {
    /// Reuse a valid fixture when present, otherwise provision it once.
    #[default]
    Auto,
    /// Pair and provision a fresh deterministic test profile.
    None,
    /// Pair, name and persist the profile as a reusable fixture.
    Provision,
    /// Reuse a previously provisioned fixture without interactive pairing.
    Reuse,
}

#[derive(Clone, Parser, Debug, Serialize)]
#[command(name = "torca-soak")]
#[allow(clippy::struct_excessive_bools)]
pub(crate) struct Cli {
    /// Soak scenario. Omit all arguments to open the interactive wizard.
    #[arg(long, value_enum, default_value_t = Scenario::RuntimeLab)]
    scenario: Scenario,
    /// Android serial. Omit to run the fake-peer-only laboratory scenario.
    #[arg(long)]
    android: Option<String>,
    /// Legacy spelling accepted by torca-battery-soak-tui.
    #[arg(long = "device-id", hide = true)]
    legacy_device_id: Option<String>,
    /// Install/restart the SOAK Android client before the run. Active
    /// Messaging enables this automatically; Auto fixtures preserve valid data.
    #[arg(long)]
    android_auto_deploy: bool,
    /// Reuse Android and bot profiles instead of the clean Active Messaging
    /// default. Intended only for investigating a previous provisioned run.
    #[arg(long)]
    preserve_profiles: bool,
    #[arg(long, default_value_t = 5)]
    fake_peers: usize,
    #[arg(long, default_value_t = 1800)]
    duration_seconds: u64,
    /// Legacy battery-soak duration in minutes.
    #[arg(long = "duration-minutes", hide = true)]
    legacy_duration_minutes: Option<u64>,
    #[arg(long, value_enum, default_value_t = RelayMode::Managed)]
    relay: RelayMode,
    /// Communication provider used by Android and isolated lab peers.
    #[arg(long, value_enum, default_value_t = CommunicationProvider::Tor)]
    communication_provider: CommunicationProvider,
    #[arg(long)]
    relay_endpoint: Option<String>,
    #[arg(long, value_enum, default_value_t = Workload::Balanced)]
    workload: Workload,
    /// Include the high-cost Radio path in the workload. Disabled by default
    /// so a normal battery soak measures messaging/attachments only.
    #[arg(long)]
    radio: bool,
    #[arg(long, value_enum, default_value_t = FaultProfile::Controlled)]
    fault_profile: FaultProfile,
    /// Test fixture lifecycle. `auto` provisions once and reuses a valid
    /// named profile; `provision` and `reuse` force either behavior.
    #[arg(long, value_enum, default_value_t = FixtureMode::Auto)]
    fixture: FixtureMode,
    /// Stable fixture name stored below .torca/soak/fixtures.
    #[arg(long, default_value = "android-default")]
    fixture_name: String,
    #[arg(long, default_value = ".torca/soak")]
    output: PathBuf,
    /// Path to the already-built lab peer executable.
    /// Optional prebuilt lab peer. When omitted, use the platform-native
    /// binary produced by `cargo build -p torca-lab-peer`.
    #[arg(long)]
    lab_peer: Option<PathBuf>,
    /// Optional persistent bot host address (for example 127.0.0.1:47890).
    /// When omitted, the runner keeps the local process-isolated fallback.
    #[arg(long)]
    bot_host: Option<String>,
    #[arg(long)]
    bot_token: Option<String>,
    #[arg(long, default_value = ".")]
    repo_root: PathBuf,
    /// Disable the interactive terminal dashboard (for CI and redirected output).
    #[arg(long)]
    plain: bool,
    /// Explicitly request the interactive dashboard.
    #[arg(long)]
    tui: bool,
    /// Require the Android device to report battery power rather than AC/USB/wireless.
    #[arg(long)]
    require_unplugged: bool,
    /// Require the Android display to be off during a physical battery measurement.
    #[arg(long)]
    require_screen_off: bool,
    /// Include native application diagnostics in the physical battery artifact.
    #[arg(long)]
    collect_native_diagnostics: bool,
    /// Validate a physical battery artifact immediately after capture.
    #[arg(long, default_value_t = true)]
    validate_after: bool,
    /// Connectivity recovery loop count.
    #[arg(long, default_value_t = 20)]
    iterations: u32,
}

#[derive(Debug, Serialize)]
struct Manifest {
    run_id: String,
    scenario: String,
    seed: u64,
    fake_peers: usize,
    android_serial: Option<String>,
    duration_seconds: u64,
    workload: String,
    radio: bool,
    fault_profile: String,
    relay_mode: String,
    communication_provider: String,
    fixture: String,
    fixture_name: String,
    started_at_ms: u128,
}

#[derive(Debug, Serialize, serde::Deserialize)]
struct FixtureManifest {
    schema: u32,
    name: String,
    scenario: String,
    android_serial: Option<String>,
    fake_peers: usize,
    expected_contacts: usize,
    expected_conversations: usize,
    nicknames: Vec<String>,
    identities: Vec<FixtureIdentity>,
    created_at_ms: u128,
}

#[derive(Debug, Serialize, serde::Deserialize)]
struct FixtureIdentity {
    participant: String,
    id: Option<String>,
    display_name: Option<String>,
    fingerprint: Option<String>,
}

#[derive(Debug, Serialize)]
struct Summary {
    run_id: String,
    scenario: String,
    status: &'static str,
    sequence: u64,
    participants: usize,
    delivered_messages: u64,
    notifications_expected: u64,
    notifications_observed: u64,
    verdict: report::Verdict,
    reasons: Vec<String>,
    completed_at_ms: u128,
}

struct PeerProcess {
    name: String,
    executable: PathBuf,
    root: PathBuf,
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
}

struct AndroidBridge {
    serial: String,
    token: String,
    host_port: u16,
}

struct BotHostClient {
    name: String,
    address: String,
    token: String,
}

struct ManagedRelay {
    repo_root: PathBuf,
    artifact_root: PathBuf,
}

struct ActiveBatteryCapture {
    serial: String,
    root: PathBuf,
}

fn android_package() -> &'static str {
    torca_deploy::android_target::package()
}

fn android_package_installed(serial: &str) -> bool {
    Command::new("adb")
        .args(["-s", serial, "shell", "pm", "path", android_package()])
        .output()
        .is_ok_and(|output| {
            output.status.success()
                && String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .any(|line| line.trim_start().starts_with("package:"))
        })
}

impl Drop for ActiveBatteryCapture {
    fn drop(&mut self) {
        let _ = capture_adb_file(
            &self.serial,
            &["shell", "dumpsys", "battery"],
            &self.root.join("battery-end.txt"),
        );
        let _ = capture_adb_file(
            &self.serial,
            &["shell", "dumpsys", "batterystats", android_package()],
            &self.root.join("batterystats.txt"),
        );
    }
}

impl Drop for AndroidBridge {
    fn drop(&mut self) {
        let _ = Command::new("adb")
            .args(["-s", &self.serial, "forward", "--remove", &format!("tcp:{}", self.host_port)])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

impl Drop for ManagedRelay {
    fn drop(&mut self) {
        // Preserve the relay-side evidence before compose removes the
        // container. This is essential when a client reports
        // RELAY_UNREACHABLE even though Docker was healthy.
        let mut logs = Command::new("docker");
        logs.args(["compose", "-f", "infra/docker/compose.yml", "logs", "--no-color", "relay"])
            .current_dir(&self.repo_root);
        if let Ok(output) = logs.output() {
            let mut evidence = String::from_utf8_lossy(&output.stdout).into_owned();
            if !output.stderr.is_empty() {
                evidence.push_str("\n--- stderr ---\n");
                evidence.push_str(&String::from_utf8_lossy(&output.stderr));
            }
            let _ = fs::write(self.artifact_root.join("relay-compose.log"), evidence);
        }
        let mut command = Command::new("docker");
        command
            .args([
                "compose",
                "-f",
                "infra/docker/compose.yml",
                "down",
                "--timeout",
                "30",
                "--remove-orphans",
            ])
            .current_dir(&self.repo_root);
        let _ = run_external_command(&mut command, "relay cleanup");
    }
}

impl ManagedRelay {
    fn pause(&self) -> Result<(), String> {
        let mut command = Command::new("docker");
        command
            .args(["compose", "-f", "infra/docker/compose.yml", "stop", "relay"])
            .current_dir(&self.repo_root);
        let result = run_external_command(&mut command, "pause managed relay")?;
        result.status.success().then_some(()).ok_or_else(|| "managed relay pause failed".into())
    }

    fn resume(&self) -> Result<(), String> {
        let mut command = Command::new("docker");
        command
            .args(["compose", "-f", "infra/docker/compose.yml", "start", "relay"])
            .current_dir(&self.repo_root);
        let result = run_external_command(&mut command, "resume managed relay")?;
        result.status.success().then_some(()).ok_or_else(|| "managed relay resume failed".into())
    }
}

enum Participant {
    Fake(PeerProcess),
    Remote(BotHostClient),
    Android(AndroidBridge),
}

impl Participant {
    fn name(&self) -> &str {
        match self {
            Self::Fake(peer) => &peer.name,
            Self::Remote(bot) => &bot.name,
            Self::Android(android) => &android.serial,
        }
    }

    fn request(
        &mut self,
        id: &str,
        operation: &str,
        extra: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        match self {
            Self::Fake(peer) => peer.request(id, operation, extra),
            Self::Remote(bot) => bot.request(id, operation, extra),
            Self::Android(android) => android.request(id, operation, extra),
        }
    }

    fn stop(&mut self) {
        if let Self::Fake(peer) = self {
            peer.stop();
        }
    }

    fn restart(&mut self) -> Result<(), String> {
        match self {
            Self::Fake(peer) => peer.restart(),
            Self::Remote(_) => Err("remote soak bot is supervised by bot host".into()),
            Self::Android(_) => {
                Err("Android process restart is controlled by adb separately".into())
            }
        }
    }

    fn set_network(&self, enabled: bool) -> Result<(), String> {
        match self {
            Self::Android(android) => android.set_wifi(enabled),
            Self::Fake(_) | Self::Remote(_) => {
                Err("network fault injection is only supported for Android".into())
            }
        }
    }
}

impl PeerProcess {
    fn request(
        &mut self,
        id: &str,
        operation: &str,
        extra: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let mut request = serde_json::Map::new();
        request.insert("id".into(), serde_json::Value::String(id.into()));
        request.insert("op".into(), serde_json::Value::String(operation.into()));
        if let serde_json::Value::Object(fields) = extra {
            request.extend(fields);
        }
        serde_json::to_writer(&mut self.input, &serde_json::Value::Object(request))
            .map_err(|error| format!("{} write request: {error}", self.name))?;
        self.input
            .write_all(b"\n")
            .map_err(|error| format!("{} write newline: {error}", self.name))?;
        self.input.flush().map_err(|error| format!("{} flush request: {error}", self.name))?;
        let mut line = String::new();
        self.output
            .read_line(&mut line)
            .map_err(|error| format!("{} read response: {error}", self.name))?;
        if line.trim().is_empty() {
            return Err(format!("{} returned an empty response", self.name));
        }
        let response: serde_json::Value = serde_json::from_str(&line)
            .map_err(|error| format!("{} decode response: {error}", self.name))?;
        if response.get("status").and_then(serde_json::Value::as_str) != Some("succeeded") {
            return Err(format!(
                "{} operation {operation} failed: {}",
                self.name,
                response.get("error").unwrap_or(&serde_json::Value::Null)
            ));
        }
        Ok(response.get("result").cloned().unwrap_or(response))
    }

    fn stop(&mut self) {
        let _ = self.request("shutdown", "shutdown", serde_json::json!({}));
        let _ = self.child.kill();
        let _ = self.child.wait();
    }

    fn restart(&mut self) -> Result<(), String> {
        self.stop();
        let (child, input, output) = spawn_peer_parts(&self.executable, &self.root, &self.name)?;
        self.child = child;
        self.input = input;
        self.output = BufReader::new(output);
        Ok(())
    }
}

impl BotHostClient {
    fn request(
        &self,
        id: &str,
        operation: &str,
        extra: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let mut request = serde_json::Map::new();
        request.insert("id".into(), serde_json::Value::String(id.into()));
        request.insert("op".into(), serde_json::Value::String(operation.into()));
        if let serde_json::Value::Object(fields) = extra {
            request.extend(fields);
        }
        let body = serde_json::Value::Object(request).to_string();
        let mut stream = TcpStream::connect(&self.address)
            .map_err(|error| format!("{} connect bot host: {error}", self.name))?;
        write!(
            stream,
            "POST /bot/{} HTTP/1.1\r\nHost: {}\r\nX-Torca-Soak-Token: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            self.name,
            self.address,
            self.token,
            body.len(),
            body
        )
        .map_err(|error| format!("{} write bot host request: {error}", self.name))?;
        let mut response = String::new();
        stream
            .read_to_string(&mut response)
            .map_err(|error| format!("{} read bot host response: {error}", self.name))?;
        let (_, body) = response
            .split_once("\r\n\r\n")
            .ok_or_else(|| format!("{} malformed bot host response", self.name))?;
        let value: serde_json::Value = serde_json::from_str(body)
            .map_err(|error| format!("{} decode bot host response: {error}", self.name))?;
        if value.get("status").and_then(serde_json::Value::as_str) != Some("succeeded") {
            return Err(format!(
                "{} operation {operation} failed: {}",
                self.name,
                value.get("error").unwrap_or(&serde_json::Value::Null)
            ));
        }
        Ok(value.get("result").cloned().unwrap_or(value))
    }
}

impl Drop for PeerProcess {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("torca-soak: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let explicit_args = std::env::args_os().skip(1).collect::<Vec<_>>();
    let mut cli = if explicit_args.is_empty() || explicit_args.iter().all(|arg| arg == "--tui") {
        if !std::io::stdout().is_terminal() || !std::io::stderr().is_terminal() {
            return Err("no soak scenario supplied and no interactive terminal is available; pass --scenario <name> --plain".into());
        }
        wizard::choose_plan()?.ok_or_else(|| "soak cancelled".to_owned())?
    } else {
        Cli::parse()
    };
    // Active Messaging is a delivery/battery baseline; injected faults belong
    // to RuntimeLab unless the operator explicitly opts into them.
    if cli.scenario == Scenario::ActiveMessaging
        && !explicit_args.iter().any(|arg| arg == "--fault-profile")
    {
        cli.fault_profile = FaultProfile::None;
    }
    // Keep the old binary/PowerShell documentation usable while all new
    // scenarios go through the same cockpit and typed CLI.
    if cli.android.is_none() {
        cli.android = cli.legacy_device_id.take();
    }
    if let Some(minutes) = cli.legacy_duration_minutes.take() {
        cli.scenario = Scenario::IdleBattery;
        cli.duration_seconds = minutes.saturating_mul(60);
    }
    let auto_fixture_requested = cli.fixture == FixtureMode::Auto;
    if auto_fixture_requested {
        // A stale or truncated fixture must not turn the click-and-play path
        // into a hard failure. Treat only a decodable manifest as reusable;
        // provisioning will replace an invalid one deterministically.
        cli.fixture = if cli.scenario == Scenario::ActiveMessaging {
            if load_fixture_manifest(&cli.repo_root, &cli.fixture_name).is_ok() {
                FixtureMode::Reuse
            } else {
                FixtureMode::Provision
            }
        } else {
            // RuntimeLab is deliberately an isolated, disposable mesh. It
            // must never accidentally reuse the Android fixture when a user
            // invokes the CLI directly instead of going through the wizard.
            FixtureMode::None
        };
    }
    // A reusable fixture is already installed and paired.  Re-running the
    // deployer here would reset/reinstall the client and re-enter relay
    // warm-up, defeating the point of a stable battery measurement (and can
    // fail independently of the already-running workload).  Fresh active
    // messaging runs still get the deterministic SOAK deploy automatically.
    if cli.scenario == Scenario::ActiveMessaging {
        let android_package_missing =
            cli.android.as_deref().is_some_and(|serial| !android_package_installed(serial));
        if auto_fixture_requested && android_package_missing {
            cli.fixture = FixtureMode::Provision;
        }
        if cli.android.is_some() && (cli.fixture != FixtureMode::Reuse || android_package_missing) {
            cli.android_auto_deploy = true;
        }
    }
    if cli.bot_host.is_none() {
        cli.bot_host = std::env::var("TORCA_SOAK_BOT_HOST").ok();
    }
    if cli.bot_token.is_none() {
        cli.bot_token = std::env::var("TORCA_SOAK_BOT_TOKEN").ok();
    }
    if !cli.plain {
        let terminal = std::io::stdout().is_terminal() && std::io::stderr().is_terminal();
        if terminal {
            return tui::run(cli);
        }
        eprintln!(
            "torca-soak: non-interactive output detected; using plain mode (pass --plain to silence this message)"
        );
    }
    run_plan(cli)
}

pub(crate) fn run_plan(cli: Cli) -> Result<(), String> {
    match cli.scenario {
        Scenario::IdleBattery => run_battery_harness(&cli),
        Scenario::Connectivity => run_connectivity_harness(&cli),
        Scenario::Deterministic => run_deterministic_harness(&cli),
        Scenario::ActiveMessaging | Scenario::RuntimeLab => run_scenario(cli),
    }
}

fn run_battery_harness(cli: &Cli) -> Result<(), String> {
    let device = cli.android.as_deref().ok_or("idle-battery requires --android <adb-serial>")?;
    let mut args = vec![
        "-DurationMinutes".to_owned(),
        cli.duration_seconds.div_ceil(60).to_string(),
        "-DeviceId".to_owned(),
        device.to_owned(),
    ];
    if cli.require_unplugged {
        args.push("-RequireUnplugged".to_owned());
    }
    if cli.require_screen_off {
        args.push("-RequireScreenOff".to_owned());
    }
    if cli.collect_native_diagnostics {
        args.push("-CollectNativeDiagnostics".to_owned());
    }
    if cli.validate_after {
        args.push("-ValidateAfter".to_owned());
    }
    run_powershell_backend(cli, "Run-TorcaBatterySoak.ps1", &args)
}

fn run_connectivity_harness(cli: &Cli) -> Result<(), String> {
    let device = cli.android.as_deref().ok_or("connectivity requires --android <adb-serial>")?;
    run_powershell_backend(
        cli,
        "Run-TorcaConnectivitySoak.ps1",
        &[
            "-Iterations".to_owned(),
            cli.iterations.to_string(),
            "-DeviceId".to_owned(),
            device.to_owned(),
        ],
    )
}

fn run_deterministic_harness(cli: &Cli) -> Result<(), String> {
    run_powershell_backend(
        cli,
        "Run-TorcaDeterministicSoak.ps1",
        &[
            "-Iterations".to_owned(),
            cli.iterations.to_string(),
            "-RepoRoot".to_owned(),
            cli.repo_root.to_string_lossy().into_owned(),
        ],
    )
}

fn run_powershell_backend(cli: &Cli, script: &str, arguments: &[String]) -> Result<(), String> {
    let shell = if cfg!(windows) { "powershell" } else { "pwsh" };
    let script_path = cli.repo_root.join("scripts").join(script);
    let mut command = Command::new(shell);
    command
        .args(["-NoLogo", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(&script_path)
        .args(arguments)
        .current_dir(&cli.repo_root);
    if !tui::is_active() {
        let status = command
            .status()
            .map_err(|error| format!("start {}: {error}", script_path.display()))?;
        return status
            .success()
            .then_some(())
            .ok_or_else(|| format!("{} failed with {status}", script_path.display()));
    }
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child =
        command.spawn().map_err(|error| format!("start {}: {error}", script_path.display()))?;
    let stdout = child.stdout.take().ok_or("PowerShell stdout unavailable")?;
    let stderr = child.stderr.take().ok_or("PowerShell stderr unavailable")?;
    let (sender, receiver) = mpsc::channel();
    spawn_backend_reader(stdout, sender.clone(), false);
    spawn_backend_reader(stderr, sender, true);
    loop {
        while let Ok((stderr, line)) = receiver.try_recv() {
            tui::publish_backend_line(&line, stderr);
        }
        if tui::cancel_requested() {
            let _ = child.kill();
        }
        if let Some(status) =
            child.try_wait().map_err(|error| format!("wait {}: {error}", script_path.display()))?
        {
            while let Ok((stderr, line)) = receiver.try_recv() {
                tui::publish_backend_line(&line, stderr);
            }
            return status
                .success()
                .then_some(())
                .ok_or_else(|| format!("{} failed with {status}", script_path.display()));
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn spawn_backend_reader<R: std::io::Read + Send + 'static>(
    reader: R,
    sender: mpsc::Sender<(bool, String)>,
    stderr: bool,
) {
    thread::spawn(move || {
        let reader = BufReader::new(reader);
        for line in reader.lines().map_while(Result::ok) {
            let _ = sender.send((stderr, line));
        }
    });
}

pub(crate) fn run_scenario(cli: Cli) -> Result<(), String> {
    let output = cli.output.clone();
    match run_scenario_inner(cli) {
        Ok(()) => Ok(()),
        Err(error) => {
            // Preserve a machine-readable failure even when setup aborts
            // before the normal summary/verdict phase (for example an
            // Android install permission prompt). The cockpit already shows
            // the live error; this artifact makes the failed run auditable.
            write_failure_artifact(&output, &error);
            Err(error)
        }
    }
}

fn write_failure_artifact(output: &Path, error: &str) {
    let Ok(runs) = fs::read_dir(output) else { return };
    let Some(root) = runs
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .filter_map(|entry| {
            let modified = entry.metadata().ok()?.modified().ok()?;
            Some((modified, entry.path()))
        })
        .max_by_key(|(modified, _)| *modified)
        .map(|(_, path)| path)
    else {
        return;
    };
    let failure = serde_json::json!({
        "status": "failed",
        "error": error,
        "recorded_at_ms": SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.as_millis())
            .unwrap_or_default(),
    });
    let _ = write_json(&root.join("failure.json"), &failure);
    if let Ok(mut timeline) =
        OpenOptions::new().create(true).append(true).open(root.join("timeline.jsonl"))
    {
        let _ = record(&mut timeline, "run_failed", failure);
    }
}

fn run_scenario_inner(cli: Cli) -> Result<(), String> {
    if cli.fake_peers < 1 {
        return Err("fake-peers must be at least 1".into());
    }
    if cli.duration_seconds == 0 {
        return Err("duration-seconds must be positive".into());
    }
    if matches!(cli.relay, RelayMode::External) && cli.relay_endpoint.is_none() {
        return Err("--relay-endpoint is required with --relay external".into());
    }
    // A second cockpit must not reset the shared bot roots or fight the
    // managed relay while the first run is active. Reuse the deployer's
    // stale-owner handling instead of inventing another lock protocol.
    let _soak_lock =
        torca_deploy::persistence::StateStore::new(torca_deploy::persistence::DeployPaths {
            repo_root: cli.repo_root.clone(),
            state_root: cli.repo_root.join(".torca/soak-state"),
        })
        .acquire_lock()
        .map_err(|error| format!("SOAK1 is already running or cannot acquire its lock: {error}"))?;
    validate_fixture_name(&cli.fixture_name)?;
    let reusable_fixture = match cli.fixture {
        FixtureMode::Reuse => Some(load_fixture_manifest(&cli.repo_root, &cli.fixture_name)?),
        FixtureMode::None | FixtureMode::Provision | FixtureMode::Auto => None,
    };
    let started =
        SystemTime::now().duration_since(UNIX_EPOCH).map_err(|error| error.to_string())?;
    let run_id = format!("soak-{}-{}", started.as_secs(), uuid::Uuid::new_v4().simple());
    let root = cli.output.join(&run_id);
    fs::create_dir_all(&root).map_err(|error| format!("create soak root: {error}"))?;
    tui::set_run_root(&root);
    let manifest = Manifest {
        run_id: run_id.clone(),
        scenario: format!("{:?}", cli.scenario),
        seed: started.as_nanos() as u64,
        fake_peers: cli.fake_peers,
        android_serial: cli.android.clone(),
        duration_seconds: cli.duration_seconds,
        workload: format!("{:?}", cli.workload),
        radio: cli.radio,
        fault_profile: format!("{:?}", cli.fault_profile),
        relay_mode: format!("{:?}", cli.relay),
        communication_provider: cli.communication_provider.wire().to_owned(),
        fixture: format!("{:?}", cli.fixture),
        fixture_name: cli.fixture_name.clone(),
        started_at_ms: started.as_millis(),
    };
    write_json(&root.join("manifest.json"), &manifest)?;
    write_json(&root.join("plan.json"), &cli)?;
    // Battery measurement starts only after setup/pairing. Build, relay warmup,
    // permission prompts and provisioning belong to preflight, not the user
    // workload being measured.
    let mut _battery_capture: Option<ActiveBatteryCapture> = None;
    let mut timeline = File::create(root.join("timeline.jsonl"))
        .map_err(|error| format!("create timeline: {error}"))?;
    record(
        &mut timeline,
        "run_started",
        serde_json::json!({"runId": run_id, "seed": manifest.seed}),
    )?;

    let managed_relay;
    let endpoint = match (cli.communication_provider, cli.relay) {
        (provider, _) if !provider.requires_managed_relay() => {
            managed_relay = None;
            None
        }
        (_, RelayMode::Managed) => {
            tui::publish_event("relay_starting", &serde_json::json!({"mode": "managed"}));
            let (endpoint, guard) = start_managed_relay(&cli.repo_root, &root)?;
            managed_relay = Some(guard);
            Some(endpoint)
        }
        (_, RelayMode::External) => {
            managed_relay = None;
            cli.relay_endpoint.clone().or_else(|| std::env::var("TORCA_RELAY_ENDPOINT").ok())
        }
    };
    if cli.communication_provider.requires_managed_relay()
        && endpoint.as_deref().is_none_or(|value| !valid_endpoint(value))
    {
        return Err("a valid Tor relay endpoint is required; start the managed relay or pass --relay-endpoint host.onion:port".into());
    }
    let provider_requires_service = cli.communication_provider.requires_managed_relay();
    record(
        &mut timeline,
        "provider_ready",
        serde_json::json!({
            "provider": cli.communication_provider.wire(),
            "endpoint": endpoint.as_deref().unwrap_or_default(),
            "health": "ready",
            "service": if provider_requires_service { "managed" } else { "provider_owned" },
            "serviceRequired": provider_requires_service,
            "incoming": if provider_requires_service { "onion_pending" } else { "provider_owned" }
        }),
    )?;

    let peer_executable = if cli.bot_host.is_none() {
        build_lab_peer(&cli.repo_root, cli.communication_provider, endpoint.as_deref())?
    } else {
        cli.lab_peer.clone().unwrap_or_else(default_lab_peer_path)
    };

    let mut peers: Vec<Participant> = Vec::new();
    if let Some(serial) = &cli.android {
        if cli.android_auto_deploy {
            record(
                &mut timeline,
                "android_preflight_started",
                serde_json::json!({"serial": serial, "autoDeploy": true}),
            )?;
            let mut reuse_built_artifact = false;
            loop {
                match ensure_android_deployed(
                    &cli.repo_root,
                    serial,
                    !cli.preserve_profiles && cli.fixture != FixtureMode::Reuse,
                    reuse_built_artifact,
                    cli.communication_provider,
                ) {
                    Ok(()) => break,
                    Err(error) if wait_for_android_preflight_retry(serial, &error) => {
                        reuse_built_artifact = error.contains("INSTALL_FAILED_USER_RESTRICTED");
                        record(
                            &mut timeline,
                            "android_preflight_retrying",
                            serde_json::json!({"serial": serial, "error": error}),
                        )?;
                    }
                    Err(error) => return Err(error),
                }
            }
            record(
                &mut timeline,
                "android_preflight_ready",
                serde_json::json!({"serial": serial}),
            )?;
        }
        let android = AndroidBridge::connect(serial)?;
        if cli.scenario == Scenario::ActiveMessaging {
            prepare_notification_probe(serial)?;
        }
        record(&mut timeline, "android_ready", serde_json::json!({"serial": serial}))?;
        peers.push(Participant::Android(android));
    }
    // Bot profiles are deliberately outside the per-run artifact directory.
    // Active Messaging resets the selected roots by default for reproducible
    // provisioning; --preserve-profiles keeps them for incident investigation.
    let bot_root = cli.repo_root.join(".torca/soak/bots");
    fs::create_dir_all(&bot_root)
        .map_err(|error| format!("create persistent bot root: {error}"))?;
    for index in 0..cli.fake_peers {
        let name = format!("peer-{}", (b'a' + index as u8) as char);
        if let Some(address) = &cli.bot_host {
            let token = cli
                .bot_token
                .as_deref()
                .ok_or("--bot-token or TORCA_SOAK_BOT_TOKEN is required with --bot-host")?;
            peers.push(Participant::Remote(BotHostClient {
                name,
                address: address.clone(),
                token: token.to_owned(),
            }));
        } else {
            let peer_root = bot_root.join(&name);
            if matches!(cli.scenario, Scenario::ActiveMessaging | Scenario::RuntimeLab)
                && cli.fixture != FixtureMode::Reuse
                && !cli.preserve_profiles
                && peer_root.exists()
            {
                fs::remove_dir_all(&peer_root).map_err(|error| {
                    format!("reset clean bot profile {}: {error}", peer_root.display())
                })?;
            }
            fs::create_dir_all(&peer_root)
                .map_err(|error| format!("create {name} root: {error}"))?;
            peers.push(Participant::Fake(spawn_peer(&peer_executable, &peer_root, &name)?));
        }
    }

    for peer in &mut peers {
        let response = peer.request("readiness", "diagnostics", serde_json::json!({}))?;
        record(
            &mut timeline,
            "peer_ready",
            serde_json::json!({"peer": peer.name(), "response": response}),
        )?;
    }

    // The lab peer's initial diagnostics response proves only that the actor
    // exists.  Pairing must not be issued against a provider that is still
    // composing: that turns a deterministic fixture setup into a durable
    // "saved locally" retry and makes the invitation invisible to the
    // orchestrator.  Wait for the selected provider itself before any profile
    // provisioning or pairing command.
    for peer in &mut peers {
        wait_for_provider_ready(peer)?;
        record(
            &mut timeline,
            "provider_ready",
            serde_json::json!({"peer": peer.name(), "provider": cli.communication_provider.wire()}),
        )?;
    }

    if cli.fixture != FixtureMode::Reuse {
        if let Some(android) = peers.iter_mut().find(|peer| matches!(peer, Participant::Android(_)))
        {
            wait_for_profile_setup(android)?;
        }
        provision_fixture_profiles(&mut peers)?;
    }

    let (initial_android_contacts, initial_android_conversations) = if cli.scenario
        == Scenario::ActiveMessaging
    {
        let android = peers
            .iter_mut()
            .find(|peer| matches!(peer, Participant::Android(_)))
            .ok_or("Android participant missing from active-messaging scenario")?;
        let snapshot = snapshot_with_retry(android, "active-contacts-before")?;
        let count = contact_count(&snapshot);
        let conversations = conversation_count(&snapshot);
        record(
            &mut timeline,
            "active_preflight_baseline",
            serde_json::json!({"android": android.name(), "contacts": count, "conversations": conversations}),
        )?;
        (count, conversations)
    } else {
        (0, 0)
    };

    if cli.scenario == Scenario::ActiveMessaging {
        if cli.android.is_none() {
            return Err("active-messaging requires --android <adb-serial>".into());
        }
        if cli.fixture == FixtureMode::Reuse {
            validate_reusable_fixture(
                &mut peers,
                reusable_fixture.as_ref().ok_or("fixture manifest missing")?,
            )?;
            rename_android_fixture_contacts(
                &mut peers,
                &reusable_fixture.as_ref().ok_or("fixture manifest missing")?.nicknames,
            )?;
            let android = peers
                .iter_mut()
                .find(|peer| matches!(peer, Participant::Android(_)))
                .ok_or("Android participant missing from reusable fixture")?;
            // The relationship already exists in a reusable fixture. Relay is
            // only needed for the original pairing transaction; workload
            // recovery needs local Tor, not a fresh rendezvous publication.
            wait_for_provider_ready(android)?;
            record(&mut timeline, "fixture_reused", serde_json::json!({"name": cli.fixture_name}))?;
        } else {
            pair_android_star(&mut peers, &mut timeline)?;
            let names = peers
                .iter()
                .filter(|peer| matches!(peer, Participant::Fake(_) | Participant::Remote(_)))
                .enumerate()
                .map(|(index, peer)| fixture_nickname(peer, index))
                .collect::<Vec<_>>();
            rename_android_fixture_contacts(&mut peers, &names)?;
        }
        pair_bot_ring(&mut peers, &mut timeline)?;
        validate_active_messaging_preflight(
            &mut peers,
            if cli.fixture == FixtureMode::Reuse {
                reusable_fixture
                    .as_ref()
                    .map_or(cli.fake_peers, |fixture| fixture.expected_contacts)
            } else {
                initial_android_contacts.saturating_add(cli.fake_peers)
            },
            if cli.fixture == FixtureMode::Reuse {
                reusable_fixture
                    .as_ref()
                    .map_or(cli.fake_peers, |fixture| fixture.expected_conversations)
            } else {
                initial_android_conversations.saturating_add(cli.fake_peers)
            },
            cli.fake_peers,
            &mut timeline,
        )?;
    } else if cli.fixture == FixtureMode::Reuse {
        validate_reusable_fixture(
            &mut peers,
            reusable_fixture.as_ref().ok_or("fixture manifest missing")?,
        )?;
        record(&mut timeline, "fixture_reused", serde_json::json!({"name": cli.fixture_name}))?;
    } else {
        pair_mesh(&mut peers, &mut timeline)?;
    }

    if cli.fixture == FixtureMode::Provision {
        persist_fixture_manifest(&cli, &mut peers, &root, started.as_millis(), &mut timeline)?;
    }

    if cli.scenario == Scenario::ActiveMessaging {
        let serial = cli.android.as_deref().ok_or("active-messaging requires --android")?;
        record(
            &mut timeline,
            "measurement_started",
            serde_json::json!({"serial": serial, "reason": "preflight_complete"}),
        )?;
        _battery_capture = Some(start_active_battery_capture(
            serial,
            &root,
            cli.require_unplugged,
            cli.require_screen_off,
        )?);
    }

    let deadline = Instant::now() + Duration::from_secs(cli.duration_seconds);
    let run_started = Instant::now();
    let mut fault_injected = false;
    let mut android_network_fault_injected = false;
    let mut peer_restart_injected = false;
    let mut sequence = 0u64;
    let mut notifications_expected = 0u64;
    let mut delivered_messages = 0u64;
    let mut next_actions = (0..peers.len())
        .map(|index| workload::initial_deadline(run_started, manifest.seed, index))
        .collect::<Vec<_>>();
    while Instant::now() < deadline && !tui::cancel_requested() {
        if !fault_injected
            && !matches!(cli.fault_profile, FaultProfile::None)
            && run_started.elapsed() >= Duration::from_secs(cli.duration_seconds / 3)
        {
            fault_injected = true;
            if let Some(relay) = managed_relay.as_ref() {
                record(
                    &mut timeline,
                    "relay_fault_started",
                    serde_json::json!({"durationSeconds": 15}),
                )?;
                relay.pause()?;
                tui::controlled_sleep(Duration::from_secs(15));
                relay.resume()?;
                record(&mut timeline, "relay_fault_recovered", serde_json::json!({}))?;
            } else if matches!(cli.fault_profile, FaultProfile::Controlled) {
                record(
                    &mut timeline,
                    "relay_fault_skipped",
                    serde_json::json!({"reason": "external relay"}),
                )?;
            }
        }
        if !android_network_fault_injected
            && matches!(cli.fault_profile, FaultProfile::Controlled)
            && run_started.elapsed() >= Duration::from_secs(cli.duration_seconds / 3)
        {
            if let Some(index) =
                peers.iter().position(|peer| matches!(peer, Participant::Android(_)))
            {
                android_network_fault_injected = true;
                let serial = peers[index].name().to_owned();
                record(
                    &mut timeline,
                    "android_network_fault_started",
                    serde_json::json!({"serial": serial, "durationSeconds": 10}),
                )?;
                peers[index].set_network(false)?;
                tui::controlled_sleep(Duration::from_secs(10));
                peers[index].set_network(true)?;
                record(
                    &mut timeline,
                    "android_network_fault_recovered",
                    serde_json::json!({"serial": serial}),
                )?;
            }
        }
        if !peer_restart_injected
            && !matches!(cli.fault_profile, FaultProfile::None)
            && run_started.elapsed() >= Duration::from_secs(cli.duration_seconds / 2)
        {
            peer_restart_injected = true;
            if let Some(peer) = peers.iter_mut().find(|peer| matches!(peer, Participant::Fake(_))) {
                let name = peer.name().to_owned();
                peer.restart()?;
                let response =
                    peer.request("restart-readiness", "diagnostics", serde_json::json!({}))?;
                record(
                    &mut timeline,
                    "peer_restarted",
                    serde_json::json!({"peer": name, "response": response}),
                )?;
            }
        }
        let now = Instant::now();
        let Some(peer_index) = next_actions.iter().position(|due| *due <= now) else {
            let next = next_actions.iter().copied().min().unwrap_or(deadline);
            tui::controlled_sleep(
                next.saturating_duration_since(now).min(Duration::from_millis(250)),
            );
            continue;
        };
        let cadence_seconds = match cli.workload {
            Workload::Balanced => 120,
            Workload::Moderate => 10,
            Workload::Minimal => 2,
        };
        next_actions[peer_index] = workload::next_deadline(
            Instant::now(),
            manifest.seed,
            peer_index,
            sequence,
            cadence_seconds,
        );
        record(
            &mut timeline,
            "participant_waking",
            serde_json::json!({"peer": peers[peer_index].name(), "sequence": sequence + 1}),
        )?;
        {
            sequence = sequence.saturating_add(1);
            let (snapshot, bot_to_bot, conversation_id, peer_name, body) = {
                let peer = &mut peers[peer_index];
                let snapshot = snapshot_with_retry(peer, "snapshot")?;
                let bot_to_bot = matches!(peer, Participant::Fake(_) | Participant::Remote(_))
                    && sequence.is_multiple_of(3);
                let conversation_index = usize::from(bot_to_bot);
                let conversation_id = conversation_id_at(&snapshot, conversation_index)
                    .ok_or_else(|| format!("{} has no conversation after pairing", peer.name()))?;
                let body = format!("torca-soak sequence={sequence} sender={}", peer.name());
                let response = peer.request(
                    &format!("message-{sequence}"),
                    "message.send",
                    serde_json::json!({"conversationIdHex": conversation_id, "body": body}),
                )?;
                let peer_name = peer.name().to_owned();
                record(
                    &mut timeline,
                    "message_queued",
                    serde_json::json!({"peer": peer_name, "sequence": sequence, "response": response}),
                )?;
                (snapshot, bot_to_bot, conversation_id.clone(), peer_name, body)
            };
            let mut attachment_name = None;
            if cli.scenario == Scenario::ActiveMessaging
                && !bot_to_bot
                && matches!(peers[peer_index], Participant::Fake(_) | Participant::Remote(_))
            {
                notifications_expected = notifications_expected.saturating_add(1);
            }
            if sequence.is_multiple_of(30) {
                let (attachment, fixture_path) = {
                    let peer = &mut peers[peer_index];
                    let fixture = peer.request(
                        &format!("fixture-{sequence}"),
                        "attachment.fixture",
                        serde_json::json!({"size": 1_048_576}),
                    )?;
                    let fixture_path = fixture
                        .pointer("/result/path")
                        .or_else(|| fixture.get("path"))
                        .and_then(serde_json::Value::as_str)
                        .ok_or_else(|| format!("{} fixture path missing", peer.name()))?
                        .to_owned();
                    let attachment = peer.request(
                        &format!("attachment-{sequence}"),
                        "attachment.queue",
                        serde_json::json!({
                            "conversationIdHex": conversation_id,
                            "sourcePath": fixture_path,
                            "name": format!("soak-{sequence}.bin"),
                            "mediaType": "application/octet-stream",
                            "size": 1_048_576_u64
                        }),
                    )?;
                    (attachment, fixture_path)
                };
                record(
                    &mut timeline,
                    "attachment_queued",
                    serde_json::json!({"peer": peer_name, "sequence": sequence, "sourcePath": fixture_path, "response": attachment}),
                )?;
                attachment_name = Some(format!("soak-{sequence}.bin"));
            }
            if cli.radio && sequence.is_multiple_of(12) {
                let contact_id = first_contact_id(&snapshot)
                    .ok_or_else(|| format!("{peer_name} has no contact for radio"))?;
                let begin = {
                    let peer = &mut peers[peer_index];
                    peer.request(
                        &format!("radio-enable-{sequence}"),
                        "radio.enable",
                        serde_json::json!({"contactIdHex": contact_id, "enabled": true}),
                    )?;
                    peer.request(
                        &format!("radio-begin-{sequence}"),
                        "radio.begin",
                        serde_json::json!({"contactIdHex": contact_id}),
                    )?
                };
                wait_for_remote_radio(&mut peers, peer_index, &contact_id)?;
                tui::controlled_sleep(Duration::from_secs(1));
                let end = peers[peer_index].request(
                    &format!("radio-end-{sequence}"),
                    "radio.end",
                    serde_json::json!({"contactIdHex": contact_id}),
                )?;
                record(
                    &mut timeline,
                    "radio_burst",
                    serde_json::json!({"peer": peer_name, "sequence": sequence, "begin": begin, "end": end}),
                )?;
            }
            wait_for_message(&mut peers, peer_index, &body)?;
            if let Some(name) = attachment_name {
                wait_for_attachment(&mut peers, peer_index, &name)?;
            }
            delivered_messages = delivered_messages.saturating_add(1);
        }
        record(
            &mut timeline,
            "participant_sleeping",
            serde_json::json!({
                "peer": peers[peer_index].name(),
                "nextActionInSeconds": next_actions[peer_index]
                    .saturating_duration_since(Instant::now())
                    .as_secs()
            }),
        )?;
    }

    let cancelled = tui::cancel_requested();
    let (notifications_observed, notification_error) =
        if cli.scenario == Scenario::ActiveMessaging && !cancelled {
            match collect_notification_probe(
                cli.android.as_deref().ok_or("active-messaging Android missing")?,
                &root,
                notifications_expected,
            ) {
                Ok(value) => (value, None),
                Err(error) => {
                    record(
                        &mut timeline,
                        events::EventKind::NotificationAssertionFailed.as_str(),
                        serde_json::json!({"error": error.clone()}),
                    )?;
                    (0, Some(error))
                }
            }
        } else {
            (0, None)
        };
    for peer in &mut peers {
        peer.stop();
    }
    let mut evaluation = report::evaluate(
        cancelled,
        matches!(cli.scenario, Scenario::ActiveMessaging | Scenario::RuntimeLab),
        cli.scenario == Scenario::ActiveMessaging,
        delivered_messages,
        notifications_expected,
        notifications_observed,
    );
    if let Some(error) = notification_error {
        if !evaluation.reasons.iter().any(|reason| reason == &error) {
            evaluation.reasons.push(error);
        }
        evaluation.verdict = report::Verdict::Fail;
    }
    tui::publish_event(
        events::EventKind::RunVerdict.as_str(),
        &serde_json::json!({
            "verdict": evaluation.verdict,
            "reasons": evaluation.reasons.clone(),
            "messagesDelivered": delivered_messages,
            "notificationsExpected": notifications_expected,
            "notificationsObserved": notifications_observed,
        }),
    );
    let status = if cancelled { "cancelled" } else { "completed" };
    record(
        &mut timeline,
        if cancelled { "run_cancelled" } else { "run_completed" },
        serde_json::json!({"sequence": sequence}),
    )?;
    let completed =
        SystemTime::now().duration_since(UNIX_EPOCH).map_err(|error| error.to_string())?;
    write_json(
        &root.join("summary.json"),
        &Summary {
            run_id,
            scenario: format!("{:?}", cli.scenario),
            status,
            sequence,
            participants: peers.len(),
            delivered_messages,
            notifications_expected,
            notifications_observed,
            verdict: evaluation.verdict,
            reasons: evaluation.reasons,
            completed_at_ms: completed.as_millis(),
        },
    )?;
    let verdict = evaluation.verdict;
    drop(managed_relay);
    if verdict == report::Verdict::Fail {
        return Err(format!("SOAK1 failed; see {}/summary.json", root.display()));
    }
    Ok(())
}

fn wait_for_android_preflight_retry(serial: &str, error: &str) -> bool {
    if !tui::is_active()
        || !(error.contains("INSTALL_FAILED_USER_RESTRICTED")
            || error.contains("requested android deployment target is unavailable")
            || error.contains("not ready"))
    {
        return false;
    }
    tui::publish_event(
        "android_action_required",
        &serde_json::json!({
            "serial": serial,
            "action": if error.contains("INSTALL_FAILED_USER_RESTRICTED") {
                "Unlock Android; enable Developer options > Install via USB / USB debugging (Security settings), approve the installation, then press r"
            } else {
                "Reconnect or unlock Android, then press r to retry"
            },
            "error": error,
        }),
    );
    while !tui::cancel_requested() {
        if tui::take_retry_requested() {
            return true;
        }
        tui::controlled_sleep(Duration::from_millis(200));
    }
    false
}

fn start_managed_relay(
    repo_root: &Path,
    artifact_root: &Path,
) -> Result<(String, ManagedRelay), String> {
    let stack_root = repo_root.join(".torca/stack");
    let _ = fs::remove_file(stack_root.join("relay_ready.txt"));
    let guard =
        ManagedRelay { repo_root: repo_root.to_owned(), artifact_root: artifact_root.to_owned() };
    let mut command = Command::new("docker");
    command
        .args(["compose", "-f", "infra/docker/compose.yml", "up", "-d", "--build", "relay"])
        .current_dir(repo_root);
    let result = if tui::is_active() {
        match run_external_command(&mut command, "relay build/start") {
            Ok(result) => result,
            Err(error) => {
                drop(guard);
                return Err(error);
            }
        }
    } else {
        let output = match command.output() {
            Ok(output) => output,
            Err(error) => {
                drop(guard);
                return Err(format!("relay build/start: {error}"));
            }
        };
        if !output.status.success() {
            drop(guard);
            return Err(format!(
                "managed relay compose start failed (exit={}): {}",
                output.status.code().map_or_else(|| "unknown".into(), |code| code.to_string()),
                command_output_tail(&output.stdout, &output.stderr),
            ));
        }
        ExternalCommandResult {
            status: output.status,
            tail: command_output_tail(&output.stdout, &output.stderr),
        }
    };
    if !result.status.success() {
        let detail = if result.tail.is_empty() {
            "see the cockpit Logs view for Docker output".to_owned()
        } else {
            result.tail
        };
        drop(guard);
        return Err(format!("managed relay compose start failed: {detail}"));
    }
    let endpoint_file = repo_root.join(".torca/stack/relay_endpoint.txt");
    let ready_file = repo_root.join(".torca/stack/relay_ready.txt");
    let deadline = Instant::now() + Duration::from_secs(180);
    while Instant::now() < deadline && !tui::cancel_requested() {
        if let Ok(endpoint) = fs::read_to_string(&endpoint_file) {
            let endpoint = endpoint.trim().to_owned();
            let ready = fs::read_to_string(&ready_file)
                .ok()
                .is_some_and(|value| value.lines().any(|line| line.trim() == endpoint));
            if valid_endpoint(&endpoint) && ready {
                return Ok((endpoint, guard));
            }
        }
        tui::controlled_sleep(Duration::from_secs(2));
    }
    drop(guard);
    Err(format!(
        "managed relay did not publish a valid endpoint within 180s: {}",
        endpoint_file.display()
    ))
}

fn command_output_tail(stdout: &[u8], stderr: &[u8]) -> String {
    const MAX_LINES: usize = 20;
    let mut lines = String::from_utf8_lossy(stdout)
        .lines()
        .chain(String::from_utf8_lossy(stderr).lines())
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if lines.len() > MAX_LINES {
        lines.drain(..lines.len() - MAX_LINES);
    }
    if lines.is_empty() { String::new() } else { lines.join(" | ") }
}

fn build_lab_peer(
    repo_root: &Path,
    provider: CommunicationProvider,
    endpoint: Option<&str>,
) -> Result<PathBuf, String> {
    let mut command = Command::new("cargo");
    command
        .args(["build", "-p", "torca-lab-peer", "--locked"])
        .env("TORCA_COMMUNICATION_PROVIDER", provider.wire())
        .current_dir(repo_root);
    if let Some(endpoint) = endpoint {
        command.env("TORCA_RELAY_ENDPOINT", endpoint);
    }
    let result = run_external_command(&mut command, "lab peer build")?;
    if !result.status.success() {
        return Err("lab peer build failed".into());
    }
    Ok(repo_root.join(if cfg!(windows) {
        "target/debug/torca-lab-peer.exe"
    } else {
        "target/debug/torca-lab-peer"
    }))
}

/// Keep noisy build/deploy tools inside the cockpit. In TUI mode each line is
/// routed to the Logs view; plain mode preserves the normal terminal output.
struct ExternalCommandResult {
    status: std::process::ExitStatus,
    tail: String,
}

fn run_external_command(
    command: &mut Command,
    label: &str,
) -> Result<ExternalCommandResult, String> {
    if !tui::is_active() {
        let output = command.output().map_err(|error| format!("{label}: {error}"))?;
        return Ok(ExternalCommandResult {
            status: output.status,
            tail: command_output_tail(&output.stdout, &output.stderr),
        });
    }
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|error| format!("start {label}: {error}"))?;
    let stdout = child.stdout.take().ok_or_else(|| format!("{label} stdout unavailable"))?;
    let stderr = child.stderr.take().ok_or_else(|| format!("{label} stderr unavailable"))?;
    let (sender, receiver) = mpsc::channel();
    spawn_backend_reader(stdout, sender.clone(), false);
    spawn_backend_reader(stderr, sender, true);
    let mut captured = Vec::new();
    loop {
        while let Ok((is_stderr, line)) = receiver.try_recv() {
            tui::publish_backend_line(&line, is_stderr);
            captured.push(line);
            if captured.len() > 20 {
                captured.remove(0);
            }
        }
        if tui::cancel_requested() {
            let _ = child.kill();
        }
        if let Some(status) = child.try_wait().map_err(|error| format!("wait {label}: {error}"))? {
            while let Ok((is_stderr, line)) = receiver.try_recv() {
                tui::publish_backend_line(&line, is_stderr);
            }
            return Ok(ExternalCommandResult { status, tail: captured.join(" | ") });
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn default_lab_peer_path() -> PathBuf {
    PathBuf::from(if cfg!(windows) {
        "target/debug/torca-lab-peer.exe"
    } else {
        "target/debug/torca-lab-peer"
    })
}

fn start_active_battery_capture(
    serial: &str,
    root: &Path,
    require_unplugged: bool,
    require_screen_off: bool,
) -> Result<ActiveBatteryCapture, String> {
    let battery = adb_output(serial, &["shell", "dumpsys", "battery"])?;
    if require_unplugged
        && ["AC powered: true", "USB powered: true", "Wireless powered: true"]
            .iter()
            .any(|marker| battery.contains(marker))
    {
        return Err(format!("active battery soak requires unplugged Android device '{serial}'"));
    }
    let telecom = adb_output(serial, &["shell", "dumpsys", "telecom"])?;
    if telecom.lines().any(|line| line.contains("state=ACTIVE")) {
        return Err(format!(
            "active battery soak cannot run while an Android telephony call is active on '{serial}'"
        ));
    }
    let mut power = adb_output(serial, &["shell", "dumpsys", "power"])?;
    if require_screen_off && !is_screen_off(&power) {
        // `am start` used by the bridge can wake the display after the caller
        // checked it.  KEYCODE_SLEEP is idempotent (unlike KEYCODE_POWER,
        // which would accidentally wake an already sleeping device), so the
        // harness can enforce the measurement precondition itself.
        let status = Command::new("adb")
            .args(["-s", serial, "shell", "input", "keyevent", "223"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|error| format!("put Android display to sleep: {error}"))?;
        if !status.success() {
            return Err(format!("put Android display to sleep failed with {status}"));
        }
        thread::sleep(Duration::from_secs(1));
        power = adb_output(serial, &["shell", "dumpsys", "power"])?;
        if !is_screen_off(&power) {
            return Err(format!(
                "active battery soak requires screen-off Android device '{serial}'"
            ));
        }
    }
    fs::write(root.join("battery-start.txt"), battery)
        .map_err(|error| format!("write active battery baseline: {error}"))?;
    fs::write(root.join("power-start.txt"), power)
        .map_err(|error| format!("write active power baseline: {error}"))?;
    let status = Command::new("adb")
        .args(["-s", serial, "shell", "dumpsys", "batterystats", "--reset"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| format!("reset Android batterystats: {error}"))?;
    if !status.success() {
        return Err(format!("reset Android batterystats failed with {status}"));
    }
    Ok(ActiveBatteryCapture { serial: serial.to_owned(), root: root.to_owned() })
}

fn is_screen_off(power_dump: &str) -> bool {
    power_dump.contains("mWakefulness=Asleep")
        || power_dump.contains("mWakefulness=Dozing")
        || power_dump.contains("mWakefulness=Sleep")
}

fn adb_output(serial: &str, arguments: &[&str]) -> Result<String, String> {
    let output = Command::new("adb")
        .args(["-s", serial])
        .args(arguments)
        .output()
        .map_err(|error| format!("run adb for {serial}: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "adb command failed for {serial}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn prepare_notification_probe(serial: &str) -> Result<(), String> {
    let package = android_package();
    let component = format!("{package}/com.torca.host.SoakNotificationListener");
    // Android 13+ gates posted notifications behind a separate runtime
    // permission. SOAK is a debug-only flavor, so grant it through ADB rather
    // than making the operator race a permission dialog during provisioning.
    let sdk = adb_output(serial, &["shell", "getprop", "ro.build.version.sdk"])?;
    if sdk.trim().parse::<u32>().unwrap_or_default() >= 33 {
        adb_output(
            serial,
            &["shell", "pm", "grant", package, "android.permission.POST_NOTIFICATIONS"],
        )
        .map_err(|error| {
            format!(
                "SOAK notification permission was not granted on {serial}; allow notifications for {package} and retry: {error}"
            )
        })?;
    }
    adb_output(serial, &["shell", "cmd", "notification", "allow_listener", &component])?;
    adb_output(
        serial,
        &[
            "shell",
            "run-as",
            package,
            "sh",
            "-c",
            "mkdir -p files/torca && : > files/torca/soak-notifications.jsonl",
        ],
    )?;
    tui::publish_event(
        "notification_probe_ready",
        &serde_json::json!({"serial": serial, "component": component}),
    );
    Ok(())
}

fn collect_notification_probe(serial: &str, root: &Path, expected: u64) -> Result<u64, String> {
    let package = android_package();
    let output = adb_output(
        serial,
        &["shell", "run-as", package, "cat", "files/torca/soak-notifications.jsonl"],
    )?;
    fs::write(root.join("notifications.jsonl"), &output)
        .map_err(|error| format!("write notification observations: {error}"))?;
    let mut malformed = 0_u64;
    let mut observed_ids = std::collections::HashSet::new();
    let mut observed = 0_u64;
    for line in output.lines().filter(|line| !line.trim().is_empty()) {
        let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
            malformed = malformed.saturating_add(1);
            continue;
        };
        if event.get("channel").and_then(serde_json::Value::as_str)
            != Some("torca_private_messages")
        {
            continue;
        }
        observed = observed.saturating_add(1);
        let id = format!(
            "{}:{}",
            event.get("id").and_then(serde_json::Value::as_i64).unwrap_or_default(),
            event.get("tag").and_then(serde_json::Value::as_str).unwrap_or_default()
        );
        if !observed_ids.insert(id) {
            return Err("notification assertion failed: duplicate notification id observed".into());
        }
    }
    if malformed != 0 {
        return Err(format!(
            "notification assertion failed: {malformed} malformed listener records"
        ));
    }
    if observed < expected {
        return Err(format!(
            "notification assertion failed: expected at least {expected} private-message notifications, observed {observed}"
        ));
    }
    tui::publish_event(
        events::EventKind::NotificationAssertionPassed.as_str(),
        &serde_json::json!({"expected": expected, "observed": observed}),
    );
    Ok(observed)
}

fn capture_adb_file(serial: &str, arguments: &[&str], path: &Path) -> Result<(), String> {
    let output = adb_output(serial, arguments)?;
    fs::write(path, output).map_err(|error| format!("write {}: {error}", path.display()))
}

fn pair_mesh(peers: &mut [Participant], timeline: &mut File) -> Result<(), String> {
    for left in 0..peers.len() {
        for right in (left + 1)..peers.len() {
            let (left_peer, right_peer) = peers.split_at_mut(right);
            let left_peer = &mut left_peer[left];
            let right_peer = &mut right_peer[0];
            let pair_id = format!("{}-{}", left_peer.name(), right_peer.name());
            let invitation = retry_operation(
                left_peer,
                "pairing.create",
                serde_json::json!({}),
                &format!("pair-create-{pair_id}"),
            )?;
            let pairing = wait_for_pairing_invitation(left_peer, invitation)?;
            let join_payload = pairing_join_payload(&pairing)?;
            let session_id = pairing
                .get("id")
                .and_then(serde_json::Value::as_str)
                .ok_or("pairing session id missing")?;
            retry_operation(
                right_peer,
                "pairing.join",
                join_payload,
                &format!("pair-join-{pair_id}"),
            )?;
            wait_for_pairing_state(left_peer, session_id, &["peer_joined", "awaiting_approval"])?;
            retry_operation(
                left_peer,
                "pairing.approve",
                serde_json::json!({"sessionIdHex": session_id}),
                &format!("pair-approve-{pair_id}"),
            )?;
            wait_for_conversation(left_peer, right_peer)?;
            record(
                timeline,
                "pairing_completed",
                serde_json::json!({
                    "left": left_peer.name(),
                    "right": right_peer.name()
                }),
            )?;
        }
    }
    Ok(())
}

/// Pair every fake peer directly with Android. This gives the physical device
/// exactly `fake_peers` contacts for the Android star. Bot-to-bot traffic is
/// added separately by `pair_bot_ring` so the Android workload stays visible.
fn pair_android_star(peers: &mut [Participant], timeline: &mut File) -> Result<(), String> {
    let android_index = peers
        .iter()
        .position(|peer| matches!(peer, Participant::Android(_)))
        .ok_or("Android participant missing from active-messaging scenario")?;
    let fake_count = peers
        .iter()
        .filter(|peer| matches!(peer, Participant::Fake(_) | Participant::Remote(_)))
        .count();
    let existing = {
        let android = &mut peers[android_index];
        snapshot_with_retry(android, "pairing-reuse-check")
            .map(|snapshot| contact_count(&snapshot))?
    };
    if existing >= fake_count {
        record(
            timeline,
            "pairing_reused",
            serde_json::json!({"android": peers[android_index].name(), "contacts": existing, "bots": fake_count}),
        )?;
        return Ok(());
    }
    for fake_index in 0..peers.len() {
        if fake_index == android_index
            || !matches!(peers[fake_index], Participant::Fake(_) | Participant::Remote(_))
        {
            continue;
        }
        let (android, fake) = two_participants_mut(peers, android_index, fake_index);
        pair_participants(android, fake, timeline)?;
    }
    Ok(())
}

/// Keep Android's five direct contacts while giving the fake devices their
/// own independent traffic path. Existing bot relationships are reused by
/// checking each participant's conversation projection before pairing.
fn pair_bot_ring(peers: &mut [Participant], timeline: &mut File) -> Result<(), String> {
    let bot_indices = peers
        .iter()
        .enumerate()
        .filter_map(|(index, peer)| {
            matches!(peer, Participant::Fake(_) | Participant::Remote(_)).then_some(index)
        })
        .collect::<Vec<_>>();
    if bot_indices.len() < 2 {
        return Ok(());
    }
    for (position, &left_index) in bot_indices.iter().enumerate() {
        let right_index = bot_indices[(position + 1) % bot_indices.len()];
        let existing = snapshot_with_retry(&mut peers[left_index], "bot-ring-check")
            .map(|snapshot| conversation_count(&snapshot))?;
        if existing >= 2 {
            continue;
        }
        let (left, right) = two_participants_mut(peers, left_index, right_index);
        pair_participants(left, right, timeline)?;
        record(
            timeline,
            "bot_ring_pairing",
            serde_json::json!({"left": left.name(), "right": right.name()}),
        )?;
    }
    Ok(())
}

fn validate_active_messaging_preflight(
    peers: &mut [Participant],
    expected_contacts: usize,
    expected_conversations: usize,
    expected_fake_peers: usize,
    timeline: &mut File,
) -> Result<(), String> {
    let android = peers
        .iter_mut()
        .find(|peer| matches!(peer, Participant::Android(_)))
        .ok_or("Android participant missing from active-messaging preflight")?;
    let snapshot = snapshot_with_retry(android, "active-contacts")?;
    let contacts = contact_count(&snapshot);
    let conversations = conversation_count(&snapshot);
    if contacts < expected_contacts || conversations < expected_conversations {
        return Err(format!(
            "active-messaging preflight failed: Android has {contacts} contacts and {conversations} conversations; expected at least {expected_contacts} contacts and {expected_conversations} conversations from {expected_fake_peers} soak bots"
        ));
    }
    record(
        timeline,
        "active_preflight_passed",
        serde_json::json!({
            "android": android.name(),
            "contacts": contacts,
            "conversations": conversations,
            "expectedContacts": expected_contacts,
            "expectedConversations": expected_conversations,
            "expectedBots": expected_fake_peers,
        }),
    )
}

fn contact_count(response: &serde_json::Value) -> usize {
    response.pointer("/snapshot/contacts").and_then(serde_json::Value::as_array).map_or(0, Vec::len)
}

fn conversation_count(response: &serde_json::Value) -> usize {
    response
        .pointer("/snapshot/conversations")
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len)
}

fn two_participants_mut(
    peers: &mut [Participant],
    left: usize,
    right: usize,
) -> (&mut Participant, &mut Participant) {
    debug_assert_ne!(left, right);
    if left < right {
        let (before, after) = peers.split_at_mut(right);
        (&mut before[left], &mut after[0])
    } else {
        let (before, after) = peers.split_at_mut(left);
        (&mut after[0], &mut before[right])
    }
}

fn pair_participants(
    inviter: &mut Participant,
    joiner: &mut Participant,
    timeline: &mut File,
) -> Result<(), String> {
    let pair_id = format!("{}-{}", inviter.name(), joiner.name());
    wait_for_pairing_network(inviter)?;
    let invitation = retry_operation(
        inviter,
        "pairing.create",
        serde_json::json!({}),
        &format!("pair-create-{pair_id}"),
    )?;
    let pairing = wait_for_pairing_invitation(inviter, invitation)?;
    let join_payload = pairing_join_payload(&pairing)?;
    let session_id = pairing
        .get("id")
        .and_then(serde_json::Value::as_str)
        .ok_or("pairing session id missing")?;
    retry_operation(joiner, "pairing.join", join_payload, &format!("pair-join-{pair_id}"))?;
    wait_for_pairing_state(inviter, session_id, &["peer_joined", "awaiting_approval"])?;
    record(
        timeline,
        "pairing_joined",
        serde_json::json!({
            "inviter": inviter.name(),
            "joiner": joiner.name(),
            "sessionId": session_id,
        }),
    )?;
    retry_operation(
        inviter,
        "pairing.approve",
        serde_json::json!({"sessionIdHex": session_id}),
        &format!("pair-approve-{pair_id}"),
    )?;
    record(
        timeline,
        "pairing_approved",
        serde_json::json!({
            "inviter": inviter.name(),
            "joiner": joiner.name(),
            "sessionId": session_id,
        }),
    )?;
    wait_for_conversation(inviter, joiner)?;
    record(
        timeline,
        "pairing_completed",
        serde_json::json!({"left": inviter.name(), "right": joiner.name(), "topology": "android-star"}),
    )
}

/// Builds the provider-neutral join payload from the invitation projection.
/// Direct providers carry a bounded bootstrap descriptor in the v3 URI; a
/// short code alone is intentionally rejected by the native runtime because
/// it cannot identify the creator's endpoint.
fn pairing_join_payload(pairing: &serde_json::Value) -> Result<serde_json::Value, String> {
    let code =
        pairing.get("code").and_then(serde_json::Value::as_str).ok_or("pairing code missing")?;
    let mut payload = serde_json::json!({"code": code});
    let Some(uri) = pairing.get("inviteUri").and_then(serde_json::Value::as_str) else {
        return Ok(payload);
    };
    let query = uri
        .split_once('?')
        .map(|(_, query)| query)
        .ok_or_else(|| "pairing invitation URI is malformed".to_owned())?;
    let mut provider = None;
    let mut bootstrap = None;
    for segment in query.split('&') {
        let Some((key, value)) = segment.split_once('=') else { continue };
        match key {
            "provider" => provider = Some(value),
            "bootstrap" => bootstrap = Some(value),
            _ => {}
        }
    }
    if let (Some(provider), Some(payload_hex)) = (provider, bootstrap) {
        if provider.is_empty()
            || payload_hex.is_empty()
            || payload_hex.len() % 2 != 0
            || !payload_hex.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err("pairing bootstrap descriptor is malformed".into());
        }
        payload["bootstrap"] = serde_json::json!({
            "provider": provider,
            "payloadHex": payload_hex,
        });
    } else if provider.is_some_and(|value| value != "tor") {
        return Err(format!(
            "direct provider invitation is missing its bootstrap descriptor: {uri}"
        ));
    }
    Ok(payload)
}

/// Pairing writes the invitation to the relay. Do not create it while the
/// local Tor process is merely `LOCAL_READY`: that command can be accepted
/// locally and then remain invisible forever if the secure relay lane is still
/// warming up. The fixture deploy therefore gates pairing on real relay
/// evidence, not only on the aggregate bootstrap phase.
fn wait_for_pairing_network(peer: &mut Participant) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(300);
    let mut attempt = 0u32;
    let mut last_phase = "unknown".to_owned();
    let mut last_provider = "unknown".to_owned();
    let mut last_communication = "unknown".to_owned();
    let mut last_relay = "unknown".to_owned();
    let mut last_error = String::new();
    while Instant::now() < deadline && !tui::cancel_requested() {
        attempt = attempt.saturating_add(1);
        match peer.request(&format!("pairing-network-{attempt}"), "snapshot", serde_json::json!({}))
        {
            Ok(snapshot) => {
                snapshot
                    .pointer("/snapshot/bootstrapPhase")
                    .or_else(|| snapshot.get("bootstrapPhase"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("unknown")
                    .clone_into(&mut last_phase);
                snapshot
                    .pointer("/snapshot/communicationProvider")
                    .or_else(|| snapshot.get("communicationProvider"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("tor")
                    .clone_into(&mut last_provider);
                snapshot
                    .pointer("/snapshot/communicationState")
                    .or_else(|| snapshot.get("communicationState"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("unknown")
                    .clone_into(&mut last_communication);
                if last_provider == "tor" {
                    snapshot
                        .pointer("/snapshot/transport/tor/state")
                        .or_else(|| snapshot.pointer("/transport/tor/state"))
                        .or_else(|| snapshot.pointer("/snapshot/torState"))
                        .or_else(|| snapshot.get("torState"))
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("unknown")
                        .clone_into(&mut last_communication);
                }
                snapshot
                    .pointer("/snapshot/transport/relay/state")
                    .or_else(|| snapshot.pointer("/transport/relay/state"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("unknown")
                    .clone_into(&mut last_relay);
                let provider_ready = matches!(last_communication.as_str(), "ready" | "healthy");
                let tor_relay_ready =
                    last_provider != "tor" || matches!(last_relay.as_str(), "ready" | "healthy");
                if provider_ready && tor_relay_ready {
                    return Ok(());
                }
                last_error.clear();
            }
            Err(error) => last_error = error,
        }
        tui::controlled_sleep(Duration::from_secs(2));
    }
    Err(format!(
        "{} pairing network did not become ready within 300s: phase={last_phase} provider={last_provider} communication={last_communication} relay={last_relay}; last_error={last_error}",
        peer.name()
    ))
}

/// A reused relationship must not be gated on relay publication. Once the
/// local Tor runtime is ready, the existing onion peer session can recover
/// independently and the workload can exercise that path.
fn wait_for_provider_ready(peer: &mut Participant) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(180);
    let mut attempt = 0u32;
    let mut last_provider = "unknown".to_owned();
    let mut last_communication = "unknown".to_owned();
    let mut last_error = String::new();
    while Instant::now() < deadline && !tui::cancel_requested() {
        attempt = attempt.saturating_add(1);
        match peer.request(&format!("provider-ready-{attempt}"), "snapshot", serde_json::json!({}))
        {
            Ok(snapshot) => {
                snapshot
                    .pointer("/snapshot/communicationProvider")
                    .or_else(|| snapshot.get("communicationProvider"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("tor")
                    .clone_into(&mut last_provider);
                snapshot
                    .pointer("/snapshot/communicationState")
                    .or_else(|| snapshot.get("communicationState"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("unknown")
                    .clone_into(&mut last_communication);
                if matches!(last_communication.as_str(), "ready" | "healthy") {
                    return Ok(());
                }
                last_error.clear();
            }
            Err(error) => last_error = error,
        }
        tui::controlled_sleep(Duration::from_secs(2));
    }
    Err(format!(
        "{} {} communication provider did not become ready within 180s: provider={last_provider} state={last_communication}; last_error={last_error}",
        peer.name(),
        last_provider.to_ascii_uppercase(),
    ))
}

fn retry_operation(
    peer: &mut Participant,
    operation: &str,
    payload: serde_json::Value,
    request_id: &str,
) -> Result<serde_json::Value, String> {
    let deadline = Instant::now() + Duration::from_secs(120);
    let mut attempt = 0u32;
    let mut last_error = String::new();
    while Instant::now() < deadline && !tui::cancel_requested() {
        attempt = attempt.saturating_add(1);
        // Reuse one correlation ID across transport retries. The production
        // runtime can then return the original result instead of executing a
        // command twice when the first response was merely delayed.
        match peer.request(request_id, operation, payload.clone()) {
            Ok(response) => return Ok(response),
            Err(error) => last_error = error,
        }
        tui::controlled_sleep(Duration::from_secs(2));
    }
    Err(format!("{} {operation} did not succeed within 120s: {last_error}", peer.name()))
}

fn wait_for_pairing_invitation(
    peer: &mut Participant,
    initial: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let mut target_id = initial
        .get("resourceId")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    let deadline = Instant::now() + Duration::from_secs(180);
    let mut latest = initial.clone();
    let mut attempt = 0u32;
    let mut last_error = None;
    let mut relay_degraded_since = None;
    let mut observed_once = false;
    // A ready provider returns the complete invitation in the command
    // response.  Prefer that authoritative response over waiting for a
    // second snapshot round: the snapshot waiter can be blocked behind a
    // provider-owned maintenance turn (notably Iroh's first accept cycle),
    // even though the invitation is already durable and usable.
    if initial.get("inviteUri").and_then(serde_json::Value::as_str).is_some() {
        let invite_uri =
            initial.get("inviteUri").and_then(serde_json::Value::as_str).unwrap_or_default();
        let code = invite_uri
            .split_once('?')
            .and_then(|(_, query)| {
                query.split('&').find_map(|part| {
                    let (key, value) = part.split_once('=')?;
                    (key == "code").then_some(value)
                })
            })
            .or_else(|| initial.get("code").and_then(serde_json::Value::as_str));
        if let (Some(id), Some(code)) = (target_id.as_deref(), code) {
            // The command response is the authoritative, correlation-bound
            // invitation.  Do not force the SOAK coordinator through a second
            // snapshot call, which can be blocked by the provider's first
            // polling turn while the durable pairing is already usable.
            return Ok(serde_json::json!({
                "id": id,
                "code": code,
                "inviteUri": invite_uri,
                "state": "open"
            }));
        }
        if let Some(pairing) = initial
            .pointer("/snapshot/pairings")
            .and_then(serde_json::Value::as_array)
            .and_then(|items| {
                target_id.as_deref().and_then(|id| {
                    items
                        .iter()
                        .find(|item| item.get("id").and_then(serde_json::Value::as_str) == Some(id))
                })
            })
            .or_else(|| {
                initial
                    .pointer("/snapshot/pairings")
                    .and_then(serde_json::Value::as_array)
                    .and_then(|items| items.first())
            })
        {
            tui::publish_event("pairing_invitation_observed", pairing);
            return Ok(pairing.clone());
        }
    }
    loop {
        // Fake peers expose the durable operation resource id directly. The
        // Android ScenarioBridge returns a compact command result and puts
        // the created pairing only in its snapshot projection, so discover
        // that id from the first published pairing when necessary.
        if target_id.is_none() {
            target_id = first_pairing_id(&latest);
        }
        if let Some(target_id) = target_id.as_deref() {
            if let Some(pairing) = pairing_by_id(&latest, target_id) {
                // The first create response can contain only the endpoint's
                // local address. Give direct providers one refresh cycle to
                // publish relay/public candidates before handing the URI to a
                // joiner; otherwise Iroh joins are deterministically rejected
                // with a generic Communication error on another network
                // namespace/process.
                if observed_once {
                    tui::publish_event("pairing_invitation_observed", pairing);
                    return Ok(pairing.clone());
                }
                observed_once = true;
            }
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "{} did not publish a pairing invitation within 180s (resource_id={}); last_response={latest}; last_transport_error={}",
                peer.name(),
                target_id.as_deref().unwrap_or("unknown"),
                last_error.as_deref().unwrap_or("none")
            ));
        }
        tui::controlled_sleep(Duration::from_secs(2));
        attempt = attempt.saturating_add(1);
        match peer.request(
            &format!("pair-create-wait-{attempt}"),
            "snapshot",
            serde_json::json!({}),
        ) {
            Ok(snapshot) => {
                latest = snapshot;
                last_error = None;
                let relay_state = latest
                    .pointer("/snapshot/transport/relay/state")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default();
                if relay_state == "degraded" {
                    let since = relay_degraded_since.get_or_insert_with(Instant::now);
                    if since.elapsed() >= Duration::from_secs(20) {
                        return Err(format!(
                            "{} pairing stopped: relay remained degraded for 20s; transport={}",
                            peer.name(),
                            latest.pointer("/snapshot/transport/relay").unwrap_or(&latest)
                        ));
                    }
                } else {
                    relay_degraded_since = None;
                }
            }
            Err(error) => {
                // A cold Arti bootstrap can temporarily occupy the production
                // runtime actor longer than the lab peer's bounded 5-second
                // query timeout. Pairing itself is asynchronous, so one busy
                // snapshot is not evidence that the operation or runtime died.
                last_error = Some(error);
            }
        }
    }
}

fn first_pairing_id(response: &serde_json::Value) -> Option<String> {
    response.pointer("/snapshot/pairings").and_then(serde_json::Value::as_array).and_then(
        |pairings| {
            pairings.iter().find_map(|pairing| {
                pairing
                    .get("id")
                    .and_then(serde_json::Value::as_str)
                    .filter(|id| !id.is_empty())
                    .map(str::to_owned)
            })
        },
    )
}

fn pairing_by_id<'a>(
    response: &'a serde_json::Value,
    target_id: &str,
) -> Option<&'a serde_json::Value> {
    response.pointer("/snapshot/pairings").and_then(serde_json::Value::as_array).and_then(|items| {
        items.iter().find(|pairing| {
            pairing.get("id").and_then(serde_json::Value::as_str) == Some(target_id)
        })
    })
}

fn wait_for_pairing_state(
    peer: &mut Participant,
    session_id: &str,
    expected_states: &[&str],
) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(180);
    let mut attempt = 0u32;
    let mut last_state = "missing".to_owned();
    let mut last_error = None;
    while Instant::now() < deadline && !tui::cancel_requested() {
        attempt = attempt.saturating_add(1);
        match peer.request(
            &format!("pair-state-{session_id}-{attempt}"),
            "snapshot",
            serde_json::json!({}),
        ) {
            Ok(snapshot) => {
                last_error = None;
                if let Some(pairing) = pairing_by_id(&snapshot, session_id) {
                    pairing
                        .get("state")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("unknown")
                        .clone_into(&mut last_state);
                    if expected_states.contains(&last_state.as_str()) {
                        return Ok(());
                    }
                } else {
                    "missing".clone_into(&mut last_state);
                }
            }
            Err(error) => last_error = Some(error),
        }
        tui::controlled_sleep(Duration::from_secs(2));
    }
    Err(format!(
        "{} pairing session {session_id} did not reach [{}] within 180s; last_state={last_state}; last_transport_error={}",
        peer.name(),
        expected_states.join(", "),
        last_error.as_deref().unwrap_or("none")
    ))
}

fn wait_for_conversation(left: &mut Participant, right: &mut Participant) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(180);
    while Instant::now() < deadline && !tui::cancel_requested() {
        let left_snapshot = snapshot_with_retry(left, "pair-check-left")?;
        let right_snapshot = snapshot_with_retry(right, "pair-check-right")?;
        if first_conversation_id(&left_snapshot).is_some()
            && first_conversation_id(&right_snapshot).is_some()
        {
            return Ok(());
        }
        tui::controlled_sleep(Duration::from_secs(2));
    }
    Err(format!(
        "pairing did not create conversations for {} and {} within 180s",
        left.name(),
        right.name()
    ))
}

fn snapshot_with_retry(
    peer: &mut Participant,
    request_prefix: &str,
) -> Result<serde_json::Value, String> {
    let deadline = Instant::now() + Duration::from_secs(120);
    let mut attempt = 0u32;
    let mut last_error = String::new();
    while Instant::now() < deadline && !tui::cancel_requested() {
        attempt = attempt.saturating_add(1);
        match peer.request(
            &format!("{request_prefix}-{attempt}"),
            "snapshot",
            serde_json::json!({}),
        ) {
            Ok(snapshot) => return Ok(snapshot),
            Err(error) => last_error = error,
        }
        tui::controlled_sleep(Duration::from_secs(2));
    }
    Err(format!("{} snapshot did not succeed within 120s: {last_error}", peer.name()))
}

impl AndroidBridge {
    fn connect(serial: &str) -> Result<Self, String> {
        let package = android_package();
        let package_check = Command::new("adb")
            .args(["-s", serial, "shell", "pm", "path", package])
            .output()
            .map_err(|error| format!("check Android package {package}: {error}"))?;
        if !package_check.status.success()
            || String::from_utf8_lossy(&package_check.stdout).trim().is_empty()
        {
            let hint = if package.ends_with(".soak") {
                "Run .\\scripts\\soak.ps1 cockpit --android <adb-serial> to build/install the SOAK1 flavor."
            } else {
                "Run the Android deploy first, or use .\\scripts\\soak.ps1 cockpit for an Android SOAK1 run."
            };
            return Err(format!(
                "Android package '{package}' is not installed on {serial}. {hint}"
            ));
        }
        let activity = android_launchable_activity(serial)?;
        tui::publish_event(
            "android_bridge_starting",
            &serde_json::json!({"serial": serial, "activity": activity}),
        );
        let launch = Command::new("adb")
            .args(["-s", serial, "shell", "am", "start", "-W", "-n", &activity])
            .output()
            .map_err(|error| format!("start Android activity: {error}"))?;
        if !launch.status.success() {
            return Err(format!(
                "Android activity '{activity}' failed to start on {serial}: {}",
                String::from_utf8_lossy(&launch.stderr).trim()
            ));
        }
        let deadline = Instant::now() + Duration::from_secs(120);
        // Dart's Directory.systemTemp maps to `code_cache` on Android. Keep
        // the cache variants for older builds/devices, but always probe the
        // actual Android temp location first so a healthy bridge is not
        // reported as missing for the full discovery timeout.
        let discovery_paths = [
            "code_cache/torca-scenario.json",
            "code_cache/torca/torca-scenario.json",
            "cache/torca-scenario.json",
            "cache/torca/torca-scenario.json",
        ];
        let mut permission_reported = false;
        let discovery = 'discovery: loop {
            for path in discovery_paths {
                let output = Command::new("adb")
                    .args(["-s", serial, "exec-out", "run-as", android_package(), "cat", path])
                    .output()
                    .map_err(|error| format!("read Android scenario discovery: {error}"))?;
                if output.status.success() {
                    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
                        break 'discovery value;
                    }
                }
            }
            if !permission_reported && android_permission_prompt_visible(serial) {
                permission_reported = true;
                tui::publish_event(
                    "android_permission_required",
                    &serde_json::json!({"serial": serial}),
                );
            }
            if Instant::now() >= deadline {
                let hint = if permission_reported {
                    " Android is showing a permission prompt; approve it, then restart the soak."
                } else {
                    " Confirm that this is a debug build and that the ScenarioBridge is enabled."
                };
                let flavor_hint = if package.ends_with(".soak") {
                    " The SOAK1 package is installed, but its debug ScenarioBridge did not publish discovery; verify the app is foregrounded and built with TORCA_SOAK_MODE=true."
                } else {
                    " This is the normal package. SOAK1 requires the .soak flavor; run .\\scripts\\soak.ps1 cockpit instead of a normal deploy."
                };
                return Err(format!(
                    "Android scenario bridge did not start on {serial} (package={package}).{hint}{flavor_hint}"
                ));
            }
            tui::controlled_sleep(Duration::from_secs(1));
        };
        let token = discovery
            .get("token")
            .and_then(serde_json::Value::as_str)
            .ok_or("Android scenario token missing")?
            .to_owned();
        let device_port = discovery
            .get("port")
            .and_then(serde_json::Value::as_u64)
            .ok_or("Android scenario port missing")?;
        let probe_listener = TcpListener::bind(("127.0.0.1", 0))
            .map_err(|error| format!("reserve Android bridge port: {error}"))?;
        let host_port = probe_listener
            .local_addr()
            .map_err(|error| format!("read Android bridge port: {error}"))?
            .port();
        drop(probe_listener);
        let forward = Command::new("adb")
            .args([
                "-s",
                serial,
                "forward",
                &format!("tcp:{host_port}"),
                &format!("tcp:{device_port}"),
            ])
            .status()
            .map_err(|error| format!("forward Android scenario bridge: {error}"))?;
        if !forward.success() {
            return Err(format!("adb forward failed on {serial}"));
        }
        Ok(Self { serial: serial.to_owned(), token, host_port })
    }

    fn request(
        &mut self,
        id: &str,
        operation: &str,
        extra: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let mut request = serde_json::Map::new();
        request.insert("id".into(), serde_json::Value::String(id.into()));
        request.insert("op".into(), serde_json::Value::String(operation.into()));
        if let serde_json::Value::Object(fields) = extra {
            request.extend(fields);
        }
        let body = serde_json::to_vec(&serde_json::Value::Object(request))
            .map_err(|error| error.to_string())?;
        let mut stream = TcpStream::connect(("127.0.0.1", self.host_port))
            .map_err(|error| format!("connect Android scenario bridge: {error}"))?;
        stream
            .set_read_timeout(Some(Duration::from_secs(15)))
            .map_err(|error| error.to_string())?;
        write!(
            stream,
            "POST / HTTP/1.1\r\nHost: localhost\r\nX-Torca-Scenario-Token: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            self.token,
            body.len()
        )
        .map_err(|error| error.to_string())?;
        stream.write_all(&body).map_err(|error| error.to_string())?;
        let mut response = Vec::new();
        stream.read_to_end(&mut response).map_err(|error| error.to_string())?;
        let separator = response
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .ok_or("invalid Android scenario response")?;
        let value: serde_json::Value = serde_json::from_slice(&response[separator + 4..])
            .map_err(|error| format!("decode Android scenario response: {error}"))?;
        if value.get("status").and_then(serde_json::Value::as_str) != Some("succeeded") {
            return Err(format!(
                "Android operation {operation} failed: {}",
                value.get("error").unwrap_or(&serde_json::Value::Null)
            ));
        }
        Ok(value.get("result").cloned().unwrap_or(value))
    }

    fn set_wifi(&self, enabled: bool) -> Result<(), String> {
        let state = if enabled { "enable" } else { "disable" };
        let status = Command::new("adb")
            .args(["-s", &self.serial, "shell", "svc", "wifi", state])
            .status()
            .map_err(|error| format!("set Android Wi-Fi {state}: {error}"))?;
        status
            .success()
            .then_some(())
            .ok_or_else(|| format!("Android Wi-Fi {state} failed on {}", self.serial))
    }
}

fn android_permission_prompt_visible(serial: &str) -> bool {
    Command::new("adb")
        .args(["-s", serial, "shell", "dumpsys", "activity", "activities"])
        .output()
        .map(|output| {
            let state = String::from_utf8_lossy(&output.stdout).to_ascii_lowercase();
            state.contains("permissioncontroller") || state.contains("grantpermissionsactivity")
        })
        .unwrap_or(false)
}

fn android_launchable_activity(serial: &str) -> Result<String, String> {
    let state = Command::new("adb")
        .args(["-s", serial, "get-state"])
        .output()
        .map_err(|error| format!("check Android device {serial}: {error}"))?;
    if !state.status.success() || String::from_utf8_lossy(&state.stdout).trim() != "device" {
        return Err(format!(
            "Android device '{serial}' is not ready (adb get-state returned '{}')",
            String::from_utf8_lossy(&state.stdout).trim()
        ));
    }

    let package = android_package();
    let installed = Command::new("adb")
        .args(["-s", serial, "shell", "pm", "path", package])
        .output()
        .map_err(|error| format!("check Android package on {serial}: {error}"))?;
    if !installed.status.success()
        || !String::from_utf8_lossy(&installed.stdout)
            .lines()
            .any(|line| line.starts_with("package:"))
    {
        return Err(format!(
            "Android package '{package}' is not installed on '{serial}'. Install the debug APK or run Run-TorcaBatterySoak.ps1 first."
        ));
    }

    let resolved = Command::new("adb")
        .args([
            "-s",
            serial,
            "shell",
            "cmd",
            "package",
            "resolve-activity",
            "--brief",
            "-a",
            "android.intent.action.MAIN",
            "-c",
            "android.intent.category.LAUNCHER",
            package,
        ])
        .output()
        .map_err(|error| format!("resolve Android launcher on {serial}: {error}"))?;
    let component = parse_launchable_activity(package, &String::from_utf8_lossy(&resolved.stdout));
    component.ok_or_else(|| {
        format!(
            "Android package '{package}' has no launchable MAIN/LAUNCHER activity on '{serial}'. Install a debug APK with ScenarioBridge enabled."
        )
    })
}

fn ensure_android_deployed(
    repo_root: &Path,
    serial: &str,
    clean: bool,
    reuse_built_artifact: bool,
    provider: CommunicationProvider,
) -> Result<(), String> {
    let state = Command::new("adb")
        .args(["-s", serial, "get-state"])
        .output()
        .map_err(|error| format!("check Android device {serial}: {error}"))?;
    if !state.status.success() || String::from_utf8_lossy(&state.stdout).trim() != "device" {
        return Err(format!(
            "Android device '{serial}' is not ready (adb get-state returned '{}')",
            String::from_utf8_lossy(&state.stdout).trim()
        ));
    }
    if let Err(error) =
        run_typed_android_deploy(repo_root, serial, clean, reuse_built_artifact, provider)
    {
        let context = android_preflight_context(serial);
        return Err(format!("{error}; android_preflight={context}"));
    }
    android_launchable_activity(serial).map(|_| ()).map_err(|error| {
        format!(
            "Android auto-deploy completed but the client is still not launchable on '{serial}': {error}"
        )
    })
}

fn android_preflight_context(serial: &str) -> String {
    let read = |arguments: &[&str]| {
        Command::new("adb")
            .args(["-s", serial])
            .args(arguments)
            .output()
            .ok()
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "unknown".to_owned())
    };
    format!(
        "manufacturer={},sdk={},transport={},install_via_usb={},verifier_verify_adb_installs={}",
        read(&["shell", "getprop", "ro.product.manufacturer"]),
        read(&["shell", "getprop", "ro.build.version.sdk"]),
        read(&["get-state"]),
        read(&["shell", "settings", "get", "secure", "install_via_usb"]),
        read(&["shell", "settings", "get", "global", "verifier_verify_adb_installs"]),
    )
}

fn run_typed_android_deploy(
    repo_root: &Path,
    serial: &str,
    clean: bool,
    reuse_built_artifact: bool,
    provider: CommunicationProvider,
) -> Result<(), String> {
    let paths = torca_deploy::persistence::DeployPaths {
        repo_root: repo_root.to_owned(),
        state_root: repo_root.join(".torca/deploy"),
    };
    let sink: torca_deploy::process::OutputSink = Arc::new(|line, stderr| {
        tui::publish_backend_line(line, stderr);
    });
    let executor = torca_deploy::DeployExecutor::with_runner(
        torca_deploy::persistence::StateStore::new(paths),
        Arc::new(torca_deploy::process::SystemCommandRunner::with_sink(sink)),
    );
    if !reuse_built_artifact {
        let mut build_plan = torca_deploy::domain::DeployPlan::normal(
            torca_deploy::domain::DeployAction::BuildArtifacts,
            vec![torca_deploy::domain::Target::Android],
            torca_deploy::domain::Configuration::Debug,
        );
        // Build only the ABI reported by the selected physical device. Without
        // the exact device the generic deploy plan intentionally builds every
        // supported Android ABI, which is unnecessary for an interactive soak.
        build_plan.device = Some(serial.to_owned());
        build_plan.client_build = torca_deploy::domain::BuildPolicy::Rebuild;
        build_plan.provider_service_build = torca_deploy::domain::BuildPolicy::Reuse;
        build_plan.communication_provider = match provider {
            CommunicationProvider::Tor => torca_deploy::domain::CommunicationProvider::Tor,
            CommunicationProvider::Iroh => torca_deploy::domain::CommunicationProvider::Iroh,
        };
        build_plan.validation = torca_deploy::domain::ValidationLevel::Skip;
        build_plan.launch = torca_deploy::domain::LaunchPolicy::Skip;
        let build_run = executor
            .create_run(build_plan)
            .map_err(|error| format!("create typed Android build plan: {error}"))?;
        executor
            .execute(build_run, torca_deploy::ExecutionMode::Execute)
            .map_err(|error| format!("typed Android artifact build failed: {error}"))?;
    }

    let mut install_plan = torca_deploy::domain::DeployPlan::normal(
        torca_deploy::domain::DeployAction::RedeployCurrent,
        vec![torca_deploy::domain::Target::Android],
        torca_deploy::domain::Configuration::Debug,
    );
    install_plan.device = Some(serial.to_owned());
    install_plan.client_build = torca_deploy::domain::BuildPolicy::Reuse;
    install_plan.provider_service_build = torca_deploy::domain::BuildPolicy::Reuse;
    install_plan.communication_provider = match provider {
        CommunicationProvider::Tor => torca_deploy::domain::CommunicationProvider::Tor,
        CommunicationProvider::Iroh => torca_deploy::domain::CommunicationProvider::Iroh,
    };
    install_plan.validation = torca_deploy::domain::ValidationLevel::Skip;
    install_plan.client_data = if clean {
        torca_deploy::domain::ClientDataPolicy::ResetAll
    } else {
        torca_deploy::domain::ClientDataPolicy::Preserve
    };
    install_plan.launch = torca_deploy::domain::LaunchPolicy::Restart;
    let install_run = executor
        .create_run(install_plan)
        .map_err(|error| format!("create typed Android install plan: {error}"))?;
    executor
        .execute(install_run, torca_deploy::ExecutionMode::Execute)
        .map_err(|error| format!("typed Android install/launch failed: {error}"))?;
    Ok(())
}

fn parse_launchable_activity(package: &str, output: &str) -> Option<String> {
    let prefix = format!("{package}/");
    output.lines().map(str::trim).find(|line| line.starts_with(&prefix)).map(str::to_owned)
}

fn first_conversation_id(response: &serde_json::Value) -> Option<String> {
    conversation_id_at(response, 0)
}

fn conversation_id_at(response: &serde_json::Value, index: usize) -> Option<String> {
    response
        .pointer("/snapshot/conversations")
        .and_then(serde_json::Value::as_array)
        .and_then(|items| items.get(index))
        .and_then(|conversation| conversation.get("id"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
}

fn first_contact_id(response: &serde_json::Value) -> Option<String> {
    response
        .pointer("/snapshot/contacts")
        .and_then(serde_json::Value::as_array)
        .and_then(|items| items.first())
        .and_then(|contact| contact.get("id"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
}

fn validate_fixture_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name.len() > 64 {
        return Err("fixture-name must contain 1-64 characters".into());
    }
    if !name.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')) {
        return Err("fixture-name may contain only ASCII letters, digits, '-' and '_'".into());
    }
    Ok(())
}

fn fixture_path(repo_root: &Path, name: &str) -> PathBuf {
    repo_root.join(".torca/soak/fixtures").join(format!("{name}.json"))
}

fn load_fixture_manifest(repo_root: &Path, name: &str) -> Result<FixtureManifest, String> {
    let path = fixture_path(repo_root, name);
    let bytes = fs::read(&path)
        .map_err(|error| format!("read fixture manifest {}: {error}", path.display()))?;
    let manifest: FixtureManifest = serde_json::from_slice(&bytes)
        .map_err(|error| format!("decode fixture manifest {}: {error}", path.display()))?;
    if manifest.schema != 1 {
        return Err(format!(
            "unsupported fixture schema {} in {}",
            manifest.schema,
            path.display()
        ));
    }
    if manifest.name != name {
        return Err(format!(
            "fixture manifest name mismatch: expected {name}, got {}",
            manifest.name
        ));
    }
    Ok(manifest)
}

fn fixture_nickname(peer: &Participant, index: usize) -> String {
    if matches!(peer, Participant::Android(_)) {
        "Soak Android".to_owned()
    } else {
        format!("Soak Bot {}", (b'A' + index as u8) as char)
    }
}

fn provision_fixture_profiles(peers: &mut [Participant]) -> Result<(), String> {
    let mut fake_index = 0usize;
    for peer in peers.iter_mut() {
        let index = if matches!(peer, Participant::Android(_)) {
            0
        } else {
            let index = fake_index;
            fake_index = fake_index.saturating_add(1);
            index
        };
        let display_name = fixture_nickname(peer, index);
        let response = retry_profile_set(peer, display_name.clone())?;
        if snapshot_identity(&response)
            .is_some_and(|identity| identity.display_name.as_deref() == Some(display_name.as_str()))
        {
            continue;
        }
        wait_for_profile(peer, &display_name)?;
    }
    Ok(())
}

fn rename_android_fixture_contacts(
    peers: &mut [Participant],
    fixture_nicknames: &[String],
) -> Result<(), String> {
    let Some(android_index) = peers.iter().position(|peer| matches!(peer, Participant::Android(_)))
    else {
        return Ok(());
    };
    let names = fixture_nicknames.iter().skip(1).filter(|name| !name.is_empty());
    let (before, _) = peers.split_at_mut(android_index + 1);
    let android = &mut before[android_index];
    let snapshot = snapshot_with_retry(android, "fixture-contact-names")?;
    let Some(contacts) =
        snapshot.pointer("/snapshot/contacts").and_then(serde_json::Value::as_array)
    else {
        return Ok(());
    };
    for (contact, name) in contacts.iter().zip(names) {
        let Some(contact_id) = contact.get("id").and_then(serde_json::Value::as_str) else {
            continue;
        };
        retry_operation(
            android,
            "contact.rename",
            serde_json::json!({"contactIdHex": contact_id, "displayName": name}),
            &format!("fixture-contact-rename-{contact_id}"),
        )?;
        wait_for_contact_name(android, contact_id, name)?;
    }
    Ok(())
}

fn wait_for_contact_name(
    peer: &mut Participant,
    contact_id: &str,
    expected: &str,
) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(90);
    let mut last_error = String::new();
    while Instant::now() < deadline && !tui::cancel_requested() {
        match snapshot_with_retry(peer, "fixture-contact-name-check") {
            Ok(snapshot) => {
                let matches = snapshot
                    .pointer("/snapshot/contacts")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(|contacts| {
                        contacts.iter().any(|contact| {
                            contact.get("id").and_then(serde_json::Value::as_str)
                                == Some(contact_id)
                                && contact.get("displayName").and_then(serde_json::Value::as_str)
                                    == Some(expected)
                        })
                    });
                if matches {
                    return Ok(());
                }
            }
            Err(error) => last_error = error,
        }
        tui::controlled_sleep(Duration::from_secs(2));
    }
    Err(format!(
        "{} contact {contact_id} did not become '{expected}' within 90s: {last_error}",
        peer.name()
    ))
}

/// Profile writes are local and idempotent. Use a fresh correlation id for
/// every attempt so a transient startup rejection is not replayed forever from
/// the native request cache under the original id.
fn retry_profile_set(
    peer: &mut Participant,
    display_name: String,
) -> Result<serde_json::Value, String> {
    let deadline = Instant::now() + Duration::from_secs(120);
    let mut attempt = 0u32;
    let mut last_error = String::new();
    while Instant::now() < deadline && !tui::cancel_requested() {
        attempt = attempt.saturating_add(1);
        let request_id = format!("profile-set-{}-{attempt}", peer.name());
        match peer.request(
            &request_id,
            "profile.set",
            serde_json::json!({"displayName": display_name}),
        ) {
            Ok(response) => return Ok(response),
            Err(error) => last_error = error,
        }
        tui::controlled_sleep(Duration::from_secs(2));
    }
    Err(format!("{} profile.set did not succeed within 120s: {last_error}", peer.name()))
}

fn wait_for_profile(peer: &mut Participant, expected: &str) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(90);
    let mut last_error = String::new();
    while Instant::now() < deadline && !tui::cancel_requested() {
        match snapshot_with_retry(peer, "profile-check") {
            Ok(snapshot) => {
                if snapshot_identity(&snapshot)
                    .is_some_and(|identity| identity.display_name.as_deref() == Some(expected))
                {
                    return Ok(());
                }
            }
            Err(error) => last_error = error,
        }
        tui::controlled_sleep(Duration::from_secs(2));
    }
    Err(format!("{} profile did not become '{expected}' within 90s: {last_error}", peer.name()))
}

fn wait_for_profile_setup(peer: &mut Participant) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(180);
    let mut attempt = 0u32;
    let mut last_phase = "unknown".to_owned();
    let mut last_error = String::new();
    while Instant::now() < deadline && !tui::cancel_requested() {
        attempt = attempt.saturating_add(1);
        match peer.request(&format!("profile-setup-{attempt}"), "snapshot", serde_json::json!({})) {
            Ok(snapshot) => {
                snapshot
                    .pointer("/snapshot/bootstrapPhase")
                    .or_else(|| snapshot.get("bootstrapPhase"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("unknown")
                    .clone_into(&mut last_phase);
                // Profile data is local durable state and does not require the
                // onion publication to reach READY. A cold Arti bootstrap can
                // legitimately keep onion publication in its 600s grace
                // window, so blocking here would make a deterministic fixture
                // impossible to provision. Identity presence proves that the
                // local storage/runtime bootstrap has completed enough for the
                // idempotent profile command; pairing remains network-gated.
                if snapshot_identity(&snapshot).is_some()
                    && !matches!(last_phase.as_str(), "failed" | "idle")
                {
                    return Ok(());
                }
                last_error.clear();
            }
            Err(error) => last_error = error,
        }
        tui::controlled_sleep(Duration::from_secs(2));
    }
    Err(format!(
        "{} did not expose local profile setup within 180s: phase={last_phase}, last_error={last_error}",
        peer.name()
    ))
}

fn snapshot_identity(response: &serde_json::Value) -> Option<FixtureIdentity> {
    let identity = response.pointer("/snapshot/identity").or_else(|| response.get("identity"))?;
    Some(FixtureIdentity {
        participant: String::new(),
        id: identity.get("id").and_then(serde_json::Value::as_str).map(str::to_owned),
        display_name: identity
            .get("displayName")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned),
        fingerprint: identity
            .get("fingerprint")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned),
    })
}

fn persist_fixture_manifest(
    cli: &Cli,
    peers: &mut [Participant],
    run_root: &Path,
    created_at_ms: u128,
    timeline: &mut File,
) -> Result<(), String> {
    let mut identities = Vec::with_capacity(peers.len());
    let mut nicknames = Vec::with_capacity(peers.len());
    let mut snapshots = Vec::with_capacity(peers.len());
    for peer in peers.iter_mut() {
        let snapshot = snapshot_with_retry(peer, "fixture-manifest")?;
        let mut identity = snapshot_identity(&snapshot)
            .ok_or_else(|| format!("{} fixture identity is missing", peer.name()))?;
        peer.name().clone_into(&mut identity.participant);
        nicknames.push(identity.display_name.clone().unwrap_or_default());
        identities.push(identity);
        snapshots.push(snapshot);
    }
    let android_index = peers.iter().position(|peer| matches!(peer, Participant::Android(_)));
    let expected_contacts = android_index.map_or(0, |index| contact_count(&snapshots[index]));
    let expected_conversations =
        android_index.map_or(0, |index| conversation_count(&snapshots[index]));
    let manifest = FixtureManifest {
        schema: 1,
        name: cli.fixture_name.clone(),
        scenario: format!("{:?}", cli.scenario),
        android_serial: cli.android.clone(),
        fake_peers: cli.fake_peers,
        expected_contacts,
        expected_conversations,
        nicknames,
        identities,
        created_at_ms,
    };
    let global_path = fixture_path(&cli.repo_root, &cli.fixture_name);
    if let Some(parent) = global_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create fixture directory {}: {error}", parent.display()))?;
    }
    write_json(&global_path, &manifest)?;
    write_json(&run_root.join("fixture.json"), &manifest)?;
    record(
        timeline,
        "fixture_provisioned",
        serde_json::json!({
            "name": cli.fixture_name,
            "path": global_path,
            "contacts": expected_contacts,
            "conversations": expected_conversations,
        }),
    )?;
    Ok(())
}

fn validate_reusable_fixture(
    peers: &mut [Participant],
    fixture: &FixtureManifest,
) -> Result<(), String> {
    let actual_fake_peers = peers
        .iter()
        .filter(|peer| matches!(peer, Participant::Fake(_) | Participant::Remote(_)))
        .count();
    if fixture.fake_peers != actual_fake_peers {
        return Err(format!(
            "fixture '{}' expects {} fake peers, but this run configured {}; use --fake-peers {} or provision a new fixture",
            fixture.name, fixture.fake_peers, actual_fake_peers, fixture.fake_peers
        ));
    }
    let android_name =
        peers.iter().find(|peer| matches!(peer, Participant::Android(_))).map(Participant::name);
    if fixture.android_serial.as_deref() != android_name {
        return Err(format!(
            "fixture '{}' is bound to Android {:?}, but selected device is {:?}; provision a fixture for this device",
            fixture.name, fixture.android_serial, android_name
        ));
    }
    for expected in &fixture.identities {
        let peer = peers
            .iter_mut()
            .find(|peer| peer.name() == expected.participant)
            .ok_or_else(|| format!("fixture peer '{}' is not present", expected.participant))?;
        let snapshot = snapshot_with_retry(peer, "fixture-validate")?;
        let actual = snapshot_identity(&snapshot)
            .ok_or_else(|| format!("{} fixture identity is missing", peer.name()))?;
        if expected.id.is_some() && actual.id != expected.id {
            return Err(format!("{} identity id changed; provision a new fixture", peer.name()));
        }
        if expected.display_name.is_some() && actual.display_name != expected.display_name {
            return Err(format!(
                "{} nickname changed from {:?} to {:?}; provision or repair the fixture",
                peer.name(),
                expected.display_name,
                actual.display_name
            ));
        }
    }
    if let Some(peer) = peers.iter_mut().find(|peer| matches!(peer, Participant::Android(_))) {
        let snapshot = snapshot_with_retry(peer, "fixture-contacts")?;
        let contacts = contact_count(&snapshot);
        let conversations = conversation_count(&snapshot);
        if contacts < fixture.expected_contacts || conversations < fixture.expected_conversations {
            return Err(format!(
                "fixture '{}' is incomplete: Android has {contacts} contacts/{conversations} conversations, expected at least {}/{}",
                fixture.name, fixture.expected_contacts, fixture.expected_conversations
            ));
        }
    }
    Ok(())
}

fn wait_for_message(
    peers: &mut [Participant],
    sender_index: usize,
    body: &str,
) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(120);
    let mut attempt = 0u32;
    let mut last_error = None;
    while Instant::now() < deadline && !tui::cancel_requested() {
        for (index, peer) in peers.iter_mut().enumerate() {
            if index == sender_index {
                continue;
            }
            attempt = attempt.saturating_add(1);
            match peer.request(
                &format!("message-check-{attempt}"),
                "snapshot",
                serde_json::json!({}),
            ) {
                Ok(snapshot) => {
                    last_error = None;
                    if snapshot_contains_message(&snapshot, body) {
                        return Ok(());
                    }
                }
                Err(error) => last_error = Some(format!("{}: {error}", peer.name())),
            }
        }
        tui::controlled_sleep(Duration::from_secs(2));
    }
    Err(format!(
        "message was not observed by a remote peer within 120s: {body}; last_transport_error={}",
        last_error.as_deref().unwrap_or("none")
    ))
}

fn snapshot_contains_message(snapshot: &serde_json::Value, body: &str) -> bool {
    // Root snapshots deliberately omit full message history. Conversation
    // summaries are the bounded projection that still proves the remote side
    // observed the latest body. Keep the old path for scripted/test gateways.
    snapshot.pointer("/snapshot/conversations").and_then(serde_json::Value::as_array).is_some_and(
        |conversations| {
            conversations.iter().any(|conversation| {
                conversation.get("lastMessageBody").and_then(serde_json::Value::as_str)
                    == Some(body)
            })
        },
    ) || snapshot.pointer("/snapshot/messages").and_then(serde_json::Value::as_array).is_some_and(
        |messages| {
            messages.iter().any(|message| {
                message.get("body").and_then(serde_json::Value::as_str) == Some(body)
            })
        },
    )
}

/// A radio command response only proves that the local coordinator accepted
/// the request. The soak must also observe the remote coordinator entering a
/// receiving/remote-floor state; otherwise a permanently queued floor request
/// would be reported as a successful radio burst.
fn wait_for_remote_radio(
    peers: &mut [Participant],
    sender_index: usize,
    contact_id: &str,
) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(45);
    let mut attempt = 0u32;
    let mut last_error = String::new();
    while Instant::now() < deadline && !tui::cancel_requested() {
        for (index, peer) in peers.iter_mut().enumerate() {
            if index == sender_index {
                continue;
            }
            attempt = attempt.saturating_add(1);
            match peer.request(&format!("radio-check-{attempt}"), "snapshot", serde_json::json!({}))
            {
                Ok(snapshot) => {
                    if snapshot_radio_is_remote_active(&snapshot, contact_id) {
                        return Ok(());
                    }
                    last_error = format!("{} radio not active yet", peer.name());
                }
                Err(error) => last_error = format!("{}: {error}", peer.name()),
            }
        }
        tui::controlled_sleep(Duration::from_millis(500));
    }
    Err(format!(
        "remote radio burst was not observed within 45s for contact {contact_id}; last_error={last_error}"
    ))
}

fn snapshot_radio_is_remote_active(snapshot: &serde_json::Value, contact_id: &str) -> bool {
    let radio = snapshot.pointer("/snapshot/radio").or_else(|| snapshot.get("radio"));
    let Some(radio) = radio else { return false };
    if radio.get("session").and_then(serde_json::Value::as_object).is_some_and(|session| {
        session.get("contactId").and_then(serde_json::Value::as_str) == Some(contact_id)
            && (session.get("floor").and_then(serde_json::Value::as_str) == Some("remote")
                || matches!(
                    session.get("state").and_then(serde_json::Value::as_str),
                    Some("receiving" | "starting_capture" | "transmitting")
                ))
    }) {
        return true;
    }
    radio.get("contacts").and_then(serde_json::Value::as_array).is_some_and(|contacts| {
        contacts.iter().any(|contact| {
            contact.get("contactId").and_then(serde_json::Value::as_str) == Some(contact_id)
                && matches!(
                    contact.get("state").and_then(serde_json::Value::as_str),
                    Some("receiving" | "starting_capture" | "transmitting")
                )
        })
    })
}

/// Attachment queue admission is intentionally fast. Completion is a
/// separate durable job state and must be asserted independently of the text
/// message which references the attachment.
fn wait_for_attachment(
    peers: &mut [Participant],
    sender_index: usize,
    name: &str,
) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(180);
    let mut attempt = 0u32;
    let mut last_error = String::new();
    while Instant::now() < deadline && !tui::cancel_requested() {
        for (index, peer) in peers.iter_mut().enumerate() {
            if index == sender_index {
                continue;
            }
            attempt = attempt.saturating_add(1);
            match peer.request(
                &format!("attachment-check-{attempt}"),
                "snapshot",
                serde_json::json!({}),
            ) {
                Ok(snapshot) => {
                    if snapshot_attachment_available(&snapshot, name) {
                        return Ok(());
                    }
                    last_error = format!("{} attachment not available yet", peer.name());
                }
                Err(error) => last_error = format!("{}: {error}", peer.name()),
            }
        }
        tui::controlled_sleep(Duration::from_secs(2));
    }
    Err(format!(
        "attachment {name} was not observed as available within 180s; last_error={last_error}"
    ))
}

fn snapshot_attachment_available(snapshot: &serde_json::Value, name: &str) -> bool {
    snapshot
        .pointer("/snapshot/attachments")
        .or_else(|| snapshot.get("attachments"))
        .and_then(serde_json::Value::as_array)
        .is_some_and(|attachments| {
            attachments.iter().any(|attachment| {
                attachment.get("name").and_then(serde_json::Value::as_str) == Some(name)
                    && attachment.get("status").and_then(serde_json::Value::as_str)
                        == Some("available")
            })
        })
}

fn spawn_peer(executable: &Path, root: &Path, name: &str) -> Result<PeerProcess, String> {
    let (child, input, output) = spawn_peer_parts(executable, root, name)?;
    Ok(PeerProcess {
        name: name.into(),
        executable: executable.to_owned(),
        root: root.to_owned(),
        child,
        input,
        output: BufReader::new(output),
    })
}

fn spawn_peer_parts(
    executable: &Path,
    root: &Path,
    name: &str,
) -> Result<(Child, ChildStdin, ChildStdout), String> {
    let mut child = Command::new(executable)
        .arg("--root")
        .arg(root)
        .arg("--control-stdio")
        // Keep fake peers deterministic and self-contained.  This is only
        // inherited by the SOAK lab process; real Android/desktop builds use
        // the production provider bind policy.
        .env("TORCA_IROH_LOCAL_ONLY", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("spawn {name}: {error}"))?;
    if let Some(stderr) = child.stderr.take() {
        let peer_name = name.to_owned();
        let stderr_path = root.join("scenario-stderr.log");
        thread::spawn(move || {
            let reader = BufReader::new(stderr);
            let mut artifact =
                std::fs::OpenOptions::new().create(true).append(true).open(&stderr_path).ok();
            for line in reader.lines().map_while(Result::ok) {
                if let Some(file) = artifact.as_mut() {
                    let _ = writeln!(file, "{line}");
                }
                tui::publish_backend_line(&format!("{peer_name}: {line}"), true);
            }
        });
    }
    let input = child.stdin.take().ok_or_else(|| format!("{name} stdin unavailable"))?;
    let output = child.stdout.take().ok_or_else(|| format!("{name} stdout unavailable"))?;
    Ok((child, input, output))
}

fn valid_endpoint(endpoint: &str) -> bool {
    let Some((host, port)) = endpoint.rsplit_once(':') else { return false };
    host.len() == 62 && host.ends_with(".onion") && port.parse::<u16>().is_ok()
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let data = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    fs::write(path, data).map_err(|error| format!("write {}: {error}", path.display()))
}

fn record(file: &mut File, event: &str, data: serde_json::Value) -> Result<(), String> {
    let line = serde_json::json!({"tsMs": SystemTime::now().duration_since(UNIX_EPOCH).map_err(|error| error.to_string())?.as_millis(), "event": event, "data": data});
    writeln!(file, "{line}").map_err(|error| format!("write timeline: {error}"))?;
    tui::publish_event(event, &line);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        contact_count, conversation_count, pairing_by_id, parse_launchable_activity,
        snapshot_attachment_available, snapshot_contains_message, snapshot_identity,
        snapshot_radio_is_remote_active, valid_endpoint, validate_fixture_name,
    };
    use serde_json::json;

    #[test]
    fn endpoint_validation_requires_v3_onion_and_port() {
        assert!(valid_endpoint(&format!("{}.onion:443", "a".repeat(56))));
        assert!(!valid_endpoint("invalid.onion:443"));
        assert!(!valid_endpoint("a.onion"));
    }

    #[test]
    fn fixture_names_are_safe_for_the_manifest_path() {
        assert!(validate_fixture_name("android-default").is_ok());
        assert!(validate_fixture_name("a_b2").is_ok());
        assert!(validate_fixture_name("../escape").is_err());
        assert!(validate_fixture_name("with space").is_err());
    }

    #[test]
    fn fixture_identity_reads_the_compact_and_native_shapes() {
        let compact =
            json!({"identity": {"id": "id", "displayName": "Alice", "fingerprint": "fp"}});
        let native = json!({"snapshot": {"identity": {"id": "id", "displayName": "Alice", "fingerprint": "fp"}}});
        assert_eq!(
            snapshot_identity(&compact).and_then(|value| value.display_name),
            Some("Alice".into())
        );
        assert_eq!(snapshot_identity(&native).and_then(|value| value.id), Some("id".into()));
    }

    #[test]
    fn launcher_parser_ignores_unrelated_resolution_lines() {
        let output = "priority=0\ncom.torca.torca_app/com.torca.MainActivity\n";
        assert_eq!(
            parse_launchable_activity("com.torca.torca_app", output).as_deref(),
            Some("com.torca.torca_app/com.torca.MainActivity")
        );
        assert!(parse_launchable_activity("com.torca.torca_app", "No activity found").is_none());
    }

    #[test]
    fn active_preflight_counts_contacts_and_conversations_independently() {
        let snapshot = json!({
            "snapshot": {
                "contacts": [{"id": "a"}, {"id": "b"}],
                "conversations": [{"id": "a"}]
            }
        });
        assert_eq!(contact_count(&snapshot), 2);
        assert_eq!(conversation_count(&snapshot), 1);
        assert_eq!(contact_count(&json!({})), 0);
        assert_eq!(conversation_count(&json!({})), 0);
    }

    #[test]
    fn pairing_lookup_uses_the_created_resource_instead_of_list_order() {
        let snapshot = json!({
            "snapshot": {
                "pairings": [
                    {"id": "current", "state": "created"},
                    {"id": "stale", "state": "accepted"}
                ]
            }
        });
        assert_eq!(
            pairing_by_id(&snapshot, "current").and_then(|pairing| pairing["state"].as_str()),
            Some("created")
        );
        assert!(pairing_by_id(&snapshot, "missing").is_none());
    }

    #[test]
    fn message_observation_uses_the_bounded_conversation_projection() {
        let snapshot = json!({
            "snapshot": {
                "messages": [],
                "conversations": [{"id": "conversation", "lastMessageBody": "hello"}]
            }
        });
        assert!(snapshot_contains_message(&snapshot, "hello"));
        assert!(!snapshot_contains_message(&snapshot, "missing"));
    }

    #[test]
    fn radio_observation_requires_remote_floor_or_receiving_state() {
        let active = json!({
            "snapshot": {
                "radio": {
                    "session": {"contactId": "peer", "state": "receiving", "floor": "remote"}
                }
            }
        });
        let queued = json!({
            "snapshot": {
                "radio": {
                    "session": {"contactId": "peer", "state": "requesting_floor", "floor": "none"}
                }
            }
        });
        assert!(snapshot_radio_is_remote_active(&active, "peer"));
        assert!(!snapshot_radio_is_remote_active(&queued, "peer"));
    }

    #[test]
    fn attachment_observation_requires_available_status_and_name() {
        let available = json!({
            "snapshot": {"attachments": [{"name": "clip.bin", "status": "available"}]}
        });
        let queued = json!({
            "snapshot": {"attachments": [{"name": "clip.bin", "status": "transferring"}]}
        });
        assert!(snapshot_attachment_available(&available, "clip.bin"));
        assert!(!snapshot_attachment_available(&queued, "clip.bin"));
    }
}
