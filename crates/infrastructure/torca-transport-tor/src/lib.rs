//! Tor process lifecycle, onion-service configuration and SOCKS5 stream establishment.

use core::fmt;
use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TorState {
    Stopped,
    Starting,
    Ready,
    Degraded,
    Failed,
    Stopping,
}
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OnionServiceConfig {
    pub directory: PathBuf,
    pub virtual_port: u16,
    pub target: SocketAddr,
}
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
    format!("\"{}\"", path.display().to_string().replace('\\', "\\\\").replace('"', "\\\""))
}
#[derive(Debug)]
pub enum TorError {
    Io(std::io::Error),
    InvalidHost,
    SocksRejected(u8),
    SocksProtocol,
    ProcessExited,
    StartupTimeout,
    InvalidState,
}
impl fmt::Display for TorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for TorError {}
impl From<std::io::Error> for TorError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}
pub struct TorProcess {
    config: TorProcessConfig,
    state: TorState,
    child: Option<Child>,
}
impl TorProcess {
    pub fn new(config: TorProcessConfig) -> Self {
        Self { config, state: TorState::Stopped, child: None }
    }
    pub const fn state(&self) -> TorState {
        self.state
    }
    pub fn start(&mut self) -> Result<(), TorError> {
        if self.state != TorState::Stopped {
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
    pub fn wait_until_ready(&mut self, timeout: Duration) -> Result<(), TorError> {
        if self.state != TorState::Starting {
            return Err(TorError::InvalidState);
        }
        let deadline = Instant::now().checked_add(timeout).ok_or(TorError::StartupTimeout)?;
        loop {
            if self.child.as_mut().ok_or(TorError::ProcessExited)?.try_wait()?.is_some() {
                self.state = TorState::Failed;
                return Err(TorError::ProcessExited);
            }
            if TcpStream::connect_timeout(&self.config.socks_address, Duration::from_millis(200))
                .is_ok()
            {
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
    pub fn stop(&mut self) -> Result<(), TorError> {
        if matches!(self.state, TorState::Stopped | TorState::Stopping) {
            return Ok(());
        }
        self.state = TorState::Stopping;
        if let Some(mut child) = self.child.take() {
            child.kill()?;
            let _ = child.wait()?;
        }
        self.state = TorState::Stopped;
        Ok(())
    }
    pub const fn socks_address(&self) -> SocketAddr {
        self.config.socks_address
    }
    pub fn onion_hostname(&self) -> Result<Option<String>, TorError> {
        let path = self.config.onion_service.directory.join("hostname");
        match fs::read_to_string(path) {
            Ok(value) => Ok(Some(value.trim().to_owned())),
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
pub struct Socks5Connector {
    socks_address: SocketAddr,
    timeout: Duration,
}
impl Socks5Connector {
    pub const fn new(socks_address: SocketAddr, timeout: Duration) -> Self {
        Self { socks_address, timeout }
    }
    pub fn connect(&self, host: &str, port: u16) -> Result<TcpStream, TorError> {
        if host.is_empty() || host.len() > 255 || !host.is_ascii() {
            return Err(TorError::InvalidHost);
        }
        let mut stream = TcpStream::connect_timeout(&self.socks_address, self.timeout)?;
        stream.set_read_timeout(Some(self.timeout))?;
        stream.set_write_timeout(Some(self.timeout))?;
        stream.write_all(&[5, 1, 0])?;
        let mut greeting = [0_u8; 2];
        stream.read_exact(&mut greeting)?;
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
        stream.write_all(&request)?;
        let mut header = [0_u8; 4];
        stream.read_exact(&mut header)?;
        if header[0] != 5 {
            return Err(TorError::SocksProtocol);
        }
        if header[1] != 0 {
            return Err(TorError::SocksRejected(header[1]));
        }
        consume_bound_address(&mut stream, header[3])?;
        Ok(stream)
    }
}
fn consume_bound_address(stream: &mut TcpStream, address_type: u8) -> Result<(), TorError> {
    let length = match address_type {
        1 => 4,
        4 => 16,
        3 => {
            let mut value = [0_u8; 1];
            stream.read_exact(&mut value)?;
            usize::from(value[0])
        }
        _ => return Err(TorError::SocksProtocol),
    };
    let mut discard = vec![0_u8; length + 2];
    stream.read_exact(&mut discard)?;
    Ok(())
}
pub fn write_torrc(path: &Path, config: &TorProcessConfig) -> Result<(), TorError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, config.render_torrc())?;
    Ok(())
}
