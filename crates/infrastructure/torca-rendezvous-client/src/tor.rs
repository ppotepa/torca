use std::net::TcpStream;
use std::time::Duration;

use torca_relay_protocol::{RelayRequest, RelayResponse};
use torca_transport_tor::{Socks5Connector, TorError};

use crate::stream::exchange_stream;
use crate::{RelayTransport, RelayTransportError, RelayTransportFailureKind};

/// Relay transport whose TCP stream is opened through the owned Tor SOCKS endpoint.
pub struct TorRelayTransport {
    connector: Socks5Connector,
    hostname: String,
    port: u16,
    stream: Option<TcpStream>,
}

impl TorRelayTransport {
    pub fn new(connector: Socks5Connector, hostname: impl Into<String>, port: u16) -> Self {
        Self { connector, hostname: hostname.into(), port, stream: None }
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
    fn reconnect(&mut self) -> Result<(), RelayTransportError> {
        self.stream = None;
        let stream = self
            .connector
            .connect_onion(&self.hostname, self.port)
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

fn map_tor_before_send(error: TorError) -> RelayTransportError {
    RelayTransportError {
        kind: match error {
            TorError::ConnectionTimeout | TorError::StartupTimeout => {
                RelayTransportFailureKind::Timeout
            }
            TorError::Io(error) => map_io_kind(error.kind()),
            TorError::ProcessExited
            | TorError::InvalidState
            | TorError::InvalidHost
            | TorError::InvalidOnionHostname
            | TorError::SocksRejected(_)
            | TorError::SocksProtocol => RelayTransportFailureKind::Unavailable,
        },
        request_was_sent: false,
    }
}

fn map_io_kind(kind: std::io::ErrorKind) -> RelayTransportFailureKind {
    if matches!(kind, std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock) {
        RelayTransportFailureKind::Timeout
    } else {
        RelayTransportFailureKind::Disconnected
    }
}
