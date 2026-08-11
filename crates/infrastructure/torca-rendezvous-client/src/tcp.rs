use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

use torca_relay_protocol::{RelayRequest, RelayResponse};

use crate::stream::{before_send, exchange_stream};
use crate::{RelayTransport, RelayTransportError};

pub struct TcpRelayTransport {
    endpoint: SocketAddr,
    connect_timeout: Duration,
    stream: Option<TcpStream>,
}

impl TcpRelayTransport {
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

impl RelayTransport for TcpRelayTransport {
    fn invalidate(&mut self) {
        self.disconnect();
    }

    fn reconnect(&mut self) -> Result<(), RelayTransportError> {
        self.stream = None;
        let stream = TcpStream::connect_timeout(&self.endpoint, self.connect_timeout)
            .map_err(|error| before_send(error.kind()))?;
        stream.set_nodelay(true).map_err(|error| before_send(error.kind()))?;
        self.stream = Some(stream);
        Ok(())
    }

    fn exchange(
        &mut self,
        request: &RelayRequest,
        timeout: Duration,
    ) -> Result<RelayResponse, RelayTransportError> {
        let stream =
            self.stream.as_mut().ok_or_else(|| before_send(std::io::ErrorKind::NotConnected))?;
        exchange_stream(stream, request, timeout)
    }
}
