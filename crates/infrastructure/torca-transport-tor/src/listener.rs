use std::net::{SocketAddr, TcpListener, TcpStream};

use crate::TorError;

/// Loopback-only TCP listener targeted by the owned Tor onion service.
pub struct PeerListener {
    listener: TcpListener,
    local_addr: SocketAddr,
}

impl PeerListener {
    /// Binds a local peer endpoint. Callers may pass port zero before constructing TorRuntimeConfig.
    pub fn bind(address: SocketAddr) -> Result<Self, TorError> {
        if !address.ip().is_loopback() {
            return Err(TorError::InvalidState);
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
    pub fn try_accept(&self) -> Result<Option<(TcpStream, SocketAddr)>, TorError> {
        match self.listener.accept() {
            Ok((stream, remote)) => {
                stream.set_nodelay(true)?;
                Ok(Some((stream, remote)))
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
            Err(error) => Err(TorError::Io(error)),
        }
    }
}
