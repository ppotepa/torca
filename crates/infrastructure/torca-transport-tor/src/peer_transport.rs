use std::collections::VecDeque;
use std::io::{ErrorKind, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpStream};
use std::time::Duration;

use torca_foundation::{CorrelationId, OpaqueId};
use torca_peer::{PeerTransport, PeerTransportError};
use torca_peer_protocol::MAX_PEER_DATA_LEN;
use torca_wire::{
    EnvelopeId, FrameDecoder, FrameMetadata, MessageKind, ProtocolFamily, ProtocolVersion,
    VersionSupport, WireCodec, WireFlags, WireLimits,
};

use crate::Socks5Connector;

const PEER_PROTOCOL_FAMILY: u16 = 2;
const PEER_PROTOCOL_MAJOR: u16 = 1;
const PEER_PROTOCOL_MINOR: u16 = 0;
const PEER_WIRE_MESSAGE_KIND: u16 = 1;
const PEER_WIRE_PAYLOAD_OVERHEAD: usize = 1024;

/// Tor SOCKS backed peer transport with strict `torca-wire` stream framing.
///
/// `PeerSession` owns authentication and protocol state. This adapter owns only the TCP stream,
/// SOCKS connection and reconstruction of complete peer payload frames from arbitrary TCP reads.
pub struct TorPeerTransport {
    connector: Socks5Connector,
    onion_address: String,
    port: u16,
    stream: Option<TcpStream>,
    decoder: FrameDecoder,
    received: VecDeque<Vec<u8>>,
    next_wire_id: u128,
}

impl TorPeerTransport {
    /// Creates a disconnected transport for one contact onion endpoint.
    pub fn new(
        socks_address: SocketAddr,
        onion_address: impl Into<String>,
        port: u16,
        timeout: Duration,
    ) -> Result<Self, PeerTransportError> {
        let onion_address = onion_address.into();
        validate_v3_onion(&onion_address)?;
        if port == 0 {
            return Err(PeerTransportError("peer port is invalid".into()));
        }
        Ok(Self {
            connector: Socks5Connector::new(socks_address, timeout),
            onion_address,
            port,
            stream: None,
            decoder: FrameDecoder::new(peer_wire_codec()?),
            received: VecDeque::new(),
            next_wire_id: 1,
        })
    }

    fn encode_frame(&mut self, payload: &[u8]) -> Result<Vec<u8>, PeerTransportError> {
        let id = OpaqueId::from_u128(self.next_wire_id);
        self.next_wire_id = self
            .next_wire_id
            .checked_add(1)
            .ok_or_else(|| PeerTransportError("peer wire sequence exhausted".into()))?;
        let metadata = FrameMetadata::new(
            protocol_version()?,
            peer_message_kind()?,
            WireFlags::REQUIRED_KIND,
            EnvelopeId::from_opaque(id),
            CorrelationId::from_opaque(id),
        );
        peer_wire_codec()?
            .encode(metadata, payload)
            .map_err(|error| PeerTransportError(format!("peer frame encode failed: {error}")))
    }

    fn read_available(&mut self) -> Result<(), PeerTransportError> {
        let Some(stream) = self.stream.as_mut() else {
            return Err(PeerTransportError("peer transport is not connected".into()));
        };

        stream
            .set_nonblocking(true)
            .map_err(|error| io_error("enable nonblocking peer read", &error))?;
        let mut buffer = [0_u8; 16 * 1024];
        let read_result = stream.read(&mut buffer);
        let restore_result = stream.set_nonblocking(false);
        if let Err(error) = restore_result {
            return Err(io_error("restore blocking peer stream", &error));
        }

        let count = match read_result {
            Ok(0) => return Err(PeerTransportError("peer connection closed".into())),
            Ok(count) => count,
            Err(error) if error.kind() == ErrorKind::WouldBlock => return Ok(()),
            Err(error) => return Err(io_error("read peer stream", &error)),
        };

        let frames = self
            .decoder
            .push(&buffer[..count])
            .map_err(|error| PeerTransportError(format!("peer frame decode failed: {error}")))?;
        let expected_kind = peer_message_kind()?;
        for frame in frames {
            if frame.metadata().message_kind() != expected_kind {
                return Err(PeerTransportError("unexpected peer wire message kind".into()));
            }
            self.received.push_back(frame.into_payload());
        }
        Ok(())
    }
}

impl PeerTransport for TorPeerTransport {
    fn connect(&mut self) -> Result<(), PeerTransportError> {
        if let Some(stream) = self.stream.take() {
            let _ = stream.shutdown(Shutdown::Both);
        }
        self.decoder.reset();
        self.received.clear();
        let stream = self
            .connector
            .connect_onion(&self.onion_address, self.port)
            .map_err(|error| PeerTransportError(format!("Tor peer connect failed: {error}")))?;
        self.stream = Some(stream);
        Ok(())
    }

    fn send(&mut self, payload: Vec<u8>) -> Result<(), PeerTransportError> {
        let frame = self.encode_frame(&payload)?;
        let stream = self
            .stream
            .as_mut()
            .ok_or_else(|| PeerTransportError("peer transport is not connected".into()))?;
        stream
            .write_all(&frame)
            .map_err(|error| io_error("write peer frame", &error))?;
        stream.flush().map_err(|error| io_error("flush peer frame", &error))
    }

    fn try_receive(&mut self) -> Result<Option<Vec<u8>>, PeerTransportError> {
        if let Some(payload) = self.received.pop_front() {
            return Ok(Some(payload));
        }
        self.read_available()?;
        Ok(self.received.pop_front())
    }

    fn close(&mut self) -> Result<(), PeerTransportError> {
        self.decoder.reset();
        self.received.clear();
        if let Some(stream) = self.stream.take() {
            stream
                .shutdown(Shutdown::Both)
                .map_err(|error| io_error("close peer stream", &error))?;
        }
        Ok(())
    }
}

fn peer_wire_codec() -> Result<WireCodec, PeerTransportError> {
    let family = ProtocolFamily::new(PEER_PROTOCOL_FAMILY)
        .ok_or_else(|| PeerTransportError("peer protocol family is invalid".into()))?;
    let versions = VersionSupport::new(PEER_PROTOCOL_MAJOR, PEER_PROTOCOL_MINOR)
        .ok_or_else(|| PeerTransportError("peer protocol version support is invalid".into()))?;
    let limits = WireLimits::new(MAX_PEER_DATA_LEN + PEER_WIRE_PAYLOAD_OVERHEAD)
        .ok_or_else(|| PeerTransportError("peer wire payload limit is invalid".into()))?;
    Ok(WireCodec::new(family, versions, limits))
}

fn protocol_version() -> Result<ProtocolVersion, PeerTransportError> {
    ProtocolVersion::new(PEER_PROTOCOL_MAJOR, PEER_PROTOCOL_MINOR)
        .ok_or_else(|| PeerTransportError("peer protocol version is invalid".into()))
}

fn peer_message_kind() -> Result<MessageKind, PeerTransportError> {
    MessageKind::new(PEER_WIRE_MESSAGE_KIND)
        .ok_or_else(|| PeerTransportError("peer message kind is invalid".into()))
}

fn validate_v3_onion(value: &str) -> Result<(), PeerTransportError> {
    let Some(label) = value.strip_suffix(".onion") else {
        return Err(PeerTransportError("peer onion address is invalid".into()));
    };
    if label.len() != 56
        || !label
            .bytes()
            .all(|byte| matches!(byte, b'a'..=b'z' | b'2'..=b'7'))
    {
        return Err(PeerTransportError("peer onion address is invalid".into()));
    }
    Ok(())
}

fn io_error(operation: &str, error: &std::io::Error) -> PeerTransportError {
    PeerTransportError(format!("{operation} failed ({:?})", error.kind()))
}
