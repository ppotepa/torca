use std::collections::VecDeque;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::{TorPeerTransport, TransportError};
use torca_peer::PeerTransportError;

type WakeCallback = Arc<dyn Fn() + Send + Sync>;
type WakeSlot = Arc<Mutex<Option<WakeCallback>>>;

/// Loopback-only TCP listener targeted by the owned Tor onion service.
pub struct PeerListener {
    local_addr: SocketAddr,
    incoming: Arc<Mutex<VecDeque<(TcpStream, SocketAddr)>>>,
    wake: WakeSlot,
    stop: Arc<AtomicBool>,
    accept_thread: Option<JoinHandle<()>>,
}

const MAX_PENDING_CONNECTIONS: usize = 64;

impl PeerListener {
    /// Binds a loopback peer endpoint. Port zero asks the OS for a free port.
    pub fn bind(address: SocketAddr) -> Result<Self, TransportError> {
        if !address.ip().is_loopback() {
            return Err(TransportError::InvalidState);
        }
        let listener = TcpListener::bind(address)?;
        let local_addr = listener.local_addr()?;
        let worker_listener = listener.try_clone()?;
        let incoming = Arc::new(Mutex::new(VecDeque::new()));
        let wake = Arc::new(Mutex::new(None));
        let stop = Arc::new(AtomicBool::new(false));
        let worker_incoming = Arc::clone(&incoming);
        let worker_wake = Arc::clone(&wake);
        let worker_stop = Arc::clone(&stop);
        let accept_thread = thread::Builder::new()
            .name("torca-peer-accept".to_owned())
            .spawn(move || accept_loop(worker_listener, worker_incoming, worker_wake, worker_stop))
            .map_err(TransportError::Io)?;
        Ok(Self { local_addr, incoming, wake, stop, accept_thread: Some(accept_thread) })
    }

    pub const fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Installs the runtime wake callback used when an inbound stream is
    /// queued. The callback runs outside the listener lock.
    pub fn set_waker(&self, waker: Arc<dyn Fn() + Send + Sync>) -> Result<(), TransportError> {
        let mut slot = self.wake.lock().map_err(|_| TransportError::InvalidState)?;
        *slot = Some(waker);
        Ok(())
    }

    /// Accepts at most one pending connection without blocking the runtime supervisor.
    pub fn try_accept(&self) -> Result<Option<(TcpStream, SocketAddr)>, TransportError> {
        let mut incoming = self.incoming.lock().map_err(|_| TransportError::InvalidState)?;
        Ok(incoming.pop_front())
    }

    /// Accepts and wraps one incoming stream in the transport used by peer sessions.
    pub fn try_accept_transport(&self) -> Result<Option<TorPeerTransport>, TransportError> {
        self.try_accept()?
            .map(|(stream, _)| {
                TorPeerTransport::from_incoming_stream(stream)
                    .map_err(|_: PeerTransportError| TransportError::InvalidState)
            })
            .transpose()
    }
}

impl Drop for PeerListener {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        // Wake the blocking accept(2) call so the worker can observe stop
        // without a timer or a leaked thread.
        let _ = TcpStream::connect(self.local_addr);
        if let Some(thread) = self.accept_thread.take() {
            let _ = thread.join();
        }
    }
}

fn accept_loop(
    listener: TcpListener,
    incoming: Arc<Mutex<VecDeque<(TcpStream, SocketAddr)>>>,
    wake: WakeSlot,
    stop: Arc<AtomicBool>,
) {
    while !stop.load(Ordering::Acquire) {
        match listener.accept() {
            Ok((stream, remote)) => {
                if stop.load(Ordering::Acquire) {
                    return;
                }
                if stream.set_nodelay(true).is_err() {
                    continue;
                }
                let Ok(mut incoming) = incoming.lock() else {
                    return;
                };
                // Do not let an unavailable consumer retain unbounded socket
                // handles. Dropping the oldest pending connection keeps the
                // listener bounded while preserving the newest activity.
                if incoming.len() >= MAX_PENDING_CONNECTIONS {
                    incoming.pop_front();
                }
                incoming.push_back((stream, remote));
                let callback = wake.lock().ok().and_then(|slot| slot.clone());
                drop(incoming);
                if let Some(callback) = callback {
                    callback();
                }
            }
            Err(_) if stop.load(Ordering::Acquire) => return,
            Err(_) => thread::sleep(Duration::from_millis(100)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn listener_wakes_consumer_when_connection_is_queued() {
        let listener =
            PeerListener::bind("127.0.0.1:0".parse().expect("loopback address")).expect("listener");
        let (wake_tx, wake_rx) = mpsc::channel();
        listener
            .set_waker(Arc::new(move || {
                let _ = wake_tx.send(());
            }))
            .expect("waker");
        let _client = TcpStream::connect(listener.local_addr()).expect("connect");
        wake_rx.recv_timeout(Duration::from_secs(1)).expect("listener wake");
        assert!(listener.try_accept().expect("accept queue").is_some());
    }
}
