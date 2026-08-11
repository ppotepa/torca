use crate::TorServiceHandle;
use std::collections::VecDeque;
use std::io::{ErrorKind, Read, Write};
use std::net::{Shutdown, TcpStream};
use torca_foundation::{CorrelationId, OpaqueId};
use torca_peer::{PeerTransport, PeerTransportError};
use torca_peer_protocol::MAX_PEER_DATA_LEN;
use torca_wire::{
    EnvelopeId, FrameDecoder, FrameMetadata, MessageKind, ProtocolFamily, ProtocolVersion,
    VersionSupport, WireCodec, WireFlags, WireLimits,
};

const PEER_PROTOCOL_FAMILY: u16 = 2;
const PEER_PROTOCOL_MAJOR: u16 = 1;
const PEER_PROTOCOL_MINOR: u16 = 0;
const PEER_WIRE_MESSAGE_KIND: u16 = 1;
const PEER_WIRE_PAYLOAD_OVERHEAD: usize = 1024;

/// Embedded-Tor peer transport with strict `torca-wire` stream framing.
pub struct TorPeerTransport {
    client: Option<TorServiceHandle>,
    onion_address: String,
    port: u16,
    stream: Option<TcpStream>,
    decoder: FrameDecoder,
    received: VecDeque<Vec<u8>>,
    next_wire_id: u128,
}

impl TorPeerTransport {
    /// Creates a disconnected transport. Onion validation remains at the embedded Tor boundary
    /// so every actual connection is validated even after configuration reloads.
    pub fn new(client: TorServiceHandle, onion_address: impl Into<String>, port: u16) -> Self {
        Self {
            client: Some(client),
            onion_address: onion_address.into(),
            port,
            stream: None,
            decoder: FrameDecoder::new(peer_wire_codec()),
            received: VecDeque::new(),
            next_wire_id: 1,
        }
    }

    /// Wraps an already accepted loopback stream for the local onion-service listener.
    pub fn from_incoming_stream(stream: TcpStream) -> Result<Self, PeerTransportError> {
        stream
            .set_nodelay(true)
            .map_err(|error| io_error("configure incoming peer stream", &error))?;
        Ok(Self {
            client: None,
            onion_address: String::new(),
            port: 0,
            stream: Some(stream),
            decoder: FrameDecoder::new(peer_wire_codec()),
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
            protocol_version(),
            peer_message_kind(),
            WireFlags::REQUIRED_KIND,
            EnvelopeId::from_opaque(id),
            CorrelationId::from_opaque(id),
        );
        peer_wire_codec()
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
        let expected_kind = peer_message_kind();
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
        if self.port == 0 {
            return Err(PeerTransportError("peer port is invalid".into()));
        }
        if let Some(stream) = self.stream.take() {
            let _ = stream.shutdown(Shutdown::Both);
        }
        self.decoder.reset();
        self.received.clear();
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| PeerTransportError("Tor peer client is unavailable".into()))?;
        let stream = client
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
        stream.write_all(&frame).map_err(|error| io_error("write peer frame", &error))?;
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

fn peer_wire_codec() -> WireCodec {
    let family =
        ProtocolFamily::new(PEER_PROTOCOL_FAMILY).expect("peer protocol family is non-zero");
    let versions = VersionSupport::new(PEER_PROTOCOL_MAJOR, PEER_PROTOCOL_MINOR)
        .expect("peer protocol major is non-zero");
    let limits = WireLimits::new(MAX_PEER_DATA_LEN + PEER_WIRE_PAYLOAD_OVERHEAD)
        .expect("peer wire payload bound is valid");
    WireCodec::new(family, versions, limits)
}
fn protocol_version() -> ProtocolVersion {
    ProtocolVersion::new(PEER_PROTOCOL_MAJOR, PEER_PROTOCOL_MINOR)
        .expect("peer protocol major is non-zero")
}
fn peer_message_kind() -> MessageKind {
    MessageKind::new(PEER_WIRE_MESSAGE_KIND).expect("peer wire kind is non-zero")
}
fn io_error(operation: &str, error: &std::io::Error) -> PeerTransportError {
    PeerTransportError(format!("{operation} failed ({:?})", error.kind()))
}
