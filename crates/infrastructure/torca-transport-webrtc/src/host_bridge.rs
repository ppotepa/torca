//! Host-owned bridge used to connect a concrete WebRTC SDK to Torca.
//!
//! The bridge deliberately contains no SDP, ICE or TURN types. A platform
//! adapter translates its SDK callbacks into this small registry and passes
//! the same `Arc` as both `WebRtcSessionProvider` and
//! `WebRtcSignalingProvider` to native composition.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use torca_contacts::{Contact, ContactId};
use torca_transport_api::{
    CommissioningState, ProviderCommissioning, ProviderRouteState, TransportKind,
    WebRtcDataChannel, WebRtcSessionProvider, WebRtcSignalingProvider,
};

pub type SignalingExchange =
    Arc<dyn Fn(&[u8], Duration) -> Result<Vec<u8>, String> + Send + Sync + 'static>;
pub type SignalingReconnect = Arc<dyn Fn() -> Result<(), String> + Send + Sync + 'static>;

type WakeHandler = Arc<dyn Fn() + Send + Sync + 'static>;
const MAX_PENDING_INCOMING_CHANNELS: usize = 64;

/// A thread-safe hand-off point between a platform WebRTC implementation and
/// the provider-neutral runtime.
pub struct WebRtcHostBridge {
    local_hint: Vec<u8>,
    commissioning: RwLock<ProviderCommissioning>,
    channels: Mutex<HashMap<ContactId, Arc<dyn WebRtcDataChannel>>>,
    incoming: Mutex<VecDeque<Arc<dyn WebRtcDataChannel>>>,
    waker: Mutex<Option<WakeHandler>>,
    signaling_exchange: SignalingExchange,
    signaling_reconnect: SignalingReconnect,
}

impl WebRtcHostBridge {
    pub fn new(local_hint: Vec<u8>, signaling_exchange: SignalingExchange) -> Result<Self, String> {
        if local_hint.is_empty() || local_hint.len() > 8 * 1024 {
            return Err("WebRTC local signaling hint must be 1..=8192 bytes".into());
        }
        Ok(Self {
            local_hint,
            commissioning: RwLock::new(default_commissioning()),
            channels: Mutex::new(HashMap::new()),
            incoming: Mutex::new(VecDeque::new()),
            waker: Mutex::new(None),
            signaling_exchange,
            signaling_reconnect: Arc::new(|| Ok(())),
        })
    }

    /// Supplies the platform-owned signaling reconnect operation. This is
    /// invoked by the provider-neutral pairing transport after a signaling
    /// session expires or a network generation changes.
    pub fn with_signaling_reconnect(mut self, reconnect: SignalingReconnect) -> Self {
        self.signaling_reconnect = reconnect;
        self
    }

    /// Updates provider-owned commissioning facts after the host receives an
    /// ICE/signaling callback. The runtime is woken exactly once per update.
    pub fn set_commissioning(&self, commissioning: ProviderCommissioning) -> Result<(), String> {
        if commissioning.provider != TransportKind::WebRtc {
            return Err("commissioning snapshot belongs to another provider".into());
        }
        *self.commissioning.write().map_err(|_| "commissioning lock poisoned")? = commissioning;
        self.notify();
        Ok(())
    }

    /// Binds a negotiated channel to the persisted contact route.
    pub fn bind_contact(
        &self,
        contact_id: ContactId,
        channel: Arc<dyn WebRtcDataChannel>,
    ) -> Result<(), String> {
        if let Some(waker) = self.waker.lock().map_err(|_| "waker registry poisoned")?.clone() {
            channel.set_waker(waker);
        }
        let previous = self
            .channels
            .lock()
            .map_err(|_| "channel registry poisoned")?
            .insert(contact_id, channel);
        if let Some(previous) = previous {
            let _ = previous.close();
        }
        self.notify();
        Ok(())
    }

    /// Removes and closes the negotiated channel for one contact. Hosts call
    /// this when the SDK reports a terminal close so a later dial cannot
    /// accidentally retrieve a dead channel from the registry.
    pub fn unbind_contact(&self, contact_id: ContactId) -> Result<bool, String> {
        let channel =
            self.channels.lock().map_err(|_| "channel registry poisoned")?.remove(&contact_id);
        let removed = channel.is_some();
        if let Some(channel) = channel {
            let _ = channel.close();
        }
        if removed {
            self.notify();
        }
        Ok(removed)
    }

    /// Publishes an incoming negotiated channel for the runtime accept lane.
    pub fn push_incoming(&self, channel: Arc<dyn WebRtcDataChannel>) -> Result<(), String> {
        if let Some(waker) = self.waker.lock().map_err(|_| "waker registry poisoned")?.clone() {
            channel.set_waker(waker);
        }
        let mut incoming =
            self.incoming.lock().map_err(|_| "incoming channel registry poisoned")?;
        if incoming.len() >= MAX_PENDING_INCOMING_CHANNELS {
            let _ = channel.close();
            return Err("WebRTC incoming channel queue is full".into());
        }
        incoming.push_back(channel);
        self.notify();
        Ok(())
    }

    /// Removes all channels owned by the previous runtime generation.
    pub fn clear_channels(&self) -> Result<(), String> {
        let channels = self
            .channels
            .lock()
            .map_err(|_| "channel registry poisoned")?
            .drain()
            .map(|(_, channel)| channel)
            .collect::<Vec<_>>();
        let incoming = self
            .incoming
            .lock()
            .map_err(|_| "incoming channel registry poisoned")?
            .drain(..)
            .collect::<Vec<_>>();
        for channel in channels.into_iter().chain(incoming) {
            let _ = channel.close();
        }
        self.notify();
        Ok(())
    }

    fn notify(&self) {
        if let Ok(waker) = self.waker.lock()
            && let Some(waker) = waker.as_ref()
        {
            waker();
        }
    }
}

impl WebRtcSessionProvider for WebRtcHostBridge {
    fn local_endpoint_hint(&self) -> Result<Vec<u8>, String> {
        Ok(self.local_hint.clone())
    }

    fn accept(&self) -> Result<Option<Arc<dyn WebRtcDataChannel>>, String> {
        self.incoming
            .lock()
            .map_err(|_| "incoming channel registry poisoned".to_owned())
            .map(|mut channels| channels.pop_front())
    }

    fn connect(&self, contact: &Contact) -> Result<Arc<dyn WebRtcDataChannel>, String> {
        self.channels
            .lock()
            .map_err(|_| "channel registry poisoned")?
            .get(&contact.id())
            .cloned()
            .ok_or_else(|| format!("no negotiated WebRTC channel for contact {}", contact.id()))
    }

    fn set_waker(&self, waker: WakeHandler) {
        if let Ok(mut slot) = self.waker.lock() {
            *slot = Some(Arc::clone(&waker));
        }
        // Never invoke SDK callbacks while holding either registry lock. A
        // callback may synchronously wake the runtime and re-enter this
        // bridge (for example by binding a replacement channel), which would
        // otherwise deadlock the host thread.
        let channels = self
            .channels
            .lock()
            .map(|channels| channels.values().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        let incoming = self
            .incoming
            .lock()
            .map(|channels| channels.iter().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        for channel in channels.into_iter().chain(incoming) {
            channel.set_waker(Arc::clone(&waker));
        }
    }

    fn refresh_route(&self) -> Result<(), String> {
        // Invalidate the previous route before asking the SDK to renegotiate.
        // Until a host callback publishes a fresh commissioning snapshot,
        // consumers must not continue to advertise the old ICE candidates as
        // usable. The registry itself has no SDP/ICE knowledge; it only owns
        // this provider-neutral state transition and wakes the runtime.
        // Existing channels were negotiated against the old candidate set;
        // close them before renegotiation so the peer layer cannot keep
        // sending on a route that the host has declared stale.
        self.clear_channels()?;
        if let Ok(mut commissioning) = self.commissioning.write() {
            commissioning.route_state = ProviderRouteState::Stale;
            for step in &mut commissioning.steps {
                if matches!(
                    step.stage,
                    torca_transport_api::CommissioningStage::IncomingReachability
                ) {
                    step.state = CommissioningState::Pending;
                }
            }
        }
        self.notify();
        Ok(())
    }

    fn commissioning(&self) -> ProviderCommissioning {
        self.commissioning
            .read()
            .map(|value| value.clone())
            .unwrap_or_else(|_| default_commissioning())
    }
}

impl WebRtcSignalingProvider for WebRtcHostBridge {
    fn invalidate(&self) {
        // Signaling state is owned by the host callback. Clearing channels is
        // the only safe provider-neutral invalidation operation.
        let _ = self.clear_channels();
    }

    fn reconnect(&self) -> Result<(), String> {
        (self.signaling_reconnect)()?;
        self.notify();
        Ok(())
    }

    fn exchange(&self, request: &[u8], timeout: Duration) -> Result<Vec<u8>, String> {
        (self.signaling_exchange)(request, timeout)
    }
}

fn default_commissioning() -> ProviderCommissioning {
    ProviderCommissioning {
        provider: TransportKind::WebRtc,
        steps: vec![
            torca_transport_api::CommissioningStep {
                stage: torca_transport_api::CommissioningStage::LocalRuntime,
                state: CommissioningState::Ready,
                required_for_local_shell: true,
                required_for_pairing: false,
            },
            torca_transport_api::CommissioningStep {
                stage: torca_transport_api::CommissioningStage::IncomingReachability,
                state: CommissioningState::Pending,
                required_for_local_shell: false,
                required_for_pairing: true,
            },
        ],
        endpoint_summary: None,
        route_state: ProviderRouteState::Unavailable,
        pairing_bootstrap: None,
    }
}
