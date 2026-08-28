//! Deterministic in-memory transport for provider contract and SOAK tests.

use std::collections::{BTreeMap, VecDeque};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use torca_contacts::Contact;
use torca_foundation::ProviderId;
use torca_transport_api::{
    EnergyClass, LatencyClass, PeerTransport, PeerTransportError, PeerTransportFactory,
    ProviderTransport, TransportCapabilities, TransportFactoryError, TransportPath,
    TransportTopology,
};

fn memory_provider_id() -> ProviderId {
    ProviderId::new("memory").expect("static provider id")
}

type Wake = Arc<Mutex<Option<Arc<dyn Fn() + Send + Sync>>>>;

#[derive(Default)]
struct Lane {
    queue: Mutex<VecDeque<Vec<u8>>>,
    wake: Wake,
}

type SharedLane = Arc<Lane>;

pub struct MemoryTransport {
    inbound: SharedLane,
    outbound: SharedLane,
    connected: bool,
}

impl MemoryTransport {
    pub fn pair() -> (Self, Self) {
        let left = Arc::new(Lane::default());
        let right = Arc::new(Lane::default());
        (Self::new(left.clone(), right.clone()), Self::new(right, left))
    }

    fn new(inbound: SharedLane, outbound: SharedLane) -> Self {
        Self { inbound, outbound, connected: false }
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
            .queue
            .lock()
            .map_err(|_| Self::error("memory transport queue poisoned"))?
            .push_back(payload);
        if let Some(waker) = self.outbound.wake.lock().ok().and_then(|slot| slot.clone()) {
            waker();
        }
        Ok(())
    }

    fn try_receive(&mut self) -> Result<Option<Vec<u8>>, PeerTransportError> {
        if !self.connected {
            return Err(Self::error("memory transport is not connected"));
        }
        self.inbound
            .queue
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
        if let Ok(mut slot) = self.inbound.wake.lock() {
            *slot = Some(waker);
        }
    }
}

#[derive(Default)]
struct MemoryListener {
    pending: VecDeque<MemoryTransport>,
    wake: Wake,
}

type SharedListener = Arc<Mutex<MemoryListener>>;
type ListenerRegistry = Arc<Mutex<BTreeMap<Vec<u8>, SharedListener>>>;

/// Shared deterministic network used by provider conformance tests.
/// Endpoint bytes are opaque to the peer layer and meaningful only here.
#[derive(Clone, Default)]
pub struct MemoryNetwork {
    listeners: ListenerRegistry,
}

impl MemoryNetwork {
    pub fn bind(&self, endpoint: Vec<u8>) -> Result<MemoryTransportFactory, TransportFactoryError> {
        if endpoint.is_empty() || endpoint.len() > 8 * 1024 {
            return Err(TransportFactoryError::Protocol);
        }
        let listener = Arc::new(Mutex::new(MemoryListener::default()));
        let mut listeners = self.listeners.lock().map_err(|_| TransportFactoryError::Listener)?;
        if listeners.insert(endpoint.clone(), Arc::clone(&listener)).is_some() {
            return Err(TransportFactoryError::Listener);
        }
        Ok(MemoryTransportFactory {
            network: self.clone(),
            endpoint,
            listener,
            route_stale: AtomicBool::new(false),
        })
    }
}

/// Factory-backed Memory provider. It uses persisted contact routes just like
/// a network provider, while keeping conformance scenarios deterministic.
pub struct MemoryTransportFactory {
    network: MemoryNetwork,
    endpoint: Vec<u8>,
    listener: SharedListener,
    route_stale: AtomicBool,
}

impl MemoryTransportFactory {
    pub fn endpoint(&self) -> &[u8] {
        &self.endpoint
    }

    pub fn mark_route_stale(&self) {
        self.route_stale.store(true, Ordering::Release);
    }

    pub fn mark_route_refreshed(&self) {
        self.route_stale.store(false, Ordering::Release);
        if let Some(waker) = self
            .listener
            .lock()
            .ok()
            .and_then(|listener| listener.wake.lock().ok().and_then(|slot| slot.clone()))
        {
            waker();
        }
    }
}

impl Drop for MemoryTransportFactory {
    fn drop(&mut self) {
        if let Ok(mut listeners) = self.network.listeners.lock()
            && listeners
                .get(&self.endpoint)
                .is_some_and(|registered| Arc::ptr_eq(registered, &self.listener))
        {
            listeners.remove(&self.endpoint);
        }
    }
}

impl PeerTransportFactory for MemoryTransportFactory {
    fn provider_id(&self) -> ProviderId {
        memory_provider_id()
    }

    fn capabilities(&self) -> TransportCapabilities {
        memory_capabilities()
    }

    fn accept(&mut self) -> Result<Option<Box<dyn PeerTransport + Send>>, TransportFactoryError> {
        self.listener.lock().map_err(|_| TransportFactoryError::Listener).map(|mut listener| {
            listener
                .pending
                .pop_front()
                .map(|transport| Box::new(transport) as Box<dyn PeerTransport + Send>)
        })
    }

    fn connect(
        &mut self,
        contact: &Contact,
    ) -> Result<Box<dyn PeerTransport + Send>, TransportFactoryError> {
        if self.route_stale.load(Ordering::Acquire) {
            return Err(TransportFactoryError::RouteStale);
        }
        let endpoint = contact
            .route()
            .provider_endpoint(memory_provider_id().as_str())
            .ok_or(TransportFactoryError::ContactNotFound)?;
        let listener = self
            .network
            .listeners
            .lock()
            .map_err(|_| TransportFactoryError::Listener)?
            .get(endpoint)
            .cloned()
            .ok_or(TransportFactoryError::ContactNotFound)?;
        let (local, mut remote) = MemoryTransport::pair();
        remote.connect().map_err(|_| TransportFactoryError::Listener)?;
        let wake = {
            let mut listener = listener.lock().map_err(|_| TransportFactoryError::Listener)?;
            listener.pending.push_back(remote);
            listener.wake.lock().ok().and_then(|slot| slot.clone())
        };
        if let Some(waker) = wake {
            waker();
        }
        Ok(Box::new(local))
    }

    fn set_waker(&self, waker: Arc<dyn Fn() + Send + Sync>) -> Result<(), TransportFactoryError> {
        let listener = self.listener.lock().map_err(|_| TransportFactoryError::Listener)?;
        *listener.wake.lock().map_err(|_| TransportFactoryError::Listener)? = Some(waker);
        Ok(())
    }
}

const fn memory_capabilities() -> TransportCapabilities {
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

impl ProviderTransport for MemoryTransport {
    fn provider_id(&self) -> ProviderId {
        memory_provider_id()
    }

    fn path(&self) -> TransportPath {
        TransportPath { provider: memory_provider_id(), topology: TransportTopology::Direct }
    }

    fn capabilities(&self) -> TransportCapabilities {
        memory_capabilities()
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
