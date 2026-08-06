//! Tor process lifecycle, onion-service configuration and SOCKS5 stream establishment.

use core::fmt;
use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// Observable local Tor runtime state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TorState {
    Stopped,
    Starting,
    Ready,
    Degraded,
    Failed,
    Stopping,
}

/// Hidden-service configuration owned by the local Tor process.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OnionServiceConfig {
    pub directory: PathBuf,
    pub virtual_port: u16,
    pub target: SocketAddr,
}

/// Complete local Tor process configuration.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TorProcessConfig {
    pub executable: PathBuf,
    pub data_directory: PathBuf,
    pub torrc_path: PathBuf,
    pub socks_address: SocketAddr,
    pub control_port: u16,
    pub onion_service: OnionServiceConfig,
}

impl TorProcessConfig {
    /// Renders the minimal torrc used by the owned client process.
    pub fn render_torrc(&self) -> String {
        format!(
            "DataDirectory {}\nSocksPort {}\nControlPort 127.0.0.1:{}\nCookieAuthentication 1\nHiddenServiceDir {}\nHiddenServiceVersion 3\nHiddenServicePort {} {}\nLog notice stdout\n",
            torrc_path(&self.data_directory),
            self.socks_address,
            self.control_port,
            torrc_path(&self.onion_service.directory),
            self.onion_service.virtual_port,
            self.onion_service.target
        )
    }
}

fn torrc_path(path: &Path) -> String {
    format!(
        "\"{}\"",
        path.display().to_string().replace('\\', "\\\\").replace('"', "\\\"")
    )
}

/// Redaction-safe Tor adapter failure.
#[derive(Debug)]
pub enum TorError {
    Io(std::io::Error),
    InvalidHost,
    InvalidOnionHostname,
    SocksRejected(u8),
    SocksProtocol,
    ProcessExited,
    StartupTimeout,
    ConnectionTimeout,
    InvalidState,
}

impl fmt::Display for TorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "Tor IO failure ({:?})", error.kind()),
            other => write!(formatter, "{other:?}"),
        }
    }
}
impl std::error::Error for TorError {}
impl From<std::io::Error> for TorError {
    fn from(value: std::io::Error) -> Self {
        map_io(value)
    }
}

/// Owner of one child Tor process.
pub struct TorProcess {
    config: TorProcessConfig,
    state: TorState,
    child: Option<Child>,
}

impl TorProcess {
    /// Creates a stopped process owner.
    pub const fn new(config: TorProcessConfig) -> Self {
        Self { config, state: TorState::Stopped, child: None }
    }

    /// Returns the last observed state.
    pub const fn state(&self) -> TorState {
        self.state
    }

    /// Starts the configured Tor child without blocking for bootstrap completion.
    pub fn start(&mut self) -> Result<(), TorError> {
        if self.state != TorState::Stopped || self.child.is_some() {
            return Err(TorError::InvalidState);
        }
        fs::create_dir_all(&self.config.data_directory)?;
        fs::create_dir_all(&self.config.onion_service.directory)?;
        if let Some(parent) = self.config.torrc_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&self.config.torrc_path, self.config.render_torrc())?;
        let child = Command::new(&self.config.executable)
            .arg("-f")
            .arg(&self.config.torrc_path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        self.child = Some(child);
        self.state = TorState::Starting;
        Ok(())
    }

    /// Waits until both SOCKS and the v3 onion-service hostname are available.
    ///
    /// Treating an open SOCKS port alone as readiness is insufficient: Tor can expose SOCKS before
    /// the local onion service has published its hostname.
    pub fn wait_until_ready(&mut self, timeout: Duration) -> Result<(), TorError> {
        if self.state != TorState::Starting {
            return Err(TorError::InvalidState);
        }
        let deadline = Instant::now().checked_add(timeout).ok_or(TorError::StartupTimeout)?;
        loop {
            self.refresh_state()?;
            if self.state == TorState::Failed {
                return Err(TorError::ProcessExited);
            }

            let socks_ready =
                TcpStream::connect_timeout(&self.config.socks_address, Duration::from_millis(200))
                    .is_ok();
            let onion_ready = self.onion_hostname()?.is_some();
            if socks_ready && onion_ready {
                self.state = TorState::Ready;
                return Ok(());
            }
            if Instant::now() >= deadline {
                self.state = TorState::Degraded;
                return Err(TorError::StartupTimeout);
            }
            thread::sleep(Duration::from_millis(100));
        }
    }

    /// Refreshes process health without blocking.
    pub fn refresh_state(&mut self) -> Result<TorState, TorError> {
        let Some(child) = self.child.as_mut() else {
            if self.state != TorState::Stopped {
                self.state = TorState::Failed;
            }
            return Ok(self.state);
        };
        if child.try_wait()?.is_some() {
            self.child = None;
            self.state = TorState::Failed;
        }
        Ok(self.state)
    }

    /// Stops the owned process. Repeated calls are idempotent.
    pub fn stop(&mut self) -> Result<(), TorError> {
        if self.state == TorState::Stopped && self.child.is_none() {
            return Ok(());
        }
        self.state = TorState::Stopping;
        if let Some(mut child) = self.child.take() {
            match child.try_wait()? {
                Some(_) => {}
                None => {
                    child.kill()?;
                    let _ = child.wait()?;
                }
            }
        }
        self.state = TorState::Stopped;
        Ok(())
    }

    /// Returns the local SOCKS endpoint.
    pub const fn socks_address(&self) -> SocketAddr {
        self.config.socks_address
    }

    /// Returns a validated v3 onion hostname once Tor has created it.
    pub fn onion_hostname(&self) -> Result<Option<String>, TorError> {
        let path = self.config.onion_service.directory.join("hostname");
        match fs::read_to_string(path) {
            Ok(value) => {
                let value = value.trim();
                if value.is_empty() {
                    return Ok(None);
                }
                validate_v3_onion_hostname(value)?;
                Ok(Some(value.to_owned()))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }
}

impl Drop for TorProcess {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

/// SOCKS5 connector that keeps DNS resolution inside Tor by using domain-name requests.
pub struct Socks5Connector {
    socks_address: SocketAddr,
    timeout: Duration,
}

impl Socks5Connector {
    /// Creates a connector using one deadline for connect/read/write operations.
    pub const fn new(socks_address: SocketAddr, timeout: Duration) -> Self {
        Self { socks_address, timeout }
    }

    /// Connects to an arbitrary ASCII host through SOCKS5 without local DNS resolution.
    pub fn connect(&self, host: &str, port: u16) -> Result<TcpStream, TorError> {
        validate_host(host)?;
        let mut stream = TcpStream::connect_timeout(&self.socks_address, self.timeout)
            .map_err(map_io)?;
        stream.set_read_timeout(Some(self.timeout)).map_err(map_io)?;
        stream.set_write_timeout(Some(self.timeout)).map_err(map_io)?;
        stream.write_all(&[5, 1, 0]).map_err(map_io)?;
        let mut greeting = [0_u8; 2];
        stream.read_exact(&mut greeting).map_err(map_io)?;
        if greeting != [5, 0] {
            return Err(TorError::SocksProtocol);
        }
        let mut request = Vec::with_capacity(7 + host.len());
        request.extend_from_slice(&[
            5,
            1,
            0,
            3,
            u8::try_from(host.len()).map_err(|_| TorError::InvalidHost)?,
        ]);
        request.extend_from_slice(host.as_bytes());
        request.extend_from_slice(&port.to_be_bytes());
        stream.write_all(&request).map_err(map_io)?;
        let mut header = [0_u8; 4];
        stream.read_exact(&mut header).map_err(map_io)?;
        if header[0] != 5 || header[2] != 0 {
            return Err(TorError::SocksProtocol);
        }
        if header[1] != 0 {
            return Err(TorError::SocksRejected(header[1]));
        }
        consume_bound_address(&mut stream, header[3])?;
        Ok(stream)
    }

    /// Connects only to a canonical v3 onion hostname.
    pub fn connect_onion(&self, hostname: &str, port: u16) -> Result<TcpStream, TorError> {
        validate_v3_onion_hostname(hostname)?;
        self.connect(hostname, port)
    }
}

fn validate_host(host: &str) -> Result<(), TorError> {
    if host.is_empty()
        || host.len() > 255
        || !host.is_ascii()
        || host.bytes().any(|byte| byte.is_ascii_whitespace() || byte == 0)
    {
        return Err(TorError::InvalidHost);
    }
    Ok(())
}

fn validate_v3_onion_hostname(host: &str) -> Result<(), TorError> {
    // Tor v3 hostnames contain 56 lowercase base32 characters followed by `.onion`.
    let Some(label) = host.strip_suffix(".onion") else {
        return Err(TorError::InvalidOnionHostname);
    };
    if label.len() != 56
        || !label
            .bytes()
            .all(|byte| matches!(byte, b'a'..=b'z' | b'2'..=b'7'))
    {
        return Err(TorError::InvalidOnionHostname);
    }
    Ok(())
}

fn consume_bound_address(stream: &mut TcpStream, address_type: u8) -> Result<(), TorError> {
    let length = match address_type {
        1 => 4,
        4 => 16,
        3 => {
            let mut value = [0_u8; 1];
            stream.read_exact(&mut value).map_err(map_io)?;
            usize::from(value[0])
        }
        _ => return Err(TorError::SocksProtocol),
    };
    let mut discard = vec![0_u8; length + 2];
    stream.read_exact(&mut discard).map_err(map_io)?;
    Ok(())
}

fn map_io(error: std::io::Error) -> TorError {
    if matches!(
        error.kind(),
        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
    ) {
        TorError::ConnectionTimeout
    } else {
        TorError::Io(error)
    }
}

/// Writes a torrc file for callers that own process creation separately.
pub fn write_torrc(path: &Path, config: &TorProcessConfig) -> Result<(), TorError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, config.render_torrc())?;
    Ok(())
}
