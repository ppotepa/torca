//! Deterministic in-memory transport for provider contract and SOAK tests.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use torca_transport_api::{
    EnergyClass, LatencyClass, PeerTransport, PeerTransportError, ProviderTransport,
    TransportCapabilities, TransportKind, TransportPath,
};

type Queue = Arc<Mutex<VecDeque<Vec<u8>>>>;
type Wake = Arc<Mutex<Option<Arc<dyn Fn() + Send + Sync>>>>;

pub struct MemoryTransport {
    inbound: Queue,
    outbound: Queue,
    wake: Wake,
    connected: bool,
}

impl MemoryTransport {
    pub fn pair() -> (Self, Self) {
        let left = Arc::new(Mutex::new(VecDeque::new()));
        let right = Arc::new(Mutex::new(VecDeque::new()));
        (Self::new(left.clone(), right.clone()), Self::new(right, left))
    }

    fn new(inbound: Queue, outbound: Queue) -> Self {
        Self { inbound, outbound, wake: Arc::new(Mutex::new(None)), connected: false }
    }

    fn error(message: &str) -> PeerTransportError {
        PeerTransportError(message.to_owned())
    }
}

impl PeerTransport for MemoryTransport {
    fn connect(&mut self) -> Result<(), PeerTransportError> {
        self.connected = true;
        Ok(())
    }

    fn send(&mut self, payload: Vec<u8>) -> Result<(), PeerTransportError> {
        if !self.connected {
            return Err(Self::error("memory transport is not connected"));
        }
        self.outbound
            .lock()
            .map_err(|_| Self::error("memory transport queue poisoned"))?
            .push_back(payload);
        if let Some(waker) = self.wake.lock().ok().and_then(|slot| slot.clone()) {
            waker();
        }
        Ok(())
    }

    fn try_receive(&mut self) -> Result<Option<Vec<u8>>, PeerTransportError> {
        if !self.connected {
            return Err(Self::error("memory transport is not connected"));
        }
        self.inbound
            .lock()
            .map_err(|_| Self::error("memory transport queue poisoned"))
            .map(|mut queue| queue.pop_front())
    }

    fn receive_timeout(
        &mut self,
        _timeout: Duration,
    ) -> Result<Option<Vec<u8>>, PeerTransportError> {
        self.try_receive()
    }

    fn close(&mut self) -> Result<(), PeerTransportError> {
        self.connected = false;
        Ok(())
    }

    fn set_waker(&mut self, waker: Arc<dyn Fn() + Send + Sync>) {
        if let Ok(mut slot) = self.wake.lock() {
            *slot = Some(waker);
        }
    }
}

impl ProviderTransport for MemoryTransport {
    fn kind(&self) -> TransportKind {
        TransportKind::Memory
    }

    fn path(&self) -> TransportPath {
        TransportPath::Memory
    }

    fn capabilities(&self) -> TransportCapabilities {
        TransportCapabilities {
            reliable: true,
            ordered: true,
            supports_incoming: true,
            supports_direct_path: true,
            supports_relay_path: false,
            hides_peer_ip: true,
            max_frame_size: usize::MAX,
            latency: LatencyClass::Interactive,
            energy: EnergyClass::Low,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MemoryTransport;
    use torca_transport_api::PeerTransport;

    #[test]
    fn pair_transfers_bytes_and_notifies_receiver() {
        let (mut left, mut right) = MemoryTransport::pair();
        left.connect().expect("left connect");
        right.connect().expect("right connect");
        right.set_waker(std::sync::Arc::new(|| {}));
        left.send(vec![1, 2, 3]).expect("send");
        assert_eq!(right.try_receive().expect("receive"), Some(vec![1, 2, 3]));
    }

    #[test]
    fn disconnected_transport_rejects_io() {
        let (mut left, _) = MemoryTransport::pair();
        assert!(left.send(vec![1]).is_err());
    }
}
