use core::fmt;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use torca_foundation::Timestamp;
use torca_relay_protocol::{RELAY_HEADER_LEN, RelayCodec, RelayResponse};

use crate::RelayBroker;

/// Minimal network configuration for the ephemeral relay process.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RelayServerConfig {
    pub bind: SocketAddr,
    pub io_timeout: Duration,
}

impl RelayServerConfig {
    pub const fn new(bind: SocketAddr, io_timeout: Duration) -> Self {
        Self { bind, io_timeout }
    }
}

/// Redaction-safe relay server failure.
#[derive(Debug)]
pub enum RelayServerError {
    Io(std::io::ErrorKind),
    Clock,
    BrokerPoisoned,
    Codec,
}
impl fmt::Display for RelayServerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for RelayServerError {}

/// Blocking TCP relay host. Each connection is independent; all product state remains in the
/// bounded in-memory [`RelayBroker`].
pub struct RelayServer {
    listener: TcpListener,
    broker: Arc<Mutex<RelayBroker>>,
    io_timeout: Duration,
}

impl RelayServer {
    /// Binds a relay server. No persistent storage is opened.
    pub fn bind(config: RelayServerConfig) -> Result<Self, RelayServerError> {
        let listener = TcpListener::bind(config.bind).map_err(io_error)?;
        Ok(Self {
            listener,
            broker: Arc::new(Mutex::new(RelayBroker::default())),
            io_timeout: config.io_timeout,
        })
    }

    /// Accepts connections until the process is terminated.
    pub fn run(self) -> Result<(), RelayServerError> {
        for connection in self.listener.incoming() {
            let stream = connection.map_err(io_error)?;
            let broker = Arc::clone(&self.broker);
            let timeout = self.io_timeout;
            let _ = thread::Builder::new()
                .name("torca-relay-client".into())
                .spawn(move || {
                    let _ = serve_connection(stream, broker, timeout);
                });
        }
        Ok(())
    }

    /// Returns the shared broker for health probes embedded in the same process.
    pub fn broker(&self) -> Arc<Mutex<RelayBroker>> {
        Arc::clone(&self.broker)
    }
}

fn serve_connection(
    mut stream: TcpStream,
    broker: Arc<Mutex<RelayBroker>>,
    timeout: Duration,
) -> Result<(), RelayServerError> {
    stream.set_read_timeout(Some(timeout)).map_err(io_error)?;
    stream.set_write_timeout(Some(timeout)).map_err(io_error)?;
    stream.set_nodelay(true).map_err(io_error)?;

    loop {
        let Some(frame) = read_frame(&mut stream)? else {
            return Ok(());
        };
        let request = RelayCodec::decode_request(&frame).map_err(|_| RelayServerError::Codec)?;
        let now = system_timestamp()?;
        let response = {
            let mut broker = broker.lock().map_err(|_| RelayServerError::BrokerPoisoned)?;
            match broker.handle(request, now) {
                Ok(response) => response,
                Err(error) => RelayResponse::Error(error),
            }
        };
        let encoded = RelayCodec::encode_response(&response).map_err(|_| RelayServerError::Codec)?;
        stream.write_all(&encoded).map_err(io_error)?;
        stream.flush().map_err(io_error)?;
    }
}

fn read_frame(stream: &mut TcpStream) -> Result<Option<Vec<u8>>, RelayServerError> {
    let mut header = [0_u8; RELAY_HEADER_LEN];
    match stream.read_exact(&mut header) {
        Ok(()) => {}
        Err(error) if matches!(error.kind(), std::io::ErrorKind::UnexpectedEof) => return Ok(None),
        Err(error) if matches!(error.kind(), std::io::ErrorKind::ConnectionReset) => return Ok(None),
        Err(error) => return Err(io_error(error)),
    }
    let frame_len = RelayCodec::frame_len_from_header(&header).map_err(|_| RelayServerError::Codec)?;
    let payload_len = frame_len - RELAY_HEADER_LEN;
    let mut frame = Vec::with_capacity(frame_len);
    frame.extend_from_slice(&header);
    if payload_len != 0 {
        let mut payload = vec![0_u8; payload_len];
        stream.read_exact(&mut payload).map_err(io_error)?;
        frame.extend_from_slice(&payload);
    }
    Ok(Some(frame))
}

fn system_timestamp() -> Result<Timestamp, RelayServerError> {
    let elapsed = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| RelayServerError::Clock)?;
    let millis = i64::try_from(elapsed.as_millis()).map_err(|_| RelayServerError::Clock)?;
    Timestamp::from_unix_millis(millis).map_err(|_| RelayServerError::Clock)
}

fn io_error(error: std::io::Error) -> RelayServerError {
    RelayServerError::Io(error.kind())
}
