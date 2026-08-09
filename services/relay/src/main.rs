use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use torca_relay::{RelayServer, RelayServerConfig};
use torca_tor::TorService;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bind = std::env::var("TORCA_RELAY_BIND")
        .unwrap_or_else(|_| "127.0.0.1:8844".to_owned())
        .parse::<SocketAddr>()?;
    let timeout_ms = std::env::var("TORCA_RELAY_IO_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(15_000);
    let server =
        RelayServer::bind(RelayServerConfig::new(bind, Duration::from_millis(timeout_ms)))?;
    let state_root = std::env::var_os("TORCA_TOR_STATE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(".torca/tor"));
    let endpoint_file = std::env::var_os("TORCA_RELAY_ENDPOINT_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(".torca/relay_endpoint.txt"));
    if endpoint_file.exists() {
        std::fs::remove_file(&endpoint_file)?;
    }
    eprintln!("torca-relay: bootstrapping in-process Arti backend");
    let mut tor = TorService::bootstrap(state_root, Duration::from_secs(180))?;
    let local_target = SocketAddr::from(([127, 0, 0, 1], bind.port()));
    let endpoint = tor.publish_onion_service(local_target, Duration::from_secs(60))?;
    if let Some(parent) = endpoint_file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&endpoint_file, format!("{endpoint}:443\n"))?;
    eprintln!("torca-relay: endpoint ready at {endpoint}:443");
    server.run()?;
    Ok(())
}
