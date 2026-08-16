mod health_check;
mod onion_worker;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use health_check::protocol_health_check;
use onion_worker::{clear_file, publish_onion_forever};
use torca_relay::{RelayServer, RelayServerConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().nth(1).as_deref() == Some("health-check") {
        return protocol_health_check();
    }

    let bind = std::env::var("TORCA_RELAY_BIND")
        .unwrap_or_else(|_| "127.0.0.1:8844".to_owned())
        .parse::<SocketAddr>()?;
    let timeout_ms = std::env::var("TORCA_RELAY_IO_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        // Pairing's adaptive poll backoff reaches 30 seconds. Keep a small
        // margin so a healthy idle Tor stream is not forcibly recycled before
        // the next poll; individual client operations retain their own much
        // shorter request deadline.
        .unwrap_or(45_000);
    let server = RelayServer::bind(RelayServerConfig::new(
        bind,
        Duration::from_millis(timeout_ms),
    ))?;

    let state_root = std::env::var_os("TORCA_TOR_STATE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(".torca/tor"));
    let endpoint_file = std::env::var_os("TORCA_RELAY_ENDPOINT_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(".torca/relay_endpoint.txt"));
    let ready_file = std::env::var_os("TORCA_RELAY_READY_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| endpoint_file.with_file_name("relay_ready.txt"));
    let status_file = std::env::var_os("TORCA_RELAY_STATUS_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| endpoint_file.with_file_name("relay_status.json"));

    clear_file(&endpoint_file);
    clear_file(&ready_file);
    clear_file(&status_file);
    let local_target = SocketAddr::from(([127, 0, 0, 1], bind.port()));

    // The local broker is useful and healthy before its public onion address
    // is reachable. Public publication/recovery belongs to its own worker so
    // slow directory bootstrap cannot terminate or block the relay process.
    let _onion_worker = thread::Builder::new()
        .name("torca-relay-onion".into())
        .spawn(move || {
            publish_onion_forever(
                state_root,
                local_target,
                endpoint_file,
                ready_file,
                status_file,
            );
        })?;

    server.run()?;
    Ok(())
}
