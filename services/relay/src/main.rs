use std::net::SocketAddr;
use std::time::Duration;

use torca_relay::{RelayServer, RelayServerConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bind = std::env::var("TORCA_RELAY_BIND")
        .unwrap_or_else(|_| "127.0.0.1:8844".to_owned())
        .parse::<SocketAddr>()?;
    let timeout_ms = std::env::var("TORCA_RELAY_IO_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(15_000);
    let server = RelayServer::bind(RelayServerConfig::new(
        bind,
        Duration::from_millis(timeout_ms),
    ))?;
    server.run()?;
    Ok(())
}
