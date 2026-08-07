use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use torca_relay_protocol::{RELAY_HEADER_LEN, RelayCodec, RelayRequest, RelayResponse};

use crate::{RelayTransportError, RelayTransportFailureKind};

pub(crate) fn exchange_stream(
    stream: &mut TcpStream,
    request: &RelayRequest,
    timeout: Duration,
) -> Result<RelayResponse, RelayTransportError> {
    let encoded = RelayCodec::encode_request(request).map_err(|_| RelayTransportError {
        kind: RelayTransportFailureKind::InvalidResponse,
        request_was_sent: false,
    })?;
    stream.set_read_timeout(Some(timeout)).map_err(|error| before_send(error.kind()))?;
    stream.set_write_timeout(Some(timeout)).map_err(|error| before_send(error.kind()))?;
    stream.write_all(&encoded).map_err(|error| after_send(error.kind()))?;
    stream.flush().map_err(|error| after_send(error.kind()))?;

    let mut header = [0_u8; RELAY_HEADER_LEN];
    stream.read_exact(&mut header).map_err(|error| after_send(error.kind()))?;
    let frame_len =
        RelayCodec::frame_len_from_header(&header).map_err(|_| RelayTransportError {
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

pub(crate) fn before_send(kind: std::io::ErrorKind) -> RelayTransportError {
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

pub(crate) fn after_send(kind: std::io::ErrorKind) -> RelayTransportError {
    RelayTransportError {
        kind: match kind {
            std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock => {
                RelayTransportFailureKind::Timeout
            }
            _ => RelayTransportFailureKind::Disconnected,
        },
        request_was_sent: true,
    }
}
