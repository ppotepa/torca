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
    validate_lab_root(&cli.root)?;
    let mut runtime = torca_native::NativeRuntimeClient::acquire_at(&cli.root)?;
    {
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
