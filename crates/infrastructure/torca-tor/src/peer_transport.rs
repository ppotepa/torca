use crate::TorServiceHandle;
use std::collections::VecDeque;
use std::io::{ErrorKind, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use torca_foundation::{CorrelationId, OpaqueId};
use torca_peer::{PeerTransport, PeerTransportError};
use torca_peer_protocol::MAX_PEER_DATA_LEN;
use torca_transport_api::{
    EnergyClass, LatencyClass, ProviderTransport, TransportCapabilities, TransportKind,
    TransportPath,
};
use torca_wire::{
    EnvelopeId, FrameDecoder, FrameMetadata, MessageKind, ProtocolFamily, ProtocolVersion,
    VersionSupport, WireCodec, WireFlags, WireLimits,
};

const PEER_PROTOCOL_FAMILY: u16 = 2;
const PEER_PROTOCOL_MAJOR: u16 = 1;
const PEER_PROTOCOL_MINOR: u16 = 0;
const PEER_WIRE_MESSAGE_KIND: u16 = 1;
const PEER_WIRE_PAYLOAD_OVERHEAD: usize = 1024;
const PEER_WRITE_TIMEOUT: Duration = Duration::from_secs(10);

/// Embedded-Tor peer transport with strict `torca-wire` stream framing.
pub struct TorPeerTransport {
    client: Option<TorServiceHandle>,
    onion_address: String,
    port: u16,
    stream: Option<TcpStream>,
    received: VecDeque<Vec<u8>>,
    reader: Option<PeerReader>,
    wake: WakeSlot,
    next_wire_id: u128,
}

type WakeCallback = Arc<dyn Fn() + Send + Sync>;
type WakeSlot = Arc<Mutex<Option<WakeCallback>>>;
type ReadResult = Result<Vec<u8>, PeerTransportError>;

struct PeerReader {
    receiver: Receiver<ReadResult>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl PeerReader {
    fn stop(mut self, stream: Option<&TcpStream>) {
        self.stop.store(true, Ordering::Release);
        if let Some(stream) = stream {
            let _ = stream.shutdown(Shutdown::Both);
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
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
            received: VecDeque::new(),
            reader: None,
            wake: Arc::new(Mutex::new(None)),
            next_wire_id: 1,
        }
    }

    /// Wraps an already accepted loopback stream for the local onion-service listener.
    pub fn from_incoming_stream(stream: TcpStream) -> Result<Self, PeerTransportError> {
        stream
            .set_nodelay(true)
            .map_err(|error| io_error("configure incoming peer stream", &error))?;
        stream
            .set_write_timeout(Some(PEER_WRITE_TIMEOUT))
            .map_err(|error| io_error("configure incoming peer write timeout", &error))?;
        let reader_stream = stream
            .try_clone()
            .map_err(|error| io_error("clone incoming peer reader stream", &error))?;
        let mut transport = Self {
            client: None,
            onion_address: String::new(),
            port: 0,
            stream: Some(stream),
            received: VecDeque::new(),
            reader: None,
            wake: Arc::new(Mutex::new(None)),
            next_wire_id: 1,
        };
        transport.spawn_reader(reader_stream)?;
        Ok(transport)
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

    fn spawn_reader(&mut self, reader_stream: TcpStream) -> Result<(), PeerTransportError> {
        let (sender, receiver) = mpsc::sync_channel(256);
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let wake = Arc::clone(&self.wake);
        let thread = thread::Builder::new()
            .name("torca-peer-reader".to_owned())
            .spawn(move || peer_reader_loop(reader_stream, sender, thread_stop, wake))
            .map_err(|error| PeerTransportError(format!("spawn peer reader failed: {error}")))?;
        self.reader = Some(PeerReader { receiver, stop, thread: Some(thread) });
        Ok(())
    }

    fn stop_reader(&mut self) {
        if let Some(reader) = self.reader.take() {
            reader.stop(self.stream.as_ref());
        }
    }
}

impl PeerTransport for TorPeerTransport {
    fn connect(&mut self) -> Result<(), PeerTransportError> {
        if self.port == 0 {
            return Err(PeerTransportError("peer port is invalid".into()));
        }
        self.stop_reader();
        if let Some(stream) = self.stream.take() {
            let _ = stream.shutdown(Shutdown::Both);
        }
        self.received.clear();
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| PeerTransportError("Tor peer client is unavailable".into()))?;
        let stream = client
            .connect_onion(&self.onion_address, self.port)
            .map_err(|error| PeerTransportError(format!("Tor peer connect failed: {error}")))?;
        let reader_stream =
            stream.try_clone().map_err(|error| io_error("clone peer reader stream", &error))?;
        stream
            .set_write_timeout(Some(PEER_WRITE_TIMEOUT))
            .map_err(|error| io_error("configure peer write timeout", &error))?;
        self.stream = Some(stream);
        self.spawn_reader(reader_stream)?;
        Ok(())
    }

    fn send(&mut self, payload: Vec<u8>) -> Result<(), PeerTransportError> {
        let frame = self.encode_frame(&payload)?;
        let result = {
            let stream = self
                .stream
                .as_mut()
                .ok_or_else(|| PeerTransportError("peer transport is not connected".into()))?;
            stream
                .write_all(&frame)
                .map_err(|error| io_error("write peer frame", &error))
                .and_then(|()| stream.flush().map_err(|error| io_error("flush peer frame", &error)))
        };
        if result.is_err() {
            // A timed-out or failed write invalidates the stream. Do not keep
            // it as a connected transport for the next durable job.
            self.stop_reader();
            if let Some(stream) = self.stream.take() {
                let _ = stream.shutdown(Shutdown::Both);
            }
        }
        result
    }

    fn try_receive(&mut self) -> Result<Option<Vec<u8>>, PeerTransportError> {
        if let Some(payload) = self.received.pop_front() {
            return Ok(Some(payload));
        }
        let Some(reader) = self.reader.as_ref() else {
            return Err(PeerTransportError("peer transport is not connected".into()));
        };
        match reader.receiver.try_recv() {
            Ok(Ok(payload)) => Ok(Some(payload)),
            Ok(Err(error)) => Err(error),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => {
                Err(PeerTransportError("peer reader stopped".into()))
            }
        }
    }

    fn receive_timeout(
        &mut self,
        timeout: Duration,
    ) -> Result<Option<Vec<u8>>, PeerTransportError> {
        if let Some(payload) = self.received.pop_front() {
            return Ok(Some(payload));
        }
        let Some(reader) = self.reader.as_ref() else {
            return Err(PeerTransportError("peer transport is not connected".into()));
        };
        match reader.receiver.recv_timeout(timeout) {
            Ok(Ok(payload)) => Ok(Some(payload)),
            Ok(Err(error)) => Err(error),
            Err(RecvTimeoutError::Timeout) => Ok(None),
            Err(RecvTimeoutError::Disconnected) => {
                Err(PeerTransportError("peer reader stopped".into()))
            }
        }
    }

    fn close(&mut self) -> Result<(), PeerTransportError> {
        self.stop_reader();
        self.received.clear();
        if let Some(stream) = self.stream.take() {
            stream
                .shutdown(Shutdown::Both)
                .map_err(|error| io_error("close peer stream", &error))?;
        }
        Ok(())
    }

    fn set_waker(&mut self, waker: WakeCallback) {
        if let Ok(mut slot) = self.wake.lock() {
            *slot = Some(waker);
        }
    }
}

impl ProviderTransport for TorPeerTransport {
    fn kind(&self) -> TransportKind {
        TransportKind::Tor
    }

    fn path(&self) -> TransportPath {
        TransportPath::TorOnion
    }

    fn capabilities(&self) -> TransportCapabilities {
        TransportCapabilities {
            reliable: true,
            ordered: true,
            supports_incoming: true,
            supports_direct_path: false,
            supports_relay_path: true,
            hides_peer_ip: true,
            max_frame_size: MAX_PEER_DATA_LEN,
            latency: LatencyClass::High,
            energy: EnergyClass::High,
        }
    }
}

impl Drop for TorPeerTransport {
    fn drop(&mut self) {
        self.stop_reader();
        if let Some(stream) = self.stream.take() {
            let _ = stream.shutdown(Shutdown::Both);
        }
    }
}

fn peer_reader_loop(
    mut stream: TcpStream,
    sender: SyncSender<ReadResult>,
    stop: Arc<AtomicBool>,
    wake: WakeSlot,
) {
    let mut decoder = FrameDecoder::new(peer_wire_codec());
    let mut buffer = [0_u8; 16 * 1024];
    let expected_kind = peer_message_kind();
    while !stop.load(Ordering::Acquire) {
        let count = match stream.read(&mut buffer) {
            Ok(0) => {
                send_reader_error(&sender, "peer connection closed", &wake);
                return;
            }
            Ok(count) => count,
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(error) => {
                send_reader_error(&sender, &format!("read peer stream failed: {error}"), &wake);
                return;
            }
        };
        let frames = match decoder.push(&buffer[..count]) {
            Ok(frames) => frames,
            Err(error) => {
                send_reader_error(&sender, &format!("peer frame decode failed: {error}"), &wake);
                return;
            }
        };
        for frame in frames {
            if frame.metadata().message_kind() != expected_kind {
                send_reader_error(&sender, "unexpected peer wire message kind", &wake);
                return;
            }
            if sender.try_send(Ok(frame.into_payload())).is_err() {
                send_reader_error(&sender, "peer reader queue full", &wake);
                return;
            }
        }
        notify_waker(&wake);
    }
}

fn send_reader_error(sender: &SyncSender<ReadResult>, message: &str, wake: &WakeSlot) {
    let _ = sender.try_send(Err(PeerTransportError(message.to_owned())));
    notify_waker(wake);
}

fn notify_waker(wake: &WakeSlot) {
    if let Some(callback) = wake.lock().ok().and_then(|slot| slot.clone()) {
        callback();
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::sync::mpsc;
    use torca_foundation::{CorrelationId, OpaqueId};
    use torca_peer_protocol::{PeerCodec, PeerMessage};
    use torca_wire::{EnvelopeId, FrameMetadata, WireFlags};

    #[test]
    fn reader_wakes_and_delivers_framed_payload_without_polling() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test listener");
        let address = listener.local_addr().expect("listener address");
        let writer = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept test peer");
            let payload = PeerCodec::encode(&PeerMessage::Ping(7)).expect("encode peer payload");
            let id = OpaqueId::from_u128(1);
            let metadata = FrameMetadata::new(
                protocol_version(),
                peer_message_kind(),
                WireFlags::REQUIRED_KIND,
                EnvelopeId::from_opaque(id),
                CorrelationId::from_opaque(id),
            );
            let frame = peer_wire_codec().encode(metadata, &payload).expect("encode wire frame");
            stream.write_all(&frame).expect("write wire frame");
        });
        let client = TcpStream::connect(address).expect("connect test peer");
        let mut transport = TorPeerTransport::from_incoming_stream(client).expect("transport");
        let (wake_tx, wake_rx) = mpsc::channel();
        transport.set_waker(Arc::new(move || {
            let _ = wake_tx.send(());
        }));
        wake_rx.recv_timeout(Duration::from_secs(1)).expect("reader wake");
        let payload = transport
            .receive_timeout(Duration::from_secs(1))
            .expect("read payload")
            .expect("payload available");
        assert_eq!(PeerCodec::decode(&payload).expect("decode peer payload"), PeerMessage::Ping(7));
        writer.join().expect("writer thread");
    }
}
