use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use torca_relay::{RelayServer, RelayServerConfig};
use torca_relay_protocol::{RELAY_HEADER_LEN, RelayCodec, RelayRequest, RelayResponse};
use torca_tor::{OnionServiceHealth, TorService};

const TOR_BOOTSTRAP_TIMEOUT: Duration = Duration::from_secs(180);
const ONION_PUBLISH_TIMEOUT: Duration = Duration::from_secs(60);
const ONION_DEGRADED_GRACE: Duration = Duration::from_secs(75);
const ONION_RETRY_BACKOFF: [Duration; 4] = [
    Duration::from_secs(5),
    Duration::from_secs(15),
    Duration::from_secs(30),
    Duration::from_secs(60),
];

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
    let server =
        RelayServer::bind(RelayServerConfig::new(bind, Duration::from_millis(timeout_ms)))?;
    let state_root = std::env::var_os("TORCA_TOR_STATE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(".torca/tor"));
    let endpoint_file = std::env::var_os("TORCA_RELAY_ENDPOINT_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(".torca/relay_endpoint.txt"));
    let ready_file = std::env::var_os("TORCA_RELAY_READY_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| endpoint_file.with_file_name("relay_ready.txt"));
    clear_file(&endpoint_file);
    clear_file(&ready_file);
    let local_target = SocketAddr::from(([127, 0, 0, 1], bind.port()));

    // The local broker is useful and healthy before its public onion address
    // is reachable.  Publishing used to happen synchronously here; a slow
    // directory bootstrap then terminated the entire process, making Docker
    // restart it and discarding all progress.  Keep liveness separate from
    // public reachability and let this durable worker recover in place.
    let _onion_worker =
        thread::Builder::new().name("torca-relay-onion".into()).spawn(move || {
            publish_onion_forever(state_root, local_target, endpoint_file, ready_file);
        })?;
    server.run()?;
    Ok(())
}

fn publish_onion_forever(
    state_root: PathBuf,
    local_target: SocketAddr,
    endpoint_file: PathBuf,
    ready_file: PathBuf,
) {
    let mut failures = 0_u32;
    loop {
        clear_file(&endpoint_file);
        clear_file(&ready_file);
        eprintln!("torca-relay: bootstrapping in-process Arti backend");
        let attempt = (|| -> Result<(), String> {
            let tor = TorService::bootstrap(state_root.clone(), TOR_BOOTSTRAP_TIMEOUT)
                .map_err(|error| error.to_string())?;
            let endpoint = tor
                .publish_onion_service(local_target, ONION_PUBLISH_TIMEOUT)
                .map_err(|error| error.to_string())?;
            write_endpoint(&endpoint_file, &endpoint).map_err(|error| error.to_string())?;
            eprintln!("torca-relay: onion allocated at {endpoint}:443; awaiting reachability");
            wait_for_onion_reachability(&tor, &ready_file, &endpoint)
        })();
        match attempt {
            Ok(()) => failures = 0,
            Err(error) => {
                clear_file(&ready_file);
                failures = failures.saturating_add(1);
                let index = usize::try_from(failures.saturating_sub(1))
                    .unwrap_or(usize::MAX)
                    .min(ONION_RETRY_BACKOFF.len() - 1);
                let delay = ONION_RETRY_BACKOFF[index];
                eprintln!(
                    "torca-relay: onion publication failed: {error}; retrying in {} s",
                    delay.as_secs()
                );
                thread::sleep(delay);
            }
        }
    }
}

fn wait_for_onion_reachability(
    tor: &TorService,
    ready_file: &PathBuf,
    endpoint: &str,
) -> Result<(), String> {
    let started = std::time::Instant::now();
    let mut last_reported = None;
    let mut degraded_since = None;
    let mut was_reachable = false;
    loop {
        let health = tor.onion_service_health();
        let state_changed = last_reported != Some(health);
        if state_changed || started.elapsed().as_secs().is_multiple_of(30) {
            eprintln!(
                "torca-relay: onion publication health={health:?}, elapsed={} s",
                started.elapsed().as_secs()
            );
            last_reported = Some(health);
        }
        match health {
            OnionServiceHealth::Reachable => {
                degraded_since = None;
                was_reachable = true;
                if state_changed || !ready_file.exists() {
                    write_endpoint(ready_file, endpoint).map_err(|error| error.to_string())?;
                    eprintln!("torca-relay: onion reachable at {endpoint}:443");
                }
            }
            OnionServiceHealth::Publishing => {
                degraded_since = None;
                // Arti periodically republishes hidden-service descriptors.
                // During that refresh it can report Bootstrapping/Publishing
                // for many minutes even though the previously published
                // descriptor and established client streams still work. Once
                // reachability has been proven, do not turn a descriptor
                // refresh into a false relay outage.
                if !was_reachable {
                    clear_file(ready_file);
                }
            }
            OnionServiceHealth::Degraded => {
                let since = degraded_since.get_or_insert_with(std::time::Instant::now);
                if since.elapsed() >= ONION_DEGRADED_GRACE {
                    clear_file(ready_file);
                    return Err(format!(
                        "onion service remained degraded for {} seconds",
                        ONION_DEGRADED_GRACE.as_secs()
                    ));
                } else if !was_reachable {
                    clear_file(ready_file);
                }
            }
            OnionServiceHealth::Failed | OnionServiceHealth::Stopped => {
                clear_file(ready_file);
                return Err("onion service stopped before it became reachable".into());
            }
        }
        thread::sleep(Duration::from_secs(1));
    }
}

fn write_endpoint(path: &PathBuf, endpoint: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, format!("{endpoint}:443\n"))
}

fn clear_file(path: &PathBuf) {
    let _ = std::fs::remove_file(path);
}

fn protocol_health_check() -> Result<(), Box<dyn std::error::Error>> {
    let address =
        std::env::var("TORCA_RELAY_HEALTH_ADDRESS").unwrap_or_else(|_| "127.0.0.1:8844".to_owned());
    let mut stream = TcpStream::connect_timeout(&address.parse()?, Duration::from_secs(2))?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    stream.write_all(&RelayCodec::encode_request(&RelayRequest::Health)?)?;
    let mut header = [0_u8; RELAY_HEADER_LEN];
    stream.read_exact(&mut header)?;
    let frame_len = RelayCodec::frame_len_from_header(&header)?;
    let mut frame = Vec::with_capacity(frame_len);
    frame.extend_from_slice(&header);
    frame.resize(frame_len, 0);
    stream.read_exact(&mut frame[RELAY_HEADER_LEN..])?;
    match RelayCodec::decode_response(&frame)? {
        RelayResponse::Healthy => Ok(()),
        response => Err(format!("unexpected relay health response: {response:?}").into()),
    }
}
