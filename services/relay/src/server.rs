use core::fmt;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use torca_foundation::Timestamp;
use torca_relay_protocol::{RELAY_HEADER_LEN, RelayCodec, RelayResponse};

use crate::{DEFAULT_MAX_ACTIVE_SLOTS, RelayBroker};

pub const DEFAULT_MAX_CONNECTIONS: usize = 1024;
pub const DEFAULT_CLEANUP_INTERVAL: Duration = Duration::from_secs(5);

#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RelayServerConfig {
    pub bind: SocketAddr,
    pub io_timeout: Duration,
    pub cleanup_interval: Duration,
    pub max_connections: usize,
    pub max_slots: usize,
}

impl RelayServerConfig {
    pub const fn new(bind: SocketAddr, io_timeout: Duration) -> Self {
        Self {
            bind,
            io_timeout,
            cleanup_interval: DEFAULT_CLEANUP_INTERVAL,
            max_connections: DEFAULT_MAX_CONNECTIONS,
            max_slots: DEFAULT_MAX_ACTIVE_SLOTS,
        }
    }

    pub fn with_limits(mut self, max_connections: usize, max_slots: usize) -> Self {
        self.max_connections = max_connections.max(1);
        self.max_slots = max_slots.max(1);
        self
    }

    pub fn with_cleanup_interval(mut self, interval: Duration) -> Self {
        if !interval.is_zero() {
            self.cleanup_interval = interval;
        }
        self
    }
}

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

pub struct RelayServer {
    listener: TcpListener,
    broker: Arc<Mutex<RelayBroker>>,
    io_timeout: Duration,
    cleanup_interval: Duration,
    max_connections: usize,
    active_connections: Arc<AtomicUsize>,
}

impl RelayServer {
    pub fn bind(config: RelayServerConfig) -> Result<Self, RelayServerError> {
        let listener = TcpListener::bind(config.bind).map_err(io_error)?;
        Ok(Self {
            listener,
            broker: Arc::new(Mutex::new(RelayBroker::with_max_slots(config.max_slots))),
            io_timeout: config.io_timeout,
            cleanup_interval: config.cleanup_interval,
            max_connections: config.max_connections.max(1),
            active_connections: Arc::new(AtomicUsize::new(0)),
        })
    }

    pub fn run(self) -> Result<(), RelayServerError> {
        let cleanup_broker = Arc::clone(&self.broker);
        let cleanup_interval = self.cleanup_interval;
        let _cleanup_worker = thread::Builder::new()
            .name("torca-relay-expiry".into())
            .spawn(move || expiry_loop(cleanup_broker, cleanup_interval))
            .map_err(io_error)?;

        for connection in self.listener.incoming() {
            let stream = connection.map_err(io_error)?;
            if !try_acquire_connection(&self.active_connections, self.max_connections) {
                drop(stream);
                continue;
            }
            let broker = Arc::clone(&self.broker);
            let active = Arc::clone(&self.active_connections);
            let timeout = self.io_timeout;
            let spawn = thread::Builder::new()
                .name("torca-relay-client".into())
                .spawn(move || {
                    let _permit = ConnectionPermit { active };
                    let _ = serve_connection(stream, broker, timeout);
                });
            if spawn.is_err() {
                self.active_connections.fetch_sub(1, Ordering::AcqRel);
            }
        }
        Ok(())
    }

    pub fn broker(&self) -> Arc<Mutex<RelayBroker>> {
        Arc::clone(&self.broker)
    }

    pub fn active_connections(&self) -> usize {
        self.active_connections.load(Ordering::Acquire)
    }
}

struct ConnectionPermit {
    active: Arc<AtomicUsize>,
}
impl Drop for ConnectionPermit {
    fn drop(&mut self) {
        self.active.fetch_sub(1, Ordering::AcqRel);
    }
}

fn try_acquire_connection(active: &AtomicUsize, maximum: usize) -> bool {
    active
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
            (current < maximum).then_some(current + 1)
        })
        .is_ok()
}

fn expiry_loop(broker: Arc<Mutex<RelayBroker>>, interval: Duration) {
    loop {
        thread::sleep(interval);
        let Ok(now) = system_timestamp() else {
            continue;
        };
        let Ok(mut broker) = broker.lock() else {
            return;
        };
        let _ = broker.expire(now);
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

pub(crate) fn system_timestamp() -> Result<Timestamp, RelayServerError> {
    let elapsed = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| RelayServerError::Clock)?;
    let millis = i64::try_from(elapsed.as_millis()).map_err(|_| RelayServerError::Clock)?;
    Timestamp::from_unix_millis(millis).map_err(|_| RelayServerError::Clock)
}

fn io_error(error: std::io::Error) -> RelayServerError {
    RelayServerError::Io(error.kind())
}
