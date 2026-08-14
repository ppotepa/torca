use crate::TorServiceHandle;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
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
const READER_QUEUE_CAPACITY: usize = 256;

type PeerWake = Arc<Mutex<Option<Arc<dyn Fn() + Send + Sync>>>>;

enum ReaderEvent {
    Payload(Vec<u8>),
    Failed(String),
}

struct PeerReader {
    receiver: Receiver<ReaderEvent>,
    worker: Option<JoinHandle<()>>,
}

impl PeerReader {
    fn spawn(stream: &TcpStream, wake: PeerWake) -> Result<Self, PeerTransportError> {
        let reader = stream
            .try_clone()
            .map_err(|error| io_error("clone peer read stream", &error))?;
        let (sender, receiver) = mpsc::sync_channel(READER_QUEUE_CAPACITY);
        let worker = thread::Builder::new()
            .name("torca-peer-read".into())
            .spawn(move || reader_loop(reader, sender, wake))
            .map_err(|error| PeerTransportError(format!("start peer reader failed: {error}")))?;
        Ok(Self { receiver, worker: Some(worker) })
    }

    fn try_receive(&self) -> Result<Option<Vec<u8>>, PeerTransportError> {
        match self.receiver.try_recv() {
            Ok(ReaderEvent::Payload(payload)) => Ok(Some(payload)),
            Ok(ReaderEvent::Failed(error)) => Err(PeerTransportError(error)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => {
                Err(PeerTransportError("peer read worker stopped".into()))
            }
        }
    }

    fn join(mut self) {
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

/// Embedded-Tor peer transport with strict `torca-wire` stream framing.
///
/// The read side is owned by one blocking worker. Incoming bytes therefore
/// wake the runtime immediately instead of requiring a periodic poll of every
/// ready peer session. The write side remains synchronous and process-owned.
pub struct TorPeerTransport {
    client: Option<TorServiceHandle>,
    onion_address: String,
    port: u16,
    stream: Option<TcpStream>,
    reader: Option<PeerReader>,
    wake: PeerWake,
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
        let wake = Arc::new(Mutex::new(None));
        let reader = PeerReader::spawn(&stream, Arc::clone(&wake))?;
        Ok(Self {
            client: None,
            onion_address: String::new(),
            port: 0,
            stream: Some(stream),
            reader: Some(reader),
            wake,
            next_wire_id: 1,
        })
    }

    /// Installs or replaces the runtime callback used by the blocking reader.
    /// A shared slot lets incoming transports start reading immediately and
    /// receive the process waker later when PeerLink accepts them.
    pub fn set_waker(&self, waker: Arc<dyn Fn() + Send + Sync>) -> Result<(), PeerTransportError> {
        let mut slot = self
            .wake
            .lock()
            .map_err(|_| PeerTransportError("peer wake slot poisoned".into()))?;
        *slot = Some(waker);
        Ok(())
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

    fn stop_io(&mut self) -> Result<(), PeerTransportError> {
        let shutdown_result = self
            .stream
            .take()
            .map(|stream| stream.shutdown(Shutdown::Both))
            .transpose()
            .map_err(|error| io_error("close peer stream", &error));
        if let Some(reader) = self.reader.take() {
            reader.join();
        }
        shutdown_result.map(|_| ())
    }
}

impl PeerTransport for TorPeerTransport {
    fn connect(&mut self) -> Result<(), PeerTransportError> {
        if self.port == 0 {
            return Err(PeerTransportError("peer port is invalid".into()));
        }
        let _ = self.stop_io();
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| PeerTransportError("Tor peer client is unavailable".into()))?;
        let stream = client
            .connect_onion(&self.onion_address, self.port)
            .map_err(|error| PeerTransportError(format!("Tor peer connect failed: {error}")))?;
        stream
            .set_nodelay(true)
            .map_err(|error| io_error("configure outgoing peer stream", &error))?;
        let reader = PeerReader::spawn(&stream, Arc::clone(&self.wake))?;
        self.stream = Some(stream);
        self.reader = Some(reader);
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
        self.reader
            .as_ref()
            .ok_or_else(|| PeerTransportError("peer transport is not connected".into()))?
            .try_receive()
    }

    fn close(&mut self) -> Result<(), PeerTransportError> {
        self.stop_io()
    }
}

impl Drop for TorPeerTransport {
    fn drop(&mut self) {
        let _ = self.stop_io();
    }
}

fn reader_loop(mut stream: TcpStream, sender: SyncSender<ReaderEvent>, wake: PeerWake) {
    let mut decoder = FrameDecoder::new(peer_wire_codec());
    let expected_kind = peer_message_kind();
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let count = match stream.read(&mut buffer) {
            Ok(0) => {
                publish_reader_failure(&sender, &wake, "peer connection closed");
                return;
            }
            Ok(count) => count,
            Err(error) => {
                publish_reader_failure(
                    &sender,
                    &wake,
                    &format!("read peer stream failed ({:?})", error.kind()),
                );
                return;
            }
        };
        let frames = match decoder.push(&buffer[..count]) {
            Ok(frames) => frames,
            Err(error) => {
                publish_reader_failure(
                    &sender,
                    &wake,
                    &format!("peer frame decode failed: {error}"),
                );
                return;
            }
        };
        let mut published = false;
        for frame in frames {
            if frame.metadata().message_kind() != expected_kind {
                publish_reader_failure(&sender, &wake, "unexpected peer wire message kind");
                return;
            }
            match sender.try_send(ReaderEvent::Payload(frame.into_payload())) {
                Ok(()) => published = true,
                Err(TrySendError::Full(_)) => {
                    publish_reader_failure(&sender, &wake, "peer read queue full");
                    return;
                }
                Err(TrySendError::Disconnected(_)) => return,
            }
        }
        if published {
            notify_waker(&wake);
        }
    }
}

fn publish_reader_failure(sender: &SyncSender<ReaderEvent>, wake: &PeerWake, error: &str) {
    let _ = sender.try_send(ReaderEvent::Failed(error.to_owned()));
    notify_waker(wake);
}

fn notify_waker(wake: &PeerWake) {
    let callback = wake.lock().ok().and_then(|slot| slot.clone());
    if let Some(callback) = callback {
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
    use std::time::Duration;

    #[test]
    fn blocking_reader_wakes_on_existing_stream_data() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let address = listener.local_addr().expect("address");
        let client = TcpStream::connect(address).expect("client");
        let (server, _) = listener.accept().expect("server");
        let mut sender = TorPeerTransport::from_incoming_stream(client).expect("sender");
        let mut receiver = TorPeerTransport::from_incoming_stream(server).expect("receiver");
        let (wake_tx, wake_rx) = mpsc::channel();
        receiver
            .set_waker(Arc::new(move || {
                let _ = wake_tx.send(());
            }))
            .expect("waker");

        sender.send(b"hello".to_vec()).expect("send");
        wake_rx.recv_timeout(Duration::from_secs(1)).expect("reader wake");
        assert_eq!(receiver.try_receive().expect("receive"), Some(b"hello".to_vec()));
        sender.close().expect("sender close");
        let _ = receiver.close();
    }
}
