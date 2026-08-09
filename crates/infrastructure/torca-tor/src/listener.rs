use std::net::{SocketAddr, TcpListener, TcpStream};

use crate::{TorPeerTransport, TransportError};
use torca_peer::PeerTransportError;

/// Loopback-only TCP listener targeted by the owned Tor onion service.
pub struct PeerListener {
    listener: TcpListener,
    local_addr: SocketAddr,
}

impl PeerListener {
    /// Binds a loopback peer endpoint. Port zero asks the OS for a free port.
    pub fn bind(address: SocketAddr) -> Result<Self, TransportError> {
        if !address.ip().is_loopback() {
            return Err(TransportError::InvalidState);
        }
        let listener = TcpListener::bind(address)?;
        listener.set_nonblocking(true)?;
        let local_addr = listener.local_addr()?;
        Ok(Self { listener, local_addr })
    }

    pub const fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Accepts at most one pending connection without blocking the runtime supervisor.
    pub fn try_accept(&self) -> Result<Option<(TcpStream, SocketAddr)>, TransportError> {
        match self.listener.accept() {
            Ok((stream, remote)) => {
                stream.set_nodelay(true)?;
                Ok(Some((stream, remote)))
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
            Err(error) => Err(TransportError::Io(error)),
        }
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
