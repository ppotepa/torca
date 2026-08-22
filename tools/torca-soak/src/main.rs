// Multi-process production-runtime soak orchestrator.
//
// The orchestrator deliberately talks to `torca-lab-peer` over JSONL instead
// of linking a second copy of the runtime into this process. Each peer is a
// real process with an isolated profile, which exercises lifecycle, storage,
// logging and Tor ownership boundaries.

use std::fs::{self, File};
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

mod tui;
mod wizard;

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
    /// Install/restart the current debug Android client before the run. Active
    /// Messaging enables this automatically and starts from a clean profile.
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
    started_at_ms: u128,
}

#[derive(Debug, Serialize)]
struct Summary {
    run_id: String,
    scenario: String,
    status: &'static str,
    sequence: u64,
    participants: usize,
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
}

struct ActiveBatteryCapture {
    serial: String,
    root: PathBuf,
}

fn android_package() -> &'static str {
    torca_deploy::android_target::package()
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
    // Keep the old binary/PowerShell documentation usable while all new
    // scenarios go through the same cockpit and typed CLI.
    if cli.android.is_none() {
        cli.android = cli.legacy_device_id.take();
    }
    if let Some(minutes) = cli.legacy_duration_minutes.take() {
        cli.scenario = Scenario::IdleBattery;
        cli.duration_seconds = minutes.saturating_mul(60);
    }
    if cli.scenario == Scenario::ActiveMessaging && cli.android.is_some() {
        cli.android_auto_deploy = true;
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
    if cli.fake_peers < 1 {
        return Err("fake-peers must be at least 1".into());
    }
    if cli.duration_seconds == 0 {
        return Err("duration-seconds must be positive".into());
    }
    if matches!(cli.relay, RelayMode::External) && cli.relay_endpoint.is_none() {
        return Err("--relay-endpoint is required with --relay external".into());
    }
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
    let endpoint = match cli.relay {
        RelayMode::Managed => {
            tui::publish_event("relay_starting", &serde_json::json!({"mode": "managed"}));
            let (endpoint, guard) = start_managed_relay(&cli.repo_root)?;
            managed_relay = Some(guard);
            Some(endpoint)
        }
        RelayMode::External => {
            managed_relay = None;
            cli.relay_endpoint.clone().or_else(|| std::env::var("TORCA_RELAY_ENDPOINT").ok())
        }
    };
    if endpoint.as_deref().is_none_or(|value| !valid_endpoint(value)) {
        return Err("a valid relay endpoint is required; start the managed relay or pass --relay-endpoint host.onion:port".into());
    }
    record(
        &mut timeline,
        "relay_ready",
        serde_json::json!({
            "endpoint": endpoint.as_deref().unwrap_or_default(),
            "health": "ready",
            "onion": "published"
        }),
    )?;

    let peer_executable = if cli.bot_host.is_none() && managed_relay.is_some() {
        build_lab_peer(&cli.repo_root, endpoint.as_deref().unwrap())?
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
            loop {
                match ensure_android_deployed(&cli.repo_root, serial, !cli.preserve_profiles) {
                    Ok(()) => break,
                    Err(error) if wait_for_android_preflight_retry(serial, &error) => {
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
            if cli.scenario == Scenario::ActiveMessaging
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
        pair_android_star(&mut peers, &mut timeline)?;
        validate_active_messaging_preflight(
            &mut peers,
            initial_android_contacts.saturating_add(cli.fake_peers),
            initial_android_conversations.saturating_add(cli.fake_peers),
            cli.fake_peers,
            &mut timeline,
        )?;
    } else {
        pair_mesh(&mut peers, &mut timeline)?;
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
        for peer_index in 0..peers.len() {
            sequence = sequence.saturating_add(1);
            let body;
            {
                let peer = &mut peers[peer_index];
                let snapshot = snapshot_with_retry(peer, "snapshot")?;
                let conversation_id = first_conversation_id(&snapshot)
                    .ok_or_else(|| format!("{} has no conversation after pairing", peer.name()))?;
                let message_body = format!("torca-soak sequence={sequence} sender={}", peer.name());
                body = message_body.clone();
                let response = peer.request(
                    &format!("message-{sequence}"),
                    "message.send",
                    serde_json::json!({
                        "conversationIdHex": conversation_id,
                        "body": message_body
                    }),
                )?;
                record(
                    &mut timeline,
                    "message_queued",
                    serde_json::json!({"peer": peer.name(), "sequence": sequence, "response": response}),
                )?;
                if sequence.is_multiple_of(30) {
                    let fixture = peer.request(
                        &format!("fixture-{sequence}"),
                        "attachment.fixture",
                        serde_json::json!({"size": 1_048_576}),
                    )?;
                    let fixture_path = fixture
                        .pointer("/result/path")
                        .or_else(|| fixture.get("path"))
                        .and_then(serde_json::Value::as_str)
                        .ok_or_else(|| format!("{} fixture path missing", peer.name()))?;
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
                    record(
                        &mut timeline,
                        "attachment_queued",
                        serde_json::json!({"peer": peer.name(), "sequence": sequence, "response": attachment}),
                    )?;
                }
                if cli.radio && sequence.is_multiple_of(12) {
                    let contact_id = first_contact_id(&snapshot)
                        .ok_or_else(|| format!("{} has no contact for radio", peer.name()))?;
                    peer.request(
                        &format!("radio-enable-{sequence}"),
                        "radio.enable",
                        serde_json::json!({"contactIdHex": contact_id, "enabled": true}),
                    )?;
                    let begin = peer.request(
                        &format!("radio-begin-{sequence}"),
                        "radio.begin",
                        serde_json::json!({"contactIdHex": contact_id}),
                    )?;
                    tui::controlled_sleep(Duration::from_millis(750));
                    let end = peer.request(
                        &format!("radio-end-{sequence}"),
                        "radio.end",
                        serde_json::json!({"contactIdHex": contact_id}),
                    )?;
                    record(
                        &mut timeline,
                        "radio_burst",
                        serde_json::json!({"peer": peer.name(), "sequence": sequence, "begin": begin, "end": end}),
                    )?;
                }
            }
            wait_for_message(&mut peers, peer_index, &body)?;
        }
        tui::controlled_sleep(Duration::from_secs(match cli.workload {
            Workload::Balanced => 120,
            Workload::Moderate => 10,
            Workload::Minimal => 2,
        }));
    }

    for peer in &mut peers {
        peer.stop();
    }
    let cancelled = tui::cancel_requested();
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
            completed_at_ms: completed.as_millis(),
        },
    )?;
    drop(managed_relay);
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
                "Approve installation on Android, then press r to retry"
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

fn start_managed_relay(repo_root: &Path) -> Result<(String, ManagedRelay), String> {
    let stack_root = repo_root.join(".torca/stack");
    let _ = fs::remove_file(stack_root.join("relay_ready.txt"));
    let mut command = Command::new("docker");
    command
        .args(["compose", "-f", "infra/docker/compose.yml", "up", "-d", "--build", "relay"])
        .current_dir(repo_root);
    let result = if tui::is_active() {
        run_external_command(&mut command, "relay build/start")?
    } else {
        let output = command.output().map_err(|error| format!("relay build/start: {error}"))?;
        if !output.status.success() {
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
                return Ok((endpoint, ManagedRelay { repo_root: repo_root.to_owned() }));
            }
        }
        tui::controlled_sleep(Duration::from_secs(2));
    }
    Err(format!(
        "managed relay did not publish a valid endpoint within 180s: {}",
        endpoint_file.display()
    ))
}

fn command_output_tail(stdout: &[u8], stderr: &[u8]) -> String {
    let mut lines = String::from_utf8_lossy(stdout)
        .lines()
        .chain(String::from_utf8_lossy(stderr).lines())
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    const MAX_LINES: usize = 20;
    if lines.len() > MAX_LINES {
        lines.drain(..lines.len() - MAX_LINES);
    }
    if lines.is_empty() { String::new() } else { lines.join(" | ") }
}

fn build_lab_peer(repo_root: &Path, endpoint: &str) -> Result<PathBuf, String> {
    let mut command = Command::new("cargo");
    command
        .args(["build", "-p", "torca-lab-peer", "--locked"])
        .env("TORCA_RELAY_ENDPOINT", endpoint)
        .current_dir(repo_root);
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
    let power = adb_output(serial, &["shell", "dumpsys", "power"])?;
    if require_screen_off && !power.contains("mWakefulness=Asleep") {
        return Err(format!("active battery soak requires screen-off Android device '{serial}'"));
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
            let invitation =
                left_peer.request("pair-create", "pairing.create", serde_json::json!({}))?;
            let pairing = wait_for_pairing_invitation(left_peer, invitation)?;
            let code = pairing
                .get("code")
                .and_then(serde_json::Value::as_str)
                .ok_or("pairing code missing")?;
            let session_id = pairing
                .get("id")
                .and_then(serde_json::Value::as_str)
                .ok_or("pairing session id missing")?;
            retry_operation(
                right_peer,
                "pairing.join",
                serde_json::json!({"code": code}),
                "pair-join",
            )?;
            retry_operation(
                left_peer,
                "pairing.approve",
                serde_json::json!({"sessionIdHex": session_id}),
                "pair-approve",
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
/// exactly `fake_peers` contacts and prevents bot-to-bot traffic from diluting
/// an active Android battery scenario.
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
    let invitation = inviter.request("pair-create", "pairing.create", serde_json::json!({}))?;
    let pairing = wait_for_pairing_invitation(inviter, invitation)?;
    let code =
        pairing.get("code").and_then(serde_json::Value::as_str).ok_or("pairing code missing")?;
    let session_id = pairing
        .get("id")
        .and_then(serde_json::Value::as_str)
        .ok_or("pairing session id missing")?;
    retry_operation(joiner, "pairing.join", serde_json::json!({"code": code}), "pair-join")?;
    retry_operation(
        inviter,
        "pairing.approve",
        serde_json::json!({"sessionIdHex": session_id}),
        "pair-approve",
    )?;
    wait_for_conversation(inviter, joiner)?;
    record(
        timeline,
        "pairing_completed",
        serde_json::json!({"left": inviter.name(), "right": joiner.name(), "topology": "android-star"}),
    )
}

fn retry_operation(
    peer: &mut Participant,
    operation: &str,
    payload: serde_json::Value,
    request_prefix: &str,
) -> Result<serde_json::Value, String> {
    let deadline = Instant::now() + Duration::from_secs(120);
    let mut attempt = 0u32;
    let mut last_error = String::new();
    while Instant::now() < deadline && !tui::cancel_requested() {
        attempt = attempt.saturating_add(1);
        match peer.request(&format!("{request_prefix}-{attempt}"), operation, payload.clone()) {
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
    let deadline = Instant::now() + Duration::from_secs(180);
    let mut latest = initial;
    loop {
        if let Some(pairing) = latest
            .pointer("/snapshot/pairings")
            .and_then(serde_json::Value::as_array)
            .and_then(|items| items.last())
        {
            return Ok(pairing.clone());
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "{} did not publish a pairing invitation within 180s; response={latest}",
                peer.name()
            ));
        }
        tui::controlled_sleep(Duration::from_secs(2));
        latest = peer.request("pair-create-wait", "snapshot", serde_json::json!({}))?;
    }
}

fn wait_for_conversation(left: &mut Participant, right: &mut Participant) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(90);
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
    Err(format!("pairing did not create conversations for {} and {}", left.name(), right.name()))
}

fn snapshot_with_retry(
    peer: &mut Participant,
    request_prefix: &str,
) -> Result<serde_json::Value, String> {
    let deadline = Instant::now() + Duration::from_secs(60);
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
    Err(format!("{} snapshot did not succeed within 60s: {last_error}", peer.name()))
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
                "Run .\\scripts\\soak.ps1 cockpit --android <adb-serial> to build/install the SOAK2 flavor."
            } else {
                "Run the Android deploy first, or use .\\scripts\\soak.ps1 cockpit for an Android SOAK2 run."
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
        let discovery_paths = ["cache/torca-scenario.json", "cache/torca/torca-scenario.json"];
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
                    " The SOAK2 package is installed, but its debug ScenarioBridge did not publish discovery; verify the app is foregrounded and built with TORCA_SOAK_MODE=true."
                } else {
                    " This is the normal package. SOAK2 requires the .soak flavor; run .\\scripts\\soak.ps1 cockpit instead of cargo run directly."
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

fn ensure_android_deployed(repo_root: &Path, serial: &str, clean: bool) -> Result<(), String> {
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
    run_typed_android_deploy(repo_root, serial, clean)?;
    android_launchable_activity(serial).map(|_| ()).map_err(|error| {
        format!(
            "Android auto-deploy completed but the client is still not launchable on '{serial}': {error}"
        )
    })
}

fn run_typed_android_deploy(repo_root: &Path, serial: &str, clean: bool) -> Result<(), String> {
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
    build_plan.relay_build = torca_deploy::domain::BuildPolicy::Reuse;
    build_plan.validation = torca_deploy::domain::ValidationLevel::Skip;
    build_plan.launch = torca_deploy::domain::LaunchPolicy::Skip;
    let build_run = executor
        .create_run(build_plan)
        .map_err(|error| format!("create typed Android build plan: {error}"))?;
    executor
        .execute(build_run, torca_deploy::ExecutionMode::Execute)
        .map_err(|error| format!("typed Android artifact build failed: {error}"))?;

    let mut install_plan = torca_deploy::domain::DeployPlan::normal(
        torca_deploy::domain::DeployAction::RedeployCurrent,
        vec![torca_deploy::domain::Target::Android],
        torca_deploy::domain::Configuration::Debug,
    );
    install_plan.device = Some(serial.to_owned());
    install_plan.client_build = torca_deploy::domain::BuildPolicy::Reuse;
    install_plan.relay_build = torca_deploy::domain::BuildPolicy::Reuse;
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
    response
        .pointer("/snapshot/conversations")
        .and_then(serde_json::Value::as_array)
        .and_then(|items| items.first())
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

fn wait_for_message(
    peers: &mut [Participant],
    sender_index: usize,
    body: &str,
) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline && !tui::cancel_requested() {
        for (index, peer) in peers.iter_mut().enumerate() {
            if index == sender_index {
                continue;
            }
            let snapshot = peer.request("message-check", "snapshot", serde_json::json!({}))?;
            let found = snapshot
                .pointer("/snapshot/messages")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|messages| {
                    messages.iter().any(|message| {
                        message.get("body").and_then(serde_json::Value::as_str) == Some(body)
                    })
                });
            if found {
                return Ok(());
            }
        }
        tui::controlled_sleep(Duration::from_secs(2));
    }
    Err(format!("message was not observed by a remote peer: {body}"))
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
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("spawn {name}: {error}"))?;
    if let Some(stderr) = child.stderr.take() {
        let peer_name = name.to_owned();
        thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines().map_while(Result::ok) {
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
    use super::{contact_count, conversation_count, parse_launchable_activity, valid_endpoint};
    use serde_json::json;

    #[test]
    fn endpoint_validation_requires_v3_onion_and_port() {
        assert!(valid_endpoint(&format!("{}.onion:443", "a".repeat(56))));
        assert!(!valid_endpoint("invalid.onion:443"));
        assert!(!valid_endpoint("a.onion"));
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
}
