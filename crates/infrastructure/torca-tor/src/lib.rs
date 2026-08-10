//! In-process Tor backend powered by Arti.
//!
//! This crate is the only place in Torca that depends on Arti.  Platform and
//! application crates consume the stable Torca API exposed by the runtime
//! layer instead of importing Arti types directly.

use std::fmt;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::Once;
use std::time::{Duration, Instant};

use arti_client::{BootstrapBehavior, TorClient, config::TorClientConfigBuilder};
use futures_util::StreamExt;
use safelog::DisplayRedacted;
use tokio::runtime::{Builder, Runtime};
use tor_rtcompat::PreferredRuntime;

mod listener;
mod peer_transport;

pub use listener::PeerListener;
pub use peer_transport::TorPeerTransport;

pub const TOR_PEER_VIRTUAL_PORT: u16 = 17491;
const BOOTSTRAP_STALL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);
const BOOTSTRAP_RETRY_BACKOFF: [Duration; 2] = [Duration::from_secs(5), Duration::from_secs(15)];
const MAX_BOOTSTRAP_ATTEMPTS: u32 = 3;

#[derive(Debug)]
pub enum TransportError {
    Io(std::io::Error),
    InvalidState,
}

impl From<std::io::Error> for TransportError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

/// Errors raised while creating or bootstrapping the in-process client.
#[derive(Debug)]
pub struct TorError(String);

/// Stable error code exposed outside the embedded Tor implementation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TorErrorCode {
    Bootstrap,
    Stall,
    OnionService,
    Stream,
    Shutdown,
}

/// Stable phase identifier for embedded Tor startup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TorBootstrapStage {
    Network,
    OnionService,
}

/// Normalized progress event emitted by the embedded Tor service.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TorBootstrapEvent {
    pub stage: TorBootstrapStage,
    pub progress: u8,
    pub attempt: u32,
    pub retry_after_ms: Option<u64>,
    pub code: &'static str,
    pub summary: String,
}

/// Health state projected by the shared runtime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TorHealth {
    Starting,
    Ready,
    Degraded,
    Failed,
}

/// Opaque handle to a running Tor service.
pub type TorServiceHandle = Arc<TorService>;

pub type TorBootstrapObserver = Arc<dyn Fn(TorBootstrapEvent) + Send + Sync>;

/// Stable blocking stream type returned by Torca transport adapters.
pub type TorStream = std::net::TcpStream;

/// Stable onion-service address handle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OnionServiceHandle {
    pub address: String,
}

impl fmt::Display for TorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for TorError {}

/// Owns one Arti runtime and one bootstrapped Tor client.
pub struct TorService {
    runtime: Runtime,
    client: Arc<TorClient<PreferredRuntime>>,
    onion_service: Option<Arc<tor_hsservice::RunningOnionService>>,
}

static RUSTLS_PROVIDER: Once = Once::new();

fn ensure_rustls_provider() {
    RUSTLS_PROVIDER.call_once(|| {
        let provider = rustls::crypto::ring::default_provider();
        let _ = provider.install_default();
    });
}

impl TorService {
    /// Creates and bootstraps a client using persistent state under `state_root`.
    pub fn bootstrap(
        state_root: impl Into<std::path::PathBuf>,
        timeout: std::time::Duration,
    ) -> Result<Self, TorError> {
        Self::bootstrap_observed(state_root, timeout, None)
    }

    /// Creates a client while publishing redacted, monotonic progress events.
    pub fn bootstrap_observed(
        state_root: impl Into<std::path::PathBuf>,
        timeout: std::time::Duration,
        observer: Option<TorBootstrapObserver>,
    ) -> Result<Self, TorError> {
        ensure_rustls_provider();
        let state_root = state_root.into();
        std::fs::create_dir_all(&state_root)
            .map_err(|error| TorError(format!("create Arti state directory: {error}")))?;
        let runtime = Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|error| TorError(format!("create Tokio runtime: {error}")))?;
        let config_root = state_root.clone();
        let bootstrap_result = runtime.block_on(async move {
            let runtime = PreferredRuntime::current()
                .map_err(|error| TorError(format!("create Arti runtime: {error}")))?;
            let config = TorClientConfigBuilder::from_directories(
                config_root.join("state"),
                config_root.join("cache"),
            )
            .build()
            .map_err(|error| TorError(format!("build Arti configuration: {error}")))?;
            // Keep construction separate from network bootstrap. Besides making
            // progress observable, this avoids hiding a synchronous local-state
            // stall behind the generic "network bootstrap" phase.
            let client = TorClient::with_runtime(runtime)
                .config(config)
                .bootstrap_behavior(BootstrapBehavior::Manual)
                .create_unbootstrapped_async()
                .await
                .map_err(|error| TorError(format!("construct Arti client: {error}")))?;

            let mut last_error = None;
            for attempt in 1..=MAX_BOOTSTRAP_ATTEMPTS {
                match bootstrap_attempt(client.clone(), timeout, attempt, observer.as_ref()).await {
                    Ok(()) => return Ok(client),
                    Err(error) if is_unambiguous_state_corruption(&error) => return Err(error),
                    Err(error) => last_error = Some(error),
                }
                if attempt < MAX_BOOTSTRAP_ATTEMPTS {
                    let backoff = BOOTSTRAP_RETRY_BACKOFF[usize::try_from(attempt - 1)
                        .unwrap_or(BOOTSTRAP_RETRY_BACKOFF.len() - 1)
                        .min(BOOTSTRAP_RETRY_BACKOFF.len() - 1)];
                    if let Some(observer) = &observer {
                        observer(TorBootstrapEvent {
                            stage: TorBootstrapStage::Network,
                            progress: progress_percent(client.bootstrap_status().as_frac()),
                            attempt,
                            retry_after_ms: u64::try_from(backoff.as_millis()).ok(),
                            code: "TOR_BOOTSTRAP_RETRYING",
                            summary: format!(
                                "Tor bootstrap attempt {attempt} stopped making progress; retry scheduled"
                            ),
                        });
                    }
                    tokio::time::sleep(backoff).await;
                }
            }
            let error = last_error
                .unwrap_or_else(|| TorError("bootstrap Arti client failed without a diagnostic".into()));
            Err(TorError(format!(
                "bootstrap Arti client exhausted {MAX_BOOTSTRAP_ATTEMPTS} attempts: {error}"
            )))
        });
        let client = match bootstrap_result {
            Ok(client) => client,
            Err(error) => {
                // A cancelled task may still be returning from platform I/O.
                // Never let Runtime::drop wait indefinitely during retry.
                runtime.shutdown_timeout(std::time::Duration::from_secs(2));
                if is_unambiguous_state_corruption(&error) {
                    quarantine_state_cache(&state_root);
                }
                return Err(error);
            }
        };

        Ok(Self { runtime, client, onion_service: None })
    }

    /// Publishes a stable onion service and forwards accepted streams to the
    /// supplied local listener. The listener remains the shared Torca peer
    /// protocol boundary; Arti only provides the encrypted Tor transport.
    pub fn publish_onion_service(
        &mut self,
        target: SocketAddr,
        timeout: std::time::Duration,
    ) -> Result<String, TorError> {
        let client = Arc::clone(&self.client);
        let service_result = self.runtime.block_on(async move {
            tokio::time::timeout(timeout, async move {
                let nickname = tor_hsservice::HsNickname::try_from("torca-peer".to_owned())
                    .map_err(|error| TorError(format!("build onion service nickname: {error}")))?;
                let config = tor_hsservice::config::OnionServiceConfigBuilder::default()
                    .nickname(nickname)
                    .build()
                    .map_err(|error| {
                        TorError(format!("build onion service configuration: {error}"))
                    })?;
                let (running, requests) = client
                    .launch_onion_service(config)
                    .map_err(|error| TorError(format!("launch onion service: {error}")))?
                    .ok_or_else(|| {
                        TorError("onion service disabled in Arti configuration".into())
                    })?;
                let address = running
                    .onion_address()
                    .ok_or_else(|| TorError("onion service identity is unavailable".into()))?
                    .display_unredacted()
                    .to_string();
                let mut streams = tor_hsservice::handle_rend_requests(requests);
                tokio::spawn(async move {
                    while let Some(request) = streams.next().await {
                        let address = target;
                        tokio::spawn(async move {
                            let Ok(mut local) = tokio::net::TcpStream::connect(address).await
                            else {
                                return;
                            };
                            let Ok(mut remote) = request
                                .accept(tor_cell::relaycell::msg::Connected::new_empty())
                                .await
                            else {
                                return;
                            };
                            let _ = tokio::io::copy_bidirectional(&mut remote, &mut local).await;
                        });
                    }
                });
                Ok::<_, TorError>((running, address))
            })
            .await
            .map_err(|_| TorError("publish onion service timed out".into()))?
        });
        let (running, address) = service_result?;
        self.onion_service = Some(running);
        Ok(address)
    }

    /// Runs a synchronous operation on the owned async runtime.
    pub fn block_on<F, T>(&self, operation: F) -> T
    where
        F: std::future::Future<Output = T>,
    {
        self.runtime.block_on(operation)
    }

    /// Opens a direct in-process Tor stream to an onion service.
    ///
    /// The returned blocking stream is backed by a private loopback bridge
    /// owned by this Arti runtime. No external Tor process or control port is involved.
    pub fn connect_onion(
        &self,
        hostname: impl Into<String>,
        port: u16,
    ) -> Result<std::net::TcpStream, TorError> {
        self.connect_onion_with_timeout(hostname, port, std::time::Duration::from_secs(15))
    }

    /// Opens an onion stream with an explicit end-to-end connection timeout.
    pub fn connect_onion_with_timeout(
        &self,
        hostname: impl Into<String>,
        port: u16,
        timeout: std::time::Duration,
    ) -> Result<std::net::TcpStream, TorError> {
        let hostname = hostname.into();
        let client = Arc::clone(&self.client);
        let connect_timeout = timeout;
        let (address, ready) = self.runtime.block_on(async move {
            let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
                .await
                .map_err(|error| TorError(format!("bind Arti stream bridge: {error}")))?;
            let address = listener
                .local_addr()
                .map_err(|error| TorError(format!("read Arti bridge address: {error}")))?;
            let (ready_sender, ready_receiver) = tokio::sync::oneshot::channel();
            tokio::spawn(async move {
                let result = async {
                    let (mut local, _) = listener
                        .accept()
                        .await
                        .map_err(|error| TorError(format!("accept Arti bridge stream: {error}")))?;
                    let mut remote =
                        tokio::time::timeout(connect_timeout, client.connect((hostname, port)))
                            .await
                            .map_err(|_| TorError("connect to onion service timed out".into()))?
                            .map_err(|error| {
                                TorError(format!("connect to onion service: {error}"))
                            })?;
                    let _ = ready_sender.send(Ok(()));
                    tokio::io::copy_bidirectional(&mut local, &mut remote)
                        .await
                        .map_err(|error| TorError(format!("copy Arti stream: {error}")))?;
                    Ok::<(), TorError>(())
                }
                .await;
                let _ = result;
            });
            Ok::<_, TorError>((address, ready_receiver))
        })?;

        let stream = std::net::TcpStream::connect_timeout(&address, timeout)
            .map_err(|error| TorError(format!("connect to Arti stream bridge: {error}")))?;
        self.runtime.block_on(async move {
            tokio::time::timeout(timeout, ready)
                .await
                .map_err(|_| TorError("Arti stream bridge timed out".into()))?
                .map_err(|_| TorError("Arti stream bridge stopped before connection".into()))?
        })?;
        stream
            .set_nodelay(true)
            .map_err(|error| TorError(format!("configure Arti stream bridge: {error}")))?;
        Ok(stream)
    }
}

async fn bootstrap_attempt(
    client: Arc<TorClient<PreferredRuntime>>,
    timeout: Duration,
    attempt: u32,
    observer: Option<&TorBootstrapObserver>,
) -> Result<(), TorError> {
    let mut events = client.bootstrap_events();
    let bootstrap_client = client.clone();
    let mut bootstrap = tokio::spawn(async move { bootstrap_client.bootstrap().await });
    let deadline = tokio::time::sleep(timeout);
    let mut stall_tick = tokio::time::interval(Duration::from_secs(1));
    let mut last_progress = Instant::now();
    let bootstrap_started = Instant::now();
    let mut last_fraction = client.bootstrap_status().as_frac();
    let mut last_summary = client.bootstrap_status().to_string();
    notify_bootstrap(observer, last_fraction, attempt, "TOR_BOOTSTRAP_STARTING", &last_summary);
    let mut events_open = true;
    tokio::pin!(deadline);
    loop {
        if last_progress.elapsed() >= BOOTSTRAP_STALL_TIMEOUT {
            bootstrap.abort();
            let _ = bootstrap.await;
            return Err(stalled_error(last_fraction, &last_summary));
        }
        if bootstrap_started.elapsed() >= timeout {
            bootstrap.abort();
            let _ = bootstrap.await;
            return Err(TorError(format!(
                "bootstrap Arti client timed out at {:.0}% ({last_summary})",
                last_fraction * 100.0
            )));
        }
        tokio::select! {
            result = &mut bootstrap => {
                result
                    .map_err(|error| TorError(format!("join Arti bootstrap task: {error}")))?
                    .map_err(|error| TorError(format!("bootstrap Arti client: {error}")))?;
                notify_bootstrap(observer, 1.0, attempt, "TOR_BOOTSTRAP_READY", "Tor network bootstrap completed");
                return Ok(());
            }
            status = events.next(), if events_open => {
                match status {
                    Some(status) => {
                        let fraction = status.as_frac();
                        last_summary = status.to_string();
                        if fraction > last_fraction + f32::EPSILON {
                            last_fraction = fraction;
                            last_progress = Instant::now();
                        }
                        let percent = progress_percent(fraction);
                        let code = if status.blocked().is_some() {
                            "TOR_BOOTSTRAP_BLOCKED"
                        } else if percent >= 100 {
                            "TOR_BOOTSTRAP_READY"
                        } else if percent >= 15 {
                            "TOR_DIRECTORY_CONSENSUS"
                        } else {
                            "TOR_CONNECTING_DIRECTORY"
                        };
                        notify_bootstrap(observer, fraction, attempt, code, &last_summary);
                    }
                    None => events_open = false,
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
            _ = stall_tick.tick() => {
                if last_progress.elapsed() >= BOOTSTRAP_STALL_TIMEOUT {
                    bootstrap.abort();
                    let _ = bootstrap.await;
                    return Err(stalled_error(last_fraction, &last_summary));
                }
            }
            () = &mut deadline => {
                bootstrap.abort();
                let _ = bootstrap.await;
                return Err(TorError(format!(
                    "bootstrap Arti client timed out at {:.0}% ({last_summary})",
                    last_fraction * 100.0
                )));
            },
        }
    }
}

fn notify_bootstrap(
    observer: Option<&TorBootstrapObserver>,
    fraction: f32,
    attempt: u32,
    code: &'static str,
    summary: &str,
) {
    if let Some(observer) = observer {
        observer(TorBootstrapEvent {
            stage: TorBootstrapStage::Network,
            progress: progress_percent(fraction),
            attempt,
            retry_after_ms: None,
            code,
            summary: summary.to_owned(),
        });
    }
}

fn stalled_error(last_fraction: f32, last_summary: &str) -> TorError {
    TorError(format!(
        "bootstrap Arti client stalled at {:.0}% ({last_summary})",
        last_fraction * 100.0
    ))
}

fn is_unambiguous_state_corruption(error: &TorError) -> bool {
    let message = error.to_string().to_ascii_lowercase();
    ["corrupt", "integrity check failed", "invalid arti state", "malformed state"]
        .iter()
        .any(|marker| message.contains(marker))
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn progress_percent(fraction: f32) -> u8 {
    (fraction * 100.0).round().clamp(0.0, 100.0) as u8
}

fn quarantine_state_cache(state_root: &std::path::Path) {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_millis())
        .unwrap_or_default();
    let quarantine = state_root.join("quarantine").join(stamp.to_string());
    if std::fs::create_dir_all(&quarantine).is_err() {
        return;
    }
    for name in ["state", "cache"] {
        let source = state_root.join(name);
        if source.exists() {
            let _ = std::fs::rename(&source, quarantine.join(name));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{TorError, is_unambiguous_state_corruption, progress_percent};

    #[test]
    fn bootstrap_progress_is_bounded_and_rounded() {
        assert_eq!(progress_percent(-0.5), 0);
        assert_eq!(progress_percent(0.0), 0);
        assert_eq!(progress_percent(0.149), 15);
        assert_eq!(progress_percent(1.0), 100);
        assert_eq!(progress_percent(1.5), 100);
    }

    #[test]
    fn only_explicit_state_corruption_triggers_quarantine() {
        assert!(is_unambiguous_state_corruption(&TorError("cache integrity check failed".into())));
        assert!(!is_unambiguous_state_corruption(&TorError(
            "directory consensus unavailable".into()
        )));
    }
}
