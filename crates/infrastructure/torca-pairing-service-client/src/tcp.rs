use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

use torca_pairing_service_protocol::{PairingServiceRequest, PairingServiceResponse};

use crate::PairingServiceTransportError;
use crate::stream::{before_send, exchange_stream};

pub struct TcpPairingServiceTransport {
    endpoint: SocketAddr,
    connect_timeout: Duration,
    stream: Option<TcpStream>,
}

impl TcpPairingServiceTransport {
    pub const fn new(endpoint: SocketAddr, connect_timeout: Duration) -> Self {
        Self { endpoint, connect_timeout, stream: None }
    }

    pub const fn endpoint(&self) -> SocketAddr {
        self.endpoint
    }

    pub fn disconnect(&mut self) {
        self.stream = None;
    }
}

impl crate::PairingServiceTransport for TcpPairingServiceTransport {
    fn invalidate(&mut self) {
        self.disconnect();
    }

    fn reconnect(&mut self) -> Result<(), PairingServiceTransportError> {
        self.stream = None;
        let stream = TcpStream::connect_timeout(&self.endpoint, self.connect_timeout)
            .map_err(|error| before_send(error.kind()))?;
        stream.set_nodelay(true).map_err(|error| before_send(error.kind()))?;
        self.stream = Some(stream);
        Ok(())
    }

    fn exchange(
        &mut self,
        request: &PairingServiceRequest,
        timeout: Duration,
    ) -> Result<PairingServiceResponse, PairingServiceTransportError> {
        let stream =
            self.stream.as_mut().ok_or_else(|| before_send(std::io::ErrorKind::NotConnected))?;
        exchange_stream(stream, request, timeout)
    }
}
