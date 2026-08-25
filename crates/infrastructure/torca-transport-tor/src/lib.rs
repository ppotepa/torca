//! Tor implementation of the provider-neutral Torca transport contract.

use std::sync::Arc;
use std::time::Duration;

use torca_contacts::Contact;
use torca_tor::{PeerListener, TOR_PEER_VIRTUAL_PORT, TorPeerTransport, TorServiceHandle};
use torca_transport_api::{
    EnergyClass, LatencyClass, PeerTransport, PeerTransportError, PeerTransportFactory,
    ProviderTransport, TransportCapabilities, TransportFactoryError, TransportKind, TransportPath,
};

/// Provider adapter around the existing authenticated onion peer stream.
/// Framing and handshake remain owned by `torca-peer`; this type only exposes
/// the transport identity and capabilities required by the registry.
pub struct TorTransport {
    inner: TorPeerTransport,
}

impl TorTransport {
    pub fn new(client: TorServiceHandle, onion_address: impl Into<String>, port: u16) -> Self {
        Self { inner: TorPeerTransport::new(client, onion_address, port) }
    }

    pub fn from_incoming_stream(stream: std::net::TcpStream) -> Result<Self, PeerTransportError> {
        Ok(Self { inner: TorPeerTransport::from_incoming_stream(stream)? })
    }
}

impl PeerTransport for TorTransport {
    fn connect(&mut self) -> Result<(), PeerTransportError> {
        self.inner.connect()
    }

    fn send(&mut self, payload: Vec<u8>) -> Result<(), PeerTransportError> {
        self.inner.send(payload)
    }

    fn try_receive(&mut self) -> Result<Option<Vec<u8>>, PeerTransportError> {
        self.inner.try_receive()
    }

    fn receive_timeout(
        &mut self,
        timeout: Duration,
    ) -> Result<Option<Vec<u8>>, PeerTransportError> {
        self.inner.receive_timeout(timeout)
    }

    fn close(&mut self) -> Result<(), PeerTransportError> {
        self.inner.close()
    }

    fn set_waker(&mut self, waker: Arc<dyn Fn() + Send + Sync>) {
        self.inner.set_waker(waker);
    }
}

impl ProviderTransport for TorTransport {
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
            max_frame_size: torca_peer_protocol::MAX_PEER_DATA_LEN,
            latency: LatencyClass::High,
            energy: EnergyClass::High,
        }
    }
}

/// Tor-owned factory for authenticated onion peer streams.
///
/// Keeping this factory in the provider crate ensures that the peer protocol
/// has no dependency on Arti or onion address types.
pub struct TorPeerTransportFactory {
    listener: PeerListener,
    tor_client: TorServiceHandle,
}

impl TorPeerTransportFactory {
    pub fn new(listener: PeerListener, tor_client: TorServiceHandle) -> Self {
        Self { listener, tor_client }
    }
}

impl PeerTransportFactory for TorPeerTransportFactory {
    fn kind(&self) -> TransportKind {
        TransportKind::Tor
    }

    fn capabilities(&self) -> TransportCapabilities {
        TorTransport::new(self.tor_client.clone(), String::new(), TOR_PEER_VIRTUAL_PORT)
            .capabilities()
    }

    fn accept(&mut self) -> Result<Option<Box<dyn PeerTransport + Send>>, TransportFactoryError> {
        self.listener.try_accept_transport().map_err(|_| TransportFactoryError::Listener).map(
            |transport| {
                transport.map(|transport| Box::new(transport) as Box<dyn PeerTransport + Send>)
            },
        )
    }

    fn connect(
        &mut self,
        contact: &Contact,
    ) -> Result<Box<dyn PeerTransport + Send>, TransportFactoryError> {
        let endpoint = contact
            .route()
            .provider_endpoint("tor")
            .ok_or(TransportFactoryError::ContactNotFound)?;
        let onion_address =
            std::str::from_utf8(endpoint).map_err(|_| TransportFactoryError::Protocol)?;
        Ok(Box::new(TorPeerTransport::new(
            self.tor_client.clone(),
            onion_address,
            TOR_PEER_VIRTUAL_PORT,
        )))
    }

    fn set_waker(&self, waker: Arc<dyn Fn() + Send + Sync>) -> Result<(), TransportFactoryError> {
        self.listener.set_waker(waker).map_err(|_| TransportFactoryError::Listener)
    }
}
