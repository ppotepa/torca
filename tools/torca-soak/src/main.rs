//! Multi-process production-runtime soak orchestrator.
//!
//! The orchestrator deliberately talks to `torca-lab-peer` over JSONL instead
//! of linking a second copy of the runtime into this process.  Each peer is a
//! real process with an isolated profile, which exercises lifecycle, storage,
//! logging and Tor ownership boundaries.

use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use clap::{Parser, ValueEnum};
use serde::Serialize;

#[derive(Clone, Copy, Debug, ValueEnum)]
enum RelayMode {
    Managed,
    External,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum Workload {
    Moderate,
    Minimal,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum FaultProfile {
    Controlled,
    RelayOnly,
    None,
}

#[derive(Parser, Debug)]
#[command(name = "torca-soak")]
struct Cli {
    /// Android serial. Omit to run the fake-peer-only laboratory scenario.
    #[arg(long)]
    android: Option<String>,
    #[arg(long, default_value_t = 3)]
    fake_peers: usize,
    #[arg(long, default_value_t = 1800)]
    duration_seconds: u64,
    #[arg(long, value_enum, default_value_t = RelayMode::Managed)]
    relay: RelayMode,
    #[arg(long)]
    relay_endpoint: Option<String>,
    #[arg(long, value_enum, default_value_t = Workload::Moderate)]
    workload: Workload,
    #[arg(long, value_enum, default_value_t = FaultProfile::Controlled)]
    fault_profile: FaultProfile,
    #[arg(long, default_value = ".torca/soak")]
    output: PathBuf,
    /// Path to the already-built lab peer executable.
    /// Optional prebuilt lab peer. When omitted, use the platform-native
    /// binary produced by `cargo build -p torca-lab-peer`.
    #[arg(long)]
    lab_peer: Option<PathBuf>,
    #[arg(long, default_value = ".")]
    repo_root: PathBuf,
}

#[derive(Debug, Serialize)]
struct Manifest {
    run_id: String,
    seed: u64,
    fake_peers: usize,
    android_serial: Option<String>,
    duration_seconds: u64,
    workload: String,
    fault_profile: String,
    relay_mode: String,
    started_at_ms: u128,
}

#[derive(Debug, Serialize)]
struct Summary {
    run_id: String,
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

struct ManagedRelay {
    repo_root: PathBuf,
}

impl Drop for AndroidBridge {
    fn drop(&mut self) {
        let _ = Command::new("adb")
            .args(["-s", &self.serial, "forward", "--remove", &format!("tcp:{}", self.host_port)])
            .status();
    }
}

impl Drop for ManagedRelay {
    fn drop(&mut self) {
        let _ = Command::new("docker")
            .args([
                "compose",
                "-f",
                "infra/docker/compose.yml",
                "down",
                "--timeout",
                "30",
                "--remove-orphans",
            ])
            .current_dir(&self.repo_root)
            .status();
    }
}

impl ManagedRelay {
    fn pause(&self) -> Result<(), String> {
        let status = Command::new("docker")
            .args(["compose", "-f", "infra/docker/compose.yml", "stop", "relay"])
            .current_dir(&self.repo_root)
            .status()
            .map_err(|error| format!("pause managed relay: {error}"))?;
        status.success().then_some(()).ok_or_else(|| "managed relay pause failed".into())
    }

    fn resume(&self) -> Result<(), String> {
        let status = Command::new("docker")
            .args(["compose", "-f", "infra/docker/compose.yml", "start", "relay"])
            .current_dir(&self.repo_root)
            .status()
            .map_err(|error| format!("resume managed relay: {error}"))?;
        status.success().then_some(()).ok_or_else(|| "managed relay resume failed".into())
    }
}

enum Participant {
    Fake(PeerProcess),
    Android(AndroidBridge),
}

impl Participant {
    fn name(&self) -> &str {
        match self {
            Self::Fake(peer) => &peer.name,
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
            Self::Android(_) => {
                Err("Android process restart is controlled by adb separately".into())
            }
        }
    }

    fn set_network(&self, enabled: bool) -> Result<(), String> {
        match self {
            Self::Android(android) => android.set_wifi(enabled),
            Self::Fake(_) => Err("network fault injection is only supported for Android".into()),
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
    let cli = Cli::parse();
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
    let manifest = Manifest {
        run_id: run_id.clone(),
        seed: started.as_nanos() as u64,
        fake_peers: cli.fake_peers,
        android_serial: cli.android.clone(),
        duration_seconds: cli.duration_seconds,
        workload: format!("{:?}", cli.workload),
        fault_profile: format!("{:?}", cli.fault_profile),
        relay_mode: format!("{:?}", cli.relay),
        started_at_ms: started.as_millis(),
    };
    write_json(&root.join("manifest.json"), &manifest)?;
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

    let peer_executable = if managed_relay.is_some() {
        build_lab_peer(&cli.repo_root, endpoint.as_deref().unwrap())?
    } else {
        cli.lab_peer.clone().unwrap_or_else(default_lab_peer_path)
    };

    let mut peers: Vec<Participant> = Vec::new();
    if let Some(serial) = &cli.android {
        let android = AndroidBridge::connect(serial)?;
        record(&mut timeline, "android_ready", serde_json::json!({"serial": serial}))?;
        peers.push(Participant::Android(android));
    }
    for index in 0..cli.fake_peers {
        let name = format!("peer-{}", (b'a' + index as u8) as char);
        let peer_root = root.join(&name);
        fs::create_dir_all(&peer_root).map_err(|error| format!("create {name} root: {error}"))?;
        peers.push(Participant::Fake(spawn_peer(&peer_executable, &peer_root, &name)?));
    }

    for peer in &mut peers {
        let response = peer.request("readiness", "diagnostics", serde_json::json!({}))?;
        record(
            &mut timeline,
            "peer_ready",
            serde_json::json!({"peer": peer.name(), "response": response}),
        )?;
    }

    pair_mesh(&mut peers, &mut timeline)?;

    let deadline = Instant::now() + Duration::from_secs(cli.duration_seconds);
    let run_started = Instant::now();
    let mut fault_injected = false;
    let mut android_network_fault_injected = false;
    let mut peer_restart_injected = false;
    let mut sequence = 0u64;
    while Instant::now() < deadline {
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
                std::thread::sleep(Duration::from_secs(15));
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
                std::thread::sleep(Duration::from_secs(10));
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
                if sequence.is_multiple_of(6) {
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
                if sequence.is_multiple_of(12) {
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
                    std::thread::sleep(Duration::from_millis(750));
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
        std::thread::sleep(Duration::from_secs(match cli.workload {
            Workload::Moderate => 10,
            Workload::Minimal => 2,
        }));
    }

    for peer in &mut peers {
        peer.stop();
    }
    record(&mut timeline, "run_completed", serde_json::json!({"sequence": sequence}))?;
    let completed =
        SystemTime::now().duration_since(UNIX_EPOCH).map_err(|error| error.to_string())?;
    write_json(
        &root.join("summary.json"),
        &Summary {
            run_id,
            status: "completed",
            sequence,
            participants: peers.len(),
            completed_at_ms: completed.as_millis(),
        },
    )?;
    drop(managed_relay);
    Ok(())
}

fn start_managed_relay(repo_root: &Path) -> Result<(String, ManagedRelay), String> {
    let stack_root = repo_root.join(".torca/stack");
    let _ = fs::remove_file(stack_root.join("relay_ready.txt"));
    let status = Command::new("docker")
        .args(["compose", "-f", "infra/docker/compose.yml", "up", "-d", "--build", "relay"])
        .current_dir(repo_root)
        .status()
        .map_err(|error| format!("start managed relay: {error}"))?;
    if !status.success() {
        return Err("managed relay compose start failed".into());
    }
    let endpoint_file = repo_root.join(".torca/stack/relay_endpoint.txt");
    let ready_file = repo_root.join(".torca/stack/relay_ready.txt");
    let deadline = Instant::now() + Duration::from_secs(180);
    while Instant::now() < deadline {
        if let Ok(endpoint) = fs::read_to_string(&endpoint_file) {
            let endpoint = endpoint.trim().to_owned();
            let ready = fs::read_to_string(&ready_file)
                .ok()
                .is_some_and(|value| value.lines().any(|line| line.trim() == endpoint));
            if valid_endpoint(&endpoint) && ready {
                return Ok((endpoint, ManagedRelay { repo_root: repo_root.to_owned() }));
            }
        }
        std::thread::sleep(Duration::from_secs(2));
    }
    Err(format!(
        "managed relay did not publish a valid endpoint within 180s: {}",
        endpoint_file.display()
    ))
}

fn build_lab_peer(repo_root: &Path, endpoint: &str) -> Result<PathBuf, String> {
    let status = Command::new("cargo")
        .args(["build", "-p", "torca-lab-peer"])
        .env("TORCA_RELAY_ENDPOINT", endpoint)
        .current_dir(repo_root)
        .status()
        .map_err(|error| format!("build lab peer: {error}"))?;
    if !status.success() {
        return Err("lab peer build failed".into());
    }
    Ok(repo_root.join(if cfg!(windows) {
        "target/debug/torca-lab-peer.exe"
    } else {
        "target/debug/torca-lab-peer"
    }))
}

fn default_lab_peer_path() -> PathBuf {
    PathBuf::from(if cfg!(windows) {
        "target/debug/torca-lab-peer.exe"
    } else {
        "target/debug/torca-lab-peer"
    })
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

fn retry_operation(
    peer: &mut Participant,
    operation: &str,
    payload: serde_json::Value,
    request_prefix: &str,
) -> Result<serde_json::Value, String> {
    let deadline = Instant::now() + Duration::from_secs(120);
    let mut attempt = 0u32;
    let mut last_error = String::new();
    while Instant::now() < deadline {
        attempt = attempt.saturating_add(1);
        match peer.request(&format!("{request_prefix}-{attempt}"), operation, payload.clone()) {
            Ok(response) => return Ok(response),
            Err(error) => last_error = error,
        }
        std::thread::sleep(Duration::from_secs(2));
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
        std::thread::sleep(Duration::from_secs(2));
        latest = peer.request("pair-create-wait", "snapshot", serde_json::json!({}))?;
    }
}

fn wait_for_conversation(left: &mut Participant, right: &mut Participant) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(90);
    while Instant::now() < deadline {
        let left_snapshot = snapshot_with_retry(left, "pair-check-left")?;
        let right_snapshot = snapshot_with_retry(right, "pair-check-right")?;
        if first_conversation_id(&left_snapshot).is_some()
            && first_conversation_id(&right_snapshot).is_some()
        {
            return Ok(());
        }
        std::thread::sleep(Duration::from_secs(2));
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
    while Instant::now() < deadline {
        attempt = attempt.saturating_add(1);
        match peer.request(
            &format!("{request_prefix}-{attempt}"),
            "snapshot",
            serde_json::json!({}),
        ) {
            Ok(snapshot) => return Ok(snapshot),
            Err(error) => last_error = error,
        }
        std::thread::sleep(Duration::from_secs(2));
    }
    Err(format!("{} snapshot did not succeed within 60s: {last_error}", peer.name()))
}

impl AndroidBridge {
    fn connect(serial: &str) -> Result<Self, String> {
        let status = Command::new("adb")
            .args(["-s", serial, "shell", "am", "start", "-n", "com.torca.torca_app/.MainActivity"])
            .status()
            .map_err(|error| format!("start Android activity: {error}"))?;
        if !status.success() {
            return Err(format!("Android activity failed to start on {serial}"));
        }
        let deadline = Instant::now() + Duration::from_secs(120);
        let discovery = loop {
            let output = Command::new("adb")
                .args([
                    "-s",
                    serial,
                    "exec-out",
                    "run-as",
                    "com.torca.torca_app",
                    "cat",
                    "cache/torca-scenario.json",
                ])
                .output()
                .map_err(|error| format!("read Android scenario discovery: {error}"))?;
            if output.status.success() {
                if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
                    break value;
                }
            }
            if Instant::now() >= deadline {
                return Err(format!("Android scenario bridge did not start on {serial}"));
            }
            std::thread::sleep(Duration::from_secs(1));
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
    while Instant::now() < deadline {
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
        std::thread::sleep(Duration::from_secs(2));
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
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|error| format!("spawn {name}: {error}"))?;
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
    writeln!(file, "{line}").map_err(|error| format!("write timeline: {error}"))
}

#[cfg(test)]
mod tests {
    use super::valid_endpoint;

    #[test]
    fn endpoint_validation_requires_v3_onion_and_port() {
        assert!(valid_endpoint(&format!("{}.onion:443", "a".repeat(56))));
        assert!(!valid_endpoint("invalid.onion:443"));
        assert!(!valid_endpoint("a.onion"));
    }
}
