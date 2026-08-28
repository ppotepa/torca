//! Provider-neutral transport contracts for authenticated Torca peer sessions.
//!
//! This crate deliberately does not implement networking. Providers adapt
//! their byte-oriented sessions to these contracts,
//! while the application protocol remains shared.

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use torca_contacts::Contact;
use torca_foundation::{OpaqueId, ProviderId, Timestamp};
use torca_pairing_protocol::PairingBootstrapDescriptor;

pub use torca_provider_api::{
    MaintenanceOption, PairingBootstrapMode, ProviderCommissioningService,
    ProviderDeploymentProfile, ProviderDeploymentState, ProviderDescriptor, ProviderFeatures,
    ProviderProfileDescriptor, ProviderRouteState,
};

/// Provider-neutral byte stream used by the authenticated peer protocol.
///
/// Providers implement this contract; the peer session consumes it. Keeping
/// the contract here prevents application code from depending on a concrete
/// networking adapter merely to describe a byte stream.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PeerTransportError(pub String);

impl fmt::Display for PeerTransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for PeerTransportError {}

pub trait PeerTransport {
    fn connect(&mut self) -> Result<(), PeerTransportError>;
    fn send(&mut self, payload: Vec<u8>) -> Result<(), PeerTransportError>;
    /// Sends several already-framed payloads as one provider operation.
    /// Providers that cannot coalesce writes retain correct semantics through
    /// the default implementation; stream providers such as Iroh can hold a
    /// single write lock and flush once to reduce mobile radio wakeups.
    fn send_batch(&mut self, payloads: Vec<Vec<u8>>) -> Result<(), PeerTransportError> {
        for payload in payloads {
            self.send(payload)?;
        }
        Ok(())
    }
    fn try_receive(&mut self) -> Result<Option<Vec<u8>>, PeerTransportError>;
    fn receive_timeout(
        &mut self,
        _timeout: Duration,
    ) -> Result<Option<Vec<u8>>, PeerTransportError> {
        self.try_receive()
    }
    fn close(&mut self) -> Result<(), PeerTransportError>;
    fn set_waker(&mut self, _waker: Arc<dyn Fn() + Send + Sync>) {}
}

impl<T> PeerTransport for Box<T>
where
    T: PeerTransport + ?Sized,
{
    fn connect(&mut self) -> Result<(), PeerTransportError> {
        (**self).connect()
    }

    fn send(&mut self, payload: Vec<u8>) -> Result<(), PeerTransportError> {
        (**self).send(payload)
    }

    fn send_batch(&mut self, payloads: Vec<Vec<u8>>) -> Result<(), PeerTransportError> {
        (**self).send_batch(payloads)
    }

    fn try_receive(&mut self) -> Result<Option<Vec<u8>>, PeerTransportError> {
        (**self).try_receive()
    }

    fn receive_timeout(
        &mut self,
        timeout: Duration,
    ) -> Result<Option<Vec<u8>>, PeerTransportError> {
        (**self).receive_timeout(timeout)
    }

    fn close(&mut self) -> Result<(), PeerTransportError> {
        (**self).close()
    }

    fn set_waker(&mut self, waker: Arc<dyn Fn() + Send + Sync>) {
        (**self).set_waker(waker);
    }
}

pub fn parse_provider_id(value: &str) -> Result<ProviderId, TransportParseError> {
    ProviderId::new(value).map_err(|_| TransportParseError(value.to_owned()))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransportParseError(pub String);

/// A stable, provider-neutral stage of bringing communication online.
///
/// The application renders these stages; provider adapters decide which ones
/// exist and when each becomes ready.  In particular, `IncomingReachability`
/// is not synonymous with any particular provider endpoint representation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CommissioningStage {
    LocalRuntime,
    IncomingReachability,
    PairingRendezvous,
}

/// State of one provider commissioning stage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommissioningState {
    NotRequired,
    Pending,
    Ready,
    Degraded,
    Failed,
}

/// One displayable commissioning stage supplied by the active provider.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommissioningStep {
    pub stage: CommissioningStage,
    pub state: CommissioningState,
    /// Local UI must wait for this stage before it can issue runtime commands.
    pub required_for_local_shell: bool,
    /// Pairing can use the provider only after this stage is ready.
    pub required_for_pairing: bool,
}

/// Snapshot consumed by bootstrap projection, diagnostics and deploy health.
///
/// `endpoint_summary` is intentionally presentation-only. It must not be
/// parsed by application policy and never carries secrets.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderCommissioning {
    pub provider: ProviderId,
    pub steps: Vec<CommissioningStep>,
    pub endpoint_summary: Option<String>,
    /// Current provider route state. This is separate from commissioning
    /// reachability: a provider may be locally ready while its advertised
    /// route is temporarily stale during network migration.
    pub route_state: ProviderRouteState,
    /// Opaque provider-owned bootstrap material used to reconstruct a
    /// durable invitation URI after the original create command has left the
    /// FFI boundary.
    pub pairing_bootstrap: Option<PairingBootstrapDescriptor>,
}

/// Redacted progress emitted while a selected provider is commissioned.
///
/// The host owns presentation and retry policy. Provider adapters only map
/// their native events to these stable stages and never expose library types
/// over the native/Flutter boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommissioningEvent {
    pub stage: CommissioningStage,
    pub progress: u8,
    pub attempt: u32,
    pub retry_after_ms: Option<u64>,
    pub code: String,
    pub summary: String,
}

pub type CommissioningObserver = Arc<dyn Fn(CommissioningEvent) + Send + Sync>;

impl ProviderCommissioning {
    pub fn step(&self, stage: CommissioningStage) -> CommissioningState {
        self.steps
            .iter()
            .find(|step| step.stage == stage)
            .map_or(CommissioningState::NotRequired, |step| step.state)
    }

    pub fn local_shell_ready(&self) -> bool {
        self.steps.iter().all(|step| {
            !step.required_for_local_shell
                || matches!(step.state, CommissioningState::Ready | CommissioningState::NotRequired)
        })
    }

    pub fn pairing_ready(&self) -> bool {
        self.steps.iter().all(|step| {
            !step.required_for_pairing
                || matches!(step.state, CommissioningState::Ready | CommissioningState::NotRequired)
        })
    }

    /// Returns whether a stage is required for the requested runtime surface.
    /// Keeping this decision next to the provider profile prevents deploy,
    /// native bootstrap and UI projections from inventing Tor-specific gates.
    pub fn requires_for_local_shell(&self, stage: CommissioningStage) -> bool {
        self.steps.iter().any(|step| step.stage == stage && step.required_for_local_shell)
    }

    pub fn requires_for_pairing(&self, stage: CommissioningStage) -> bool {
        self.steps.iter().any(|step| step.stage == stage && step.required_for_pairing)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransportPath {
    pub provider: ProviderId,
    pub topology: TransportTopology,
}

#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Deserialize, serde::Serialize,
)]
#[serde(rename_all = "snake_case")]
pub enum TransportTopology {
    Direct,
    Relay,
    Unknown,
}

/// Redaction-safe provider runtime facts used by diagnostics and soak runs.
///
/// These are deliberately optional: a provider that does not expose endpoint
/// generations or public reachability can still participate without inventing
/// provider-shaped values. No endpoint address, peer identity or secret crosses
/// this boundary.
#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderRuntimeDiagnostics {
    pub endpoint_generation: Option<u64>,
    /// Monotonic generation of the provider's advertised route. Unlike the
    /// endpoint generation, this also changes when the same endpoint learns
    /// new IP/relay addresses after a platform network transition.
    pub route_generation: Option<u64>,
    pub network_generation: Option<u64>,
    pub endpoint_active: Option<bool>,
    /// Whether the provider currently has a route safe to advertise. `None`
    /// means the provider does not expose route freshness.
    pub route_fresh: Option<bool>,
    /// Typed route state for diagnostics and provider-neutral scheduling.
    /// `route_fresh` remains for compatibility with older consumers.
    pub route_state: Option<ProviderRouteState>,
    /// Number of provider runtime workers. This is a diagnostic fact so
    /// battery experiments can compare a one-worker mobile profile with the
    /// default pool without parsing provider-specific configuration.
    pub runtime_threads: Option<u16>,
    /// Relative provider energy class used for diagnostics and soak labels.
    /// This is not a physical battery measurement and must not be presented
    /// as mAh or a percentage.
    pub energy_class: Option<EnergyClass>,
    /// Whether the runtime currently holds an incoming-reachability demand.
    /// This is a policy fact, not a claim that the provider is reachable.
    pub reachability_demanded: Option<bool>,
    pub incoming_reachability: Option<String>,
    pub online_probe_attempts: Option<u64>,
    pub online_probe_failures: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LatencyClass {
    Interactive,
    Normal,
    High,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EnergyClass {
    Low,
    Medium,
    High,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransportCapabilities {
    pub reliable: bool,
    pub ordered: bool,
    pub supports_incoming: bool,
    pub supports_direct_path: bool,
    pub supports_relay_path: bool,
    pub hides_peer_ip: bool,
    pub max_frame_size: usize,
    pub latency: LatencyClass,
    pub energy: EnergyClass,
}

/// Provider-neutral capabilities of a realtime lane such as Radio.  These
/// facts are deliberately transport-level: the Radio coordinator still owns
/// floor arbitration and audio semantics, while the selected provider owns
/// connection lifecycle and its idle budget.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RealtimeCapabilities {
    pub reliable: bool,
    pub ordered: bool,
    pub supports_datagrams: bool,
    pub max_frame_size: usize,
    /// Maximum safe application silence in milliseconds.  A provider must not
    /// be forced to use a longer heartbeat gap than its transport can sustain.
    pub max_idle_interval_ms: u64,
    /// Whether the application must emit a heartbeat while the lane is idle.
    /// QUIC providers such as Iroh have their own endpoint/connection liveness
    /// machinery and should not be forced into a second periodic heartbeat.
    pub requires_application_keep_alive: bool,
}

impl Default for RealtimeCapabilities {
    fn default() -> Self {
        Self {
            reliable: true,
            ordered: true,
            supports_datagrams: false,
            max_frame_size: 64 * 1024,
            max_idle_interval_ms: 30_000,
            requires_application_keep_alive: true,
        }
    }
}

pub trait ProviderTransport: PeerTransport + Send {
    fn provider_id(&self) -> ProviderId;
    fn path(&self) -> TransportPath;
    fn capabilities(&self) -> TransportCapabilities;
}

/// Provider-neutral factory for incoming and outgoing byte transports used by
/// one authenticated Torca peer link.
///
/// The factory deliberately knows only persisted contact routes and byte
/// streams. Handshake, delivery, retries and application encryption remain
/// above this boundary.
pub trait PeerTransportFactory: Send {
    fn provider_id(&self) -> ProviderId;
    fn capabilities(&self) -> TransportCapabilities;
    fn accept(&mut self) -> Result<Option<Box<dyn PeerTransport + Send>>, TransportFactoryError>;
    fn connect(
        &mut self,
        contact: &Contact,
    ) -> Result<Box<dyn PeerTransport + Send>, TransportFactoryError>;
    /// Whether existing sessions can survive an OS network-generation event.
    /// QUIC providers can migrate paths in place; stream providers that cannot
    /// do so retain the conservative close-and-reconnect behavior.
    fn preserves_sessions_on_network_change(&self) -> bool {
        false
    }
    fn set_waker(&self, waker: Arc<dyn Fn() + Send + Sync>) -> Result<(), TransportFactoryError>;
}

/// Error vocabulary shared by transport factories without exposing a provider
/// library error type to the peer protocol.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransportFactoryError {
    Listener,
    ContactNotFound,
    Protocol,
    /// The provider has a valid endpoint, but its local route is temporarily
    /// stale (for example during an Iroh Wi-Fi/LTE migration). This is
    /// retryable and must not be reported as a protocol failure.
    RouteStale,
}

/// Metadata used by manager and diagnostics without exposing provider bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransportSessionSnapshot {
    pub session_id: OpaqueId,
    pub provider_id: ProviderId,
    pub path: TransportPath,
    pub last_activity_at: Option<Timestamp>,
}

/// Provider-neutral manager boundary. A manager must return at most one
/// transport for a contact; fallback is selected only after the active one is
/// closed.
pub trait TransportManager: Send {
    fn default_provider(&self) -> ProviderId;
    fn active_session(&self, contact_id: OpaqueId) -> Option<TransportSessionSnapshot>;
    fn set_waker(&mut self, waker: Arc<dyn Fn() + Send + Sync>);
    fn next_wake(&self) -> Option<Duration>;
    fn shutdown(&mut self);
}

/// Redaction-safe error boundary for future providers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransportError {
    Unavailable,
    Timeout,
    Authentication,
    Protocol,
    PolicyDenied,
    NetworkChanged,
    Closed,
    Provider(String),
}

#[cfg(test)]
mod tests {
    use super::{
        CommissioningStage, CommissioningState, CommissioningStep, ProviderCommissioning,
        ProviderId,
    };

    #[test]
    fn local_shell_does_not_wait_for_optional_incoming_reachability() {
        let commissioning = ProviderCommissioning {
            provider: ProviderId::default(),
            steps: vec![
                CommissioningStep {
                    stage: CommissioningStage::LocalRuntime,
                    state: CommissioningState::Ready,
                    required_for_local_shell: true,
                    required_for_pairing: true,
                },
                CommissioningStep {
                    stage: CommissioningStage::IncomingReachability,
                    state: CommissioningState::Pending,
                    required_for_local_shell: false,
                    required_for_pairing: false,
                },
            ],
            endpoint_summary: None,
            route_state: crate::ProviderRouteState::Unavailable,
            pairing_bootstrap: None,
        };

        assert!(commissioning.local_shell_ready());
        assert!(commissioning.pairing_ready());
    }
}
