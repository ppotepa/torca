use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

use torca_relay_protocol::{RELAY_HEADER_LEN, RelayCodec, RelayRequest, RelayResponse};

use crate::{RelayTransport, RelayTransportError, RelayTransportFailureKind};

/// Blocking direct TCP transport for the relay wire protocol.
pub struct TcpRelayTransport {
    endpoint: SocketAddr,
    connect_timeout: Duration,
    stream: Option<TcpStream>,
}

impl TcpRelayTransport {
    pub const fn new(endpoint: SocketAddr, connect_timeout: Duration) -> Self {
        Self { endpoint, connect_timeout, stream: None }
    }

    pub const fn endpoint(&self) -> SocketAddr {
        self.endpoint
    }

    pub fn disconnect(&mut self) {
        self.stream = None;
    }
}

impl RelayTransport for TcpRelayTransport {
    fn reconnect(&mut self) -> Result<(), RelayTransportError> {
        self.stream = None;
        let stream = TcpStream::connect_timeout(&self.endpoint, self.connect_timeout)
            .map_err(|error| before_send(error.kind()))?;
        stream.set_nodelay(true).map_err(|error| before_send(error.kind()))?;
        self.stream = Some(stream);
        Ok(())
    }

    fn exchange(
        &mut self,
        request: &RelayRequest,
        timeout: Duration,
    ) -> Result<RelayResponse, RelayTransportError> {
        let encoded = RelayCodec::encode_request(request).map_err(|_| RelayTransportError {
            kind: RelayTransportFailureKind::InvalidResponse,
            request_was_sent: false,
        })?;
        let stream = self.stream.as_mut().ok_or(RelayTransportError {
            kind: RelayTransportFailureKind::Disconnected,
            request_was_sent: false,
        })?;
        stream.set_read_timeout(Some(timeout)).map_err(|error| before_send(error.kind()))?;
        stream.set_write_timeout(Some(timeout)).map_err(|error| before_send(error.kind()))?;

        // Once write_all is attempted, a failure is conservatively outcome-unknown: the kernel or
        // remote relay may already have received a complete request even when the local call fails.
        stream.write_all(&encoded).map_err(|error| after_send(error.kind()))?;
        stream.flush().map_err(|error| after_send(error.kind()))?;

        let mut header = [0_u8; RELAY_HEADER_LEN];
        stream.read_exact(&mut header).map_err(|error| after_send(error.kind()))?;
        let frame_len = RelayCodec::frame_len_from_header(&header).map_err(|_| RelayTransportError {
            kind: RelayTransportFailureKind::InvalidResponse,
            request_was_sent: true,
        })?;
        let payload_len = frame_len - RELAY_HEADER_LEN;
        let mut frame = Vec::with_capacity(frame_len);
        frame.extend_from_slice(&header);
        if payload_len != 0 {
            let mut payload = vec![0_u8; payload_len];
            stream.read_exact(&mut payload).map_err(|error| after_send(error.kind()))?;
            frame.extend_from_slice(&payload);
        }
        RelayCodec::decode_response(&frame).map_err(|_| RelayTransportError {
            kind: RelayTransportFailureKind::InvalidResponse,
            request_was_sent: true,
        })
    }
}

fn before_send(kind: std::io::ErrorKind) -> RelayTransportError {
    RelayTransportError {
        kind: match kind {
            std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock => {
                RelayTransportFailureKind::Timeout
            }
            std::io::ErrorKind::ConnectionRefused
            | std::io::ErrorKind::AddrNotAvailable
            | std::io::ErrorKind::NotConnected => RelayTransportFailureKind::Unavailable,
            _ => RelayTransportFailureKind::Disconnected,
        },
        request_was_sent: false,
    }
}

fn after_send(kind: std::io::ErrorKind) -> RelayTransportError {
    RelayTransportError {
        kind: if matches!(kind, std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock) {
            RelayTransportFailureKind::Timeout
        } else {
            RelayTransportFailureKind::Disconnected
        },
        request_was_sent: true,
    }
}
