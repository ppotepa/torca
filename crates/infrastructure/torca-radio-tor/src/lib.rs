//! Tor/onion implementation of the provider-neutral Radio media boundary.
//!
//! The common Radio worker deliberately has no dependency on Tor. This crate
//! adapts Tor's local onion virtual port to the `RadioMediaSystemFactory`
//! contract; Iroh, WebRTC and test providers live in their own crates.

use std::io::{Read, Write};
use std::net::{Shutdown, SocketAddr, TcpStream};
use std::sync::Arc;
use std::time::Duration;

use torca_radio_adapters::{
    RadioMediaConnector, RadioMediaDirectory, RadioMediaRoute, RadioMediaStream, RadioMediaSystem,
    RadioMediaSystemFactory,
};
use torca_radio_coordinator::RadioApplicationError;
use torca_tor::{PeerListener, TOR_RADIO_VIRTUAL_PORT, TorServiceHandle};

/// Provider-owned Tor Radio media factory.
pub struct TorRadioMediaSystemFactory {
    tor: TorServiceHandle,
}

struct TorRadioMediaStream(TcpStream);

impl Read for TorRadioMediaStream {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buffer)
    }
}

impl Write for TorRadioMediaStream {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.write(bytes)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}

impl RadioMediaStream for TorRadioMediaStream {
    fn configure(&self, read: Duration, write: Duration) -> Result<(), RadioApplicationError> {
        self.0
            .set_read_timeout(Some(read))
            .and_then(|_| self.0.set_write_timeout(Some(write)))
            .and_then(|_| self.0.set_nodelay(true))
            .map_err(|_| RadioApplicationError::MediaTransport)
    }

    fn close_stream(&self) -> Result<(), RadioApplicationError> {
        self.0.shutdown(Shutdown::Both).map_err(|_| RadioApplicationError::MediaTransport)
    }

    fn set_read_deadline(&self, timeout: Duration) -> Result<(), RadioApplicationError> {
        self.0.set_read_timeout(Some(timeout)).map_err(|_| RadioApplicationError::MediaTransport)
    }
}

struct TorRadioMediaConnector {
    tor: TorServiceHandle,
    listener: PeerListener,
}

impl RadioMediaConnector for TorRadioMediaConnector {
    fn set_incoming_waker(&mut self, waker: Arc<dyn Fn() + Send + Sync>) {
        // PeerListener owns a blocking accept thread and can wake the common
        // media worker as soon as an onion stream is queued. This keeps Tor
        // on the same event-driven contract as Iroh instead of relying on
        // the worker's one-second defensive polling fallback.
        let _ = self.listener.set_waker(waker);
    }

    fn connect(
        &mut self,
        route: &RadioMediaRoute,
        timeout: Duration,
    ) -> Result<Box<dyn RadioMediaStream>, RadioApplicationError> {
        if route.provider != "tor" {
            return Err(RadioApplicationError::MediaEndpointUnavailable);
        }
        let onion = std::str::from_utf8(&route.endpoint)
            .map_err(|_| RadioApplicationError::MediaEndpointUnavailable)?;
        self.tor
            .connect_onion_with_timeout(onion, TOR_RADIO_VIRTUAL_PORT, timeout)
            .map(|stream| Box::new(TorRadioMediaStream(stream)) as Box<dyn RadioMediaStream>)
            .map_err(|_| RadioApplicationError::MediaTransport)
    }

    fn try_accept(&mut self) -> Result<Option<Box<dyn RadioMediaStream>>, RadioApplicationError> {
        self.listener.try_accept().map_err(|_| RadioApplicationError::MediaTransport).map(
            |stream| {
                stream.map(|(stream, _)| {
                    Box::new(TorRadioMediaStream(stream)) as Box<dyn RadioMediaStream>
                })
            },
        )
    }
}

impl TorRadioMediaSystemFactory {
    #[must_use]
    pub const fn new(tor: TorServiceHandle) -> Self {
        Self { tor }
    }
}

impl RadioMediaSystemFactory for TorRadioMediaSystemFactory {
    fn start(
        self: Box<Self>,
        directory: Box<dyn RadioMediaDirectory>,
    ) -> Result<RadioMediaSystem, RadioApplicationError> {
        let listener = PeerListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
            .map_err(|_| RadioApplicationError::MediaTransport)?;
        self.tor
            .register_onion_route(TOR_RADIO_VIRTUAL_PORT, listener.local_addr())
            .map_err(|_| RadioApplicationError::MediaTransport)?;
        let connector = TorRadioMediaConnector { tor: self.tor, listener };
        RadioMediaSystem::start_with_connector(Box::new(connector), directory)
    }
}
