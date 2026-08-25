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
    CommissioningState, ProviderCommissioning, TransportKind, WebRtcDataChannel,
    WebRtcSessionProvider, WebRtcSignalingProvider,
};

pub type SignalingExchange =
    Arc<dyn Fn(&[u8], Duration) -> Result<Vec<u8>, String> + Send + Sync + 'static>;

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
        })
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

    /// Publishes an incoming negotiated channel for the runtime accept lane.
    pub fn push_incoming(&self, channel: Arc<dyn WebRtcDataChannel>) -> Result<(), String> {
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
            *slot = Some(waker);
        }
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
        pairing_bootstrap: None,
    }
}
