use std::io::{Read, Write};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use torca_relay_protocol::{RELAY_HEADER_LEN, RelayCodec, RelayRequest, RelayResponse};
use torca_tor::{OnionServiceHealth, TorService};

const TOR_BOOTSTRAP_TIMEOUT: Duration = Duration::from_secs(180);
const ONION_PUBLISH_TIMEOUT: Duration = Duration::from_secs(60);
const ONION_E2E_PROBE_TIMEOUT: Duration = Duration::from_secs(10);
const RELAY_ONION_PORT: u16 = 443;
const ONION_DEGRADED_GRACE: Duration = Duration::from_secs(75);
const ONION_RETRY_BACKOFF: [Duration; 4] = [
    Duration::from_secs(5),
    Duration::from_secs(15),
    Duration::from_secs(30),
    Duration::from_secs(60),
];

pub fn clear_file(path: &PathBuf) {
    let _ = std::fs::remove_file(path);
}

pub fn publish_onion_forever(
    state_root: PathBuf,
    local_target: SocketAddr,
    endpoint_file: PathBuf,
    ready_file: PathBuf,
    status_file: PathBuf,
) {
    let mut failures = 0_u32;
    loop {
        clear_file(&endpoint_file);
        clear_file(&ready_file);
        clear_file(&status_file);
        eprintln!("torca-relay: bootstrapping in-process Arti backend");
        let attempt = (|| -> Result<(), String> {
            let tor = TorService::bootstrap(state_root.clone(), TOR_BOOTSTRAP_TIMEOUT)
                .map_err(|error| error.to_string())?;
            let endpoint = tor
                .publish_onion_service_on_port(
                    RELAY_ONION_PORT,
                    local_target,
                    ONION_PUBLISH_TIMEOUT,
                )
                .map_err(|error| error.to_string())?;
            write_endpoint(&endpoint_file, &endpoint).map_err(|error| error.to_string())?;
            write_status(&status_file, &endpoint, "publishing", false, 0);
            eprintln!("torca-relay: onion allocated at {endpoint}:443; awaiting reachability");
            wait_for_onion_reachability(&tor, &ready_file, &status_file, &endpoint)
        })();
        match attempt {
            Ok(()) => failures = 0,
            Err(error) => {
                clear_file(&ready_file);
                clear_file(&status_file);
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
    status_file: &PathBuf,
    endpoint: &str,
) -> Result<(), String> {
    let started = std::time::Instant::now();
    let mut last_reported = None;
    let mut degraded_since = None;
    let mut was_reachable = false;
    let mut e2e_verified = false;
    let mut next_e2e_probe = started + Duration::from_secs(5);
    loop {
        let health = tor.onion_service_health();
        let state_changed = last_reported != Some(health);
        if state_changed || started.elapsed().as_secs().is_multiple_of(30) {
            eprintln!(
                "torca-relay: onion publication health={health:?}, elapsed={} s",
                started.elapsed().as_secs()
            );
            last_reported = Some(health);
            write_status(
                status_file,
                endpoint,
                if e2e_verified {
                    "reachable"
                } else {
                    onion_health_state(health)
                },
                e2e_verified,
                started.elapsed().as_millis(),
            );
        }
        // Arti documents `is_fully_reachable()` as a one-way implication:
        // while its aggregate status is still Bootstrapping/Publishing the
        // service can already be reachable. Probe the real client path at a
        // controlled cadence instead of waiting indefinitely for that event.
        if !e2e_verified
            && matches!(
                health,
                OnionServiceHealth::Publishing | OnionServiceHealth::Reachable
            )
            && std::time::Instant::now() >= next_e2e_probe
        {
            match verify_onion_endpoint(tor, endpoint) {
                Ok(()) => {
                    e2e_verified = true;
                    was_reachable = true;
                    write_status(
                        status_file,
                        endpoint,
                        "reachable",
                        true,
                        started.elapsed().as_millis(),
                    );
                    write_endpoint(ready_file, endpoint).map_err(|error| error.to_string())?;
                    eprintln!("torca-relay: onion endpoint E2E probe passed");
                    eprintln!("torca-relay: onion reachable at {endpoint}:443");
                }
                Err(error) => {
                    clear_file(ready_file);
                    eprintln!("torca-relay: onion endpoint E2E probe pending: {error}");
                    next_e2e_probe = std::time::Instant::now() + Duration::from_secs(10);
                }
            }
        }
        match health {
            OnionServiceHealth::Reachable => {
                degraded_since = None;
                was_reachable = true;
                if (state_changed || !ready_file.exists()) && e2e_verified {
                    write_endpoint(ready_file, endpoint).map_err(|error| error.to_string())?;
                    eprintln!("torca-relay: onion reachable at {endpoint}:443");
                }
            }
            OnionServiceHealth::Publishing => {
                degraded_since = None;
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

fn onion_health_state(health: OnionServiceHealth) -> &'static str {
    match health {
        OnionServiceHealth::Publishing => "publishing",
        OnionServiceHealth::Reachable => "reachable",
        OnionServiceHealth::Degraded => "degraded",
        OnionServiceHealth::Failed => "failed",
        OnionServiceHealth::Stopped => "stopped",
    }
}

/// Proves the same path that clients use: this Arti instance dials the newly
/// published onion address and receives a relay protocol Health response.
fn verify_onion_endpoint(tor: &TorService, endpoint: &str) -> Result<(), String> {
    let mut stream = tor
        .connect_onion_with_timeout(endpoint, RELAY_ONION_PORT, ONION_E2E_PROBE_TIMEOUT)
        .map_err(|error| format!("connect onion endpoint for E2E probe: {error}"))?;
    stream
        .set_read_timeout(Some(ONION_E2E_PROBE_TIMEOUT))
        .map_err(|error| format!("set E2E probe read timeout: {error}"))?;
    stream
        .set_write_timeout(Some(ONION_E2E_PROBE_TIMEOUT))
        .map_err(|error| format!("set E2E probe write timeout: {error}"))?;
    stream
        .write_all(
            &RelayCodec::encode_request(&RelayRequest::Health)
                .map_err(|error| error.to_string())?,
        )
        .map_err(|error| format!("write E2E probe: {error}"))?;
    let mut header = [0_u8; RELAY_HEADER_LEN];
    stream
        .read_exact(&mut header)
        .map_err(|error| format!("read E2E probe header: {error}"))?;
    let frame_len = RelayCodec::frame_len_from_header(&header).map_err(|error| error.to_string())?;
    let mut frame = Vec::with_capacity(frame_len);
    frame.extend_from_slice(&header);
    frame.resize(frame_len, 0);
    stream
        .read_exact(&mut frame[RELAY_HEADER_LEN..])
        .map_err(|error| format!("read E2E probe response: {error}"))?;
    match RelayCodec::decode_response(&frame).map_err(|error| error.to_string())? {
        RelayResponse::Healthy => Ok(()),
        response => Err(format!("unexpected E2E probe response: {response:?}")),
    }
}

fn write_endpoint(path: &PathBuf, endpoint: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, format!("{endpoint}:443\n"))
}

fn write_status(
    path: &PathBuf,
    endpoint: &str,
    state: &str,
    e2e_verified: bool,
    elapsed_ms: u128,
) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let body = format!(
        "{{\"schema\":1,\"endpoint\":\"{endpoint}:443\",\"state\":\"{state}\",\"e2eVerified\":{e2e_verified},\"elapsedMs\":{elapsed_ms}}}\n"
    );
    let temporary = path.with_extension("json.tmp");
    if std::fs::write(&temporary, body).is_ok() {
        let _ = std::fs::rename(temporary, path);
    }
}
