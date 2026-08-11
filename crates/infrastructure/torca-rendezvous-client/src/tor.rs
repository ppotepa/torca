use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use torca_relay_protocol::{RelayInfo, RelayRequest, RelayResponse};
use torca_tor::TorServiceHandle;

use crate::stream::exchange_stream;
use crate::{RelayTransport, RelayTransportError, RelayTransportFailureKind};

const RELAY_CONNECT_TIMEOUT: Duration = Duration::from_secs(8);

/// Relay transport whose TCP stream is opened through the in-process Arti client.
pub struct TorRelayTransport {
    client: TorServiceHandle,
    hostname: String,
    port: u16,
    stream: Option<TcpStream>,
}

/// Cloneable access to one durable Tor relay stream. Pairing operations and
/// the application-owned health worker share this transport, so health is an
/// observation of the real operation path rather than a second onion dial.
#[derive(Clone)]
pub struct SharedTorRelayTransport {
    inner: Arc<Mutex<TorRelayTransport>>,
}

impl SharedTorRelayTransport {
    pub fn new(client: TorServiceHandle, hostname: impl Into<String>, port: u16) -> Self {
        Self { inner: Arc::new(Mutex::new(TorRelayTransport::new(client, hostname, port))) }
    }

    /// Sends a health request through the existing stream. Only a failed
    /// request creates a new Tor stream, keeping the established connection
    /// warm while preserving recovery for a broken circuit.
    pub fn check_health(&self, timeout: Duration) -> Result<(), RelayTransportError> {
        self.relay_info(timeout).map(|_| ())
    }

    /// Reads build and protocol identity through the same persistent stream
    /// used by pairing. A successful response is also an authoritative health
    /// sample for that connection.
    pub fn relay_info(&self, timeout: Duration) -> Result<RelayInfo, RelayTransportError> {
        let mut transport = self.inner.lock().map_err(|_| RelayTransportError {
            kind: RelayTransportFailureKind::Unavailable,
            request_was_sent: false,
        })?;
        let response = match transport.exchange(&RelayRequest::Info, timeout) {
            Ok(response) => response,
            Err(_) => {
                transport.reconnect()?;
                transport.exchange(&RelayRequest::Info, timeout)?
            }
        };
        match response {
            RelayResponse::Info(info) => Ok(info),
            _ => Err(RelayTransportError {
                kind: RelayTransportFailureKind::InvalidResponse,
                request_was_sent: true,
            }),
        }
    }
}

impl TorRelayTransport {
    pub fn new(client: TorServiceHandle, hostname: impl Into<String>, port: u16) -> Self {
        Self { client, hostname: hostname.into(), port, stream: None }
    }

    pub fn hostname(&self) -> &str {
        &self.hostname
    }

    pub const fn port(&self) -> u16 {
        self.port
    }

    pub fn disconnect(&mut self) {
        self.stream = None;
    }
}

impl RelayTransport for TorRelayTransport {
    fn invalidate(&mut self) {
        self.disconnect();
    }

    fn reconnect(&mut self) -> Result<(), RelayTransportError> {
        self.stream = None;
        let stream = self
            .client
            .connect_onion_with_timeout(&self.hostname, self.port, RELAY_CONNECT_TIMEOUT)
            .map_err(map_tor_before_send)?;
        stream.set_nodelay(true).map_err(|error| RelayTransportError {
            kind: map_io_kind(error.kind()),
            request_was_sent: false,
        })?;
        self.stream = Some(stream);
        Ok(())
    }

    fn exchange(
        &mut self,
        request: &RelayRequest,
        timeout: Duration,
    ) -> Result<RelayResponse, RelayTransportError> {
        let stream = self.stream.as_mut().ok_or(RelayTransportError {
            kind: RelayTransportFailureKind::Disconnected,
            request_was_sent: false,
        })?;
        exchange_stream(stream, request, timeout)
    }
}

impl RelayTransport for SharedTorRelayTransport {
    fn invalidate(&mut self) {
        if let Ok(mut transport) = self.inner.lock() {
            transport.invalidate();
        }
    }

    fn reconnect(&mut self) -> Result<(), RelayTransportError> {
        self.inner
            .lock()
            .map_err(|_| RelayTransportError {
                kind: RelayTransportFailureKind::Unavailable,
                request_was_sent: false,
            })?
            .reconnect()
    }

    fn exchange(
        &mut self,
        request: &RelayRequest,
        timeout: Duration,
    ) -> Result<RelayResponse, RelayTransportError> {
        self.inner
            .lock()
            .map_err(|_| RelayTransportError {
                kind: RelayTransportFailureKind::Unavailable,
                request_was_sent: false,
            })?
            .exchange(request, timeout)
    }
}

fn map_tor_before_send(_error: impl std::fmt::Display) -> RelayTransportError {
    RelayTransportError { kind: RelayTransportFailureKind::Unavailable, request_was_sent: false }
}

fn map_io_kind(kind: std::io::ErrorKind) -> RelayTransportFailureKind {
    if matches!(kind, std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock) {
        RelayTransportFailureKind::Timeout
    } else {
        RelayTransportFailureKind::Disconnected
    }
}
