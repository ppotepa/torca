use core::fmt;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use torca_foundation::Timestamp;
use torca_relay_protocol::{RELAY_HEADER_LEN, RelayCodec, RelayCodecError, RelayResponse};

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
    /// A peer sent an invalid or incompatible relay wire frame. The codec
    /// error is safe to log: it contains only framing/version information,
    /// never a pairing code, token, payload or endpoint.
    Codec(RelayCodecError),
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
    requests: Arc<AtomicUsize>,
    failures: Arc<AtomicUsize>,
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
            requests: Arc::new(AtomicUsize::new(0)),
            failures: Arc::new(AtomicUsize::new(0)),
        })
    }

    pub fn run(self) -> Result<(), RelayServerError> {
        let cleanup_broker = Arc::clone(&self.broker);
        let cleanup_interval = self.cleanup_interval;
        let cleanup_active = Arc::clone(&self.active_connections);
        let cleanup_requests = Arc::clone(&self.requests);
        let cleanup_failures = Arc::clone(&self.failures);
        let _cleanup_worker = thread::Builder::new()
            .name("torca-relay-expiry".into())
            .spawn(move || {
                expiry_loop(
                    cleanup_broker,
                    cleanup_interval,
                    cleanup_active,
                    cleanup_requests,
                    cleanup_failures,
                );
            })
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
            let requests = Arc::clone(&self.requests);
            let failures = Arc::clone(&self.failures);
            let spawn = thread::Builder::new().name("torca-relay-client".into()).spawn(move || {
                let _permit = ConnectionPermit { active };
                if let Err(error) =
                    serve_connection(stream, broker, timeout, requests, failures.as_ref())
                {
                    failures.fetch_add(1, Ordering::Relaxed);
                    eprintln!("torca-relay: connection closed error={error}");
                }
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

#[allow(clippy::needless_pass_by_value)]
fn expiry_loop(
    broker: Arc<Mutex<RelayBroker>>,
    interval: Duration,
    active: Arc<AtomicUsize>,
    requests: Arc<AtomicUsize>,
    failures: Arc<AtomicUsize>,
) {
    let mut ticks = 0_u32;
    loop {
        thread::sleep(interval);
        let Ok(now) = system_timestamp() else {
            continue;
        };
        let Ok(mut broker) = broker.lock() else {
            return;
        };
        let expired = broker.expire(now);
        ticks = ticks.saturating_add(1);
        if ticks >= 12 || expired != 0 {
            ticks = 0;
            eprintln!(
                "torca-relay: stats active_connections={} active_slots={} requests={} failures={} expired_slots={expired}",
                active.load(Ordering::Relaxed),
                broker.active_slots(),
                requests.load(Ordering::Relaxed),
                failures.load(Ordering::Relaxed),
            );
        }
    }
}

#[allow(clippy::needless_pass_by_value)]
fn serve_connection(
    mut stream: TcpStream,
    broker: Arc<Mutex<RelayBroker>>,
    timeout: Duration,
    requests: Arc<AtomicUsize>,
    failures: &AtomicUsize,
) -> Result<(), RelayServerError> {
    stream.set_read_timeout(Some(timeout)).map_err(io_error)?;
    stream.set_write_timeout(Some(timeout)).map_err(io_error)?;
    stream.set_nodelay(true).map_err(io_error)?;

    loop {
        let Some(frame) = read_frame(&mut stream)? else {
            return Ok(());
        };
        let request = RelayCodec::decode_request(&frame).map_err(RelayServerError::Codec)?;
        requests.fetch_add(1, Ordering::Relaxed);
        let now = system_timestamp()?;
        let response = {
            let mut broker = broker.lock().map_err(|_| RelayServerError::BrokerPoisoned)?;
            match broker.handle(request, now) {
                Ok(response) => response,
                Err(error) => {
                    failures.fetch_add(1, Ordering::Relaxed);
                    RelayResponse::Error(error)
                }
            }
        };
        let encoded = RelayCodec::encode_response(&response).map_err(RelayServerError::Codec)?;
        stream.write_all(&encoded).map_err(io_error)?;
        stream.flush().map_err(io_error)?;
    }
}

fn read_frame(stream: &mut TcpStream) -> Result<Option<Vec<u8>>, RelayServerError> {
    let mut header = [0_u8; RELAY_HEADER_LEN];
    match stream.read_exact(&mut header) {
        Ok(()) => {}
        // A rendezvous client intentionally keeps a Tor stream between polls.
        // Its adaptive backoff reaches 30 seconds, so treating the configured
        // read deadline as a transport *failure* created needless reconnects
        // and misleading relay failure counters. Closing an idle stream is
        // still a bounded resource policy; it is simply a normal close.
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::UnexpectedEof
                    | std::io::ErrorKind::TimedOut
                    | std::io::ErrorKind::WouldBlock
            ) =>
        {
            return Ok(None);
        }
        Err(error) if matches!(error.kind(), std::io::ErrorKind::ConnectionReset) => {
            return Ok(None);
        }
        Err(error) => return Err(io_error(error)),
    }
    let frame_len = RelayCodec::frame_len_from_header(&header).map_err(RelayServerError::Codec)?;
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
    let elapsed =
        SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| RelayServerError::Clock)?;
    let millis = i64::try_from(elapsed.as_millis()).map_err(|_| RelayServerError::Clock)?;
    Timestamp::from_unix_millis(millis).map_err(|_| RelayServerError::Clock)
}

#[allow(clippy::needless_pass_by_value)]
fn io_error(error: std::io::Error) -> RelayServerError {
    RelayServerError::Io(error.kind())
}

#[cfg(test)]
mod tests {
    use super::read_frame;
    use std::net::{TcpListener, TcpStream};
    use std::time::Duration;

    #[test]
    fn idle_stream_timeout_is_a_normal_close() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let address = listener.local_addr().expect("address");
        let client = TcpStream::connect(address).expect("client");
        let (mut server, _) = listener.accept().expect("accept");
        server.set_read_timeout(Some(Duration::from_millis(20))).expect("timeout");
        assert_eq!(read_frame(&mut server).expect("idle close"), None);
        drop(client);
    }
}
