//! Process-isolated production-runtime peer for BATTERY1 laboratory runs.
//!
//! Each invocation owns one native runtime.  A scenario orchestrator can run
//! two copies with different roots; their identities, databases and Tor caches
//! remain isolated exactly as on two physical clients.

use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

use clap::{Parser, ValueEnum};

#[derive(Clone, Copy, Debug, ValueEnum)]
enum Operation {
    Observe,
    Create,
    Join,
    Approve,
}

#[derive(Parser, Debug)]
#[command(name = "torca-lab-peer")]
struct Cli {
    /// Dedicated storage root. It must not point at a normal Torca profile.
    #[arg(long)]
    root: PathBuf,
    /// How long the real native runtime remains available for orchestration.
    #[arg(long, default_value_t = 30)]
    duration_seconds: u64,
    /// Bounded time to wait for the production runtime bootstrap before an
    /// operation is sent. This prevents a lab scenario from mistaking startup
    /// races for pairing failures.
    #[arg(long, default_value_t = 120)]
    startup_timeout_seconds: u64,
    /// Contract operation to execute before observing the real runtime.
    #[arg(long, value_enum, default_value_t = Operation::Observe)]
    operation: Operation,
    /// Invitation code required by `join`.
    #[arg(long)]
    code: Option<String>,
    /// Pairing session id required by `approve`.
    #[arg(long)]
    session_id: Option<String>,
    /// Keep the process alive and accept newline-delimited JSON commands.
    #[arg(long)]
    control_stdio: bool,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("torca-lab-peer: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let cli = Cli::parse();
    if cli.duration_seconds == 0 {
        return Err("duration-seconds must be positive".into());
    }
    validate_lab_root(&cli.root)?;
    let mut runtime = torca_native::NativeRuntimeClient::acquire_at(&cli.root)?;
    {
        wait_until_ready(&mut runtime, cli.startup_timeout_seconds)?;
        execute_operation(&mut runtime, &cli)?;
        if cli.control_stdio {
            return control_loop(&mut runtime, &cli.root);
        }
        let started = Instant::now();
        let deadline = started + Duration::from_secs(cli.duration_seconds);
        while Instant::now() < deadline {
            let response = invoke(&mut runtime, "lab-diagnostics")?;
            let status =
                response.get("status").and_then(serde_json::Value::as_str).unwrap_or("unknown");
            if status != "succeeded" {
                let code = response
                    .pointer("/error/code")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("UNKNOWN");
                return Err(format!("native runtime is not ready ({code})"));
            }
            let revision =
                response.get("revision").and_then(serde_json::Value::as_u64).unwrap_or(0);
            println!("lab-peer root={} status={status} revision={revision}", cli.root.display());
            thread::sleep(Duration::from_secs(1));
        }
        Ok(())
    }
}

fn control_loop(
    runtime: &mut torca_native::NativeRuntimeClient,
    root: &std::path::Path,
) -> Result<(), String> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for line in stdin.lock().lines() {
        let line = line.map_err(|error| format!("read control request failed: {error}"))?;
        if line.trim().is_empty() {
            continue;
        }
        let request: serde_json::Value = serde_json::from_str(&line)
            .map_err(|error| format!("decode control request failed: {error}"))?;
        let id = request.get("id").cloned().unwrap_or_else(|| serde_json::json!(null));
        let result = control_request(runtime, root, &request);
        let response = match result {
            Ok(value) => serde_json::json!({"id": id, "status": "succeeded", "result": value}),
            Err(error) => serde_json::json!({"id": id, "status": "failed", "error": error}),
        };
        writeln!(stdout, "{response}")
            .map_err(|error| format!("write control response failed: {error}"))?;
        stdout.flush().map_err(|error| format!("flush control response failed: {error}"))?;
        if request.get("op").and_then(serde_json::Value::as_str) == Some("shutdown") {
            break;
        }
    }
    Ok(())
}

fn control_request(
    runtime: &mut torca_native::NativeRuntimeClient,
    root: &std::path::Path,
    request: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let operation = request.get("op").and_then(serde_json::Value::as_str).unwrap_or_default();
    if operation == "shutdown" {
        return Ok(serde_json::json!({"stopping": true}));
    }
    if operation == "attachment.fixture" {
        let size =
            request.get("size").and_then(serde_json::Value::as_u64).ok_or("size is required")?;
        if !(1..=5 * 1024 * 1024).contains(&size) {
            return Err("fixture size must be between 1 byte and 5 MiB".into());
        }
        let path = root.join("scenario-fixture.bin");
        let mut file =
            std::fs::File::create(&path).map_err(|error| format!("create fixture: {error}"))?;
        let block = [0x54_u8; 4096];
        let mut remaining = size;
        while remaining > 0 {
            let count = remaining.min(block.len() as u64) as usize;
            std::io::Write::write_all(&mut file, &block[..count])
                .map_err(|error| format!("write fixture: {error}"))?;
            remaining -= count as u64;
        }
        return Ok(serde_json::json!({"path": path, "size": size}));
    }
    let (kind, name, payload) = match operation {
        "snapshot" => ("query", "snapshot.get", serde_json::json!({})),
        "diagnostics" => ("query", "diagnostics.get", serde_json::json!({})),
        "pairing.create" => ("command", "pairing.create", serde_json::json!({})),
        "pairing.join" => (
            "command",
            "pairing.join",
            serde_json::json!({
                "code": request.get("code").and_then(serde_json::Value::as_str).ok_or("code is required")?
            }),
        ),
        "pairing.approve" | "pairing.reject" | "pairing.cancel" => (
            "command",
            operation,
            serde_json::json!({
                "sessionIdHex": request.get("sessionIdHex").and_then(serde_json::Value::as_str).ok_or("sessionIdHex is required")?
            }),
        ),
        "message.send" => (
            "command",
            "message.send",
            serde_json::json!({
                "conversationIdHex": request.get("conversationIdHex").and_then(serde_json::Value::as_str).ok_or("conversationIdHex is required")?,
                "body": request.get("body").and_then(serde_json::Value::as_str).ok_or("body is required")?
            }),
        ),
        "attachment.queue" => (
            "command",
            "attachment.queue",
            serde_json::json!({
                "conversationIdHex": request.get("conversationIdHex").and_then(serde_json::Value::as_str).ok_or("conversationIdHex is required")?,
                "sourcePath": request.get("sourcePath").and_then(serde_json::Value::as_str).ok_or("sourcePath is required")?,
                "name": request.get("name").and_then(serde_json::Value::as_str).ok_or("name is required")?,
                "mediaType": request.get("mediaType").and_then(serde_json::Value::as_str).unwrap_or("application/octet-stream"),
                "size": request.get("size").and_then(serde_json::Value::as_u64).ok_or("size is required")?
            }),
        ),
        "radio.enable" => (
            "command",
            "radio.set_enabled",
            serde_json::json!({
                "contactIdHex": request.get("contactIdHex").and_then(serde_json::Value::as_str).ok_or("contactIdHex is required")?,
                "enabled": request.get("enabled").and_then(serde_json::Value::as_bool).unwrap_or(true)
            }),
        ),
        "radio.begin" => (
            "command",
            "radio.begin_transmission",
            serde_json::json!({
                "contactIdHex": request.get("contactIdHex").and_then(serde_json::Value::as_str).ok_or("contactIdHex is required")?
            }),
        ),
        "radio.end" => (
            "command",
            "radio.end_transmission",
            serde_json::json!({
                "contactIdHex": request.get("contactIdHex").and_then(serde_json::Value::as_str).ok_or("contactIdHex is required")?
            }),
        ),
        _ => return Err(format!("unsupported control operation: {operation}")),
    };
    let response = invoke_request(runtime, "lab-control", kind, name, payload)?;
    let status = response.get("status").and_then(serde_json::Value::as_str).unwrap_or("unknown");
    if status == "failed" {
        return Err(response
            .pointer("/error/code")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("runtime operation failed")
            .to_owned());
    }
    Ok(response.get("result").cloned().unwrap_or(response))
}

fn wait_until_ready(
    runtime: &mut torca_native::NativeRuntimeClient,
    timeout_seconds: u64,
) -> Result<(), String> {
    if timeout_seconds == 0 {
        return Err("startup-timeout-seconds must be positive".into());
    }
    let deadline = Instant::now() + Duration::from_secs(timeout_seconds);
    loop {
        let response = invoke(runtime, "lab-readiness")?;
        let status =
            response.get("status").and_then(serde_json::Value::as_str).unwrap_or("unknown");
        if status == "failed" {
            let code = response
                .pointer("/error/code")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("UNKNOWN");
            return Err(format!("native runtime startup failed ({code})"));
        }
        let phase = response
            .pointer("/snapshot/bootstrapPhase")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("starting");
        // The soak harness must be able to observe degraded/slow Tor startup
        // instead of exiting before it can record a useful runtime snapshot.
        // `runtimeId` proves that the native actor is alive; network readiness
        // is deliberately validated by the subsequent pairing/workload steps.
        let actor_alive = response
            .pointer("/runtimeId")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|value| !value.is_empty());
        if status == "succeeded" && actor_alive && phase != "failed" {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "native actor did not become observable within {timeout_seconds}s (phase={phase})"
            ));
        }
        thread::sleep(Duration::from_secs(1));
    }
}

fn validate_lab_root(root: &std::path::Path) -> Result<(), String> {
    let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") else {
        return Ok(());
    };
    let production_root = PathBuf::from(local_app_data).join("Torca");
    if is_production_root(root, &production_root) {
        return Err(format!(
            "lab root must not use the production Torca profile: {}",
            production_root.display()
        ));
    }
    Ok(())
}

fn is_production_root(root: &std::path::Path, production: &std::path::Path) -> bool {
    root == production || root.starts_with(production)
}

fn invoke(
    runtime: &mut torca_native::NativeRuntimeClient,
    request_id: &str,
) -> Result<serde_json::Value, String> {
    invoke_request(runtime, request_id, "query", "diagnostics.get", serde_json::json!({}))
}

fn execute_operation(
    runtime: &mut torca_native::NativeRuntimeClient,
    cli: &Cli,
) -> Result<(), String> {
    let (name, payload) = match cli.operation {
        Operation::Observe => return Ok(()),
        Operation::Create => ("pairing.create", serde_json::json!({})),
        Operation::Join => (
            "pairing.join",
            serde_json::json!({ "code": cli.code.as_deref().ok_or("--code is required for join")? }),
        ),
        Operation::Approve => (
            "pairing.approve",
            serde_json::json!({ "sessionIdHex": cli.session_id.as_deref().ok_or("--session-id is required for approve")? }),
        ),
    };
    let response = invoke_request(runtime, "lab-operation", "command", name, payload)?;
    let status = response.get("status").and_then(serde_json::Value::as_str).unwrap_or("unknown");
    if status != "succeeded" {
        let code = response
            .pointer("/error/code")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("UNKNOWN");
        return Err(format!("{name} failed ({code})"));
    }
    let pairings = response.pointer("/snapshot/pairings").cloned().unwrap_or_default();
    println!("lab-peer operation={name} pairings={pairings}");
    Ok(())
}

fn invoke_request(
    runtime: &mut torca_native::NativeRuntimeClient,
    request_id: &str,
    kind: &str,
    name: &str,
    payload: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let request = serde_json::json!({
        "schema": 1,
        "requestId": request_id,
        "kind": kind,
        "name": name,
        "payload": payload,
    })
    .to_string();
    let response = runtime.invoke_json(&request, Duration::from_secs(5))?;
    serde_json::from_str(&response)
        .map_err(|error| format!("decode runtime response failed: {error}"))
}

#[cfg(test)]
mod tests {
    use super::is_production_root;
    use std::path::Path;

    #[test]
    fn lab_root_does_not_accept_the_production_profile() {
        let root = Path::new("C:/Users/example/AppData/Local/Torca");
        let production = Path::new("C:/Users/example/AppData/Local/Torca");
        assert!(is_production_root(root, production));
    }

    #[test]
    fn unrelated_root_is_not_a_production_profile() {
        let root = Path::new("G:/lab/peer-a");
        let production = Path::new("C:/Users/example/AppData/Local/Torca");
        assert!(!is_production_root(root, production));
    }
}
