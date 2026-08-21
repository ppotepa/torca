//! Process-isolated production-runtime peer for BATTERY1 laboratory runs.
//!
//! Each invocation owns one native runtime.  A scenario orchestrator can run
//! two copies with different roots; their identities, databases and Tor caches
//! remain isolated exactly as on two physical clients.

use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "torca-lab-peer")]
struct Cli {
    /// Dedicated storage root. It must not point at a normal Torca profile.
    #[arg(long)]
    root: PathBuf,
    /// How long the real native runtime remains available for orchestration.
    #[arg(long, default_value_t = 30)]
    duration_seconds: u64,
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
    let mut runtime = torca_native::NativeRuntimeClient::acquire_at(&cli.root)?;
    {
        let started = Instant::now();
        let deadline = started + Duration::from_secs(cli.duration_seconds);
        while Instant::now() < deadline {
            let response = invoke(&mut runtime, "lab-diagnostics")?;
            let status =
                response.get("status").and_then(serde_json::Value::as_str).unwrap_or("unknown");
            let revision =
                response.get("revision").and_then(serde_json::Value::as_u64).unwrap_or(0);
            println!("lab-peer root={} status={status} revision={revision}", cli.root.display());
            thread::sleep(Duration::from_secs(1));
        }
        Ok(())
    }
}

fn invoke(
    runtime: &mut torca_native::NativeRuntimeClient,
    request_id: &str,
) -> Result<serde_json::Value, String> {
    let request = serde_json::json!({
        "schema": 1,
        "requestId": request_id,
        "kind": "query",
        "name": "diagnostics.get",
        "payload": {},
    })
    .to_string();
    let response = runtime.invoke_json(&request, Duration::from_secs(5))?;
    serde_json::from_str(&response)
        .map_err(|error| format!("decode runtime response failed: {error}"))
}
