//! Process-wide, payload-free connectivity observations and projections.

mod supervisor;
pub use supervisor::{
    PairingServiceHealthHandle, PairingServiceHealthPort, PairingServiceHealthSnapshot,
    PairingServiceHealthWorker,
};
mod peer_supervisor;
pub use peer_supervisor::{PeerProbeCandidate, PeerProbeRequest, PeerProbeSupervisor};

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};

use torca_foundation::{ErrorCode, OpaqueId, Timestamp};
use torca_probing::{ProbeResult, ProbeStatus, ProbeTarget};

const EVENT_CAPACITY: usize = 1024;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum TransportLayer {
    /// Selected communication provider. New provider adapters record here.
    Communication,
    /// Provider-owned short-lived service used to exchange pairing state.
    /// It may be a rendezvous relay, discovery service or signaling service.
    PairingService,
    Peer(Option<OpaqueId>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransportDirection {
    Tx,
    Rx,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransportOperation {
    Connect,
    Handshake,
    Request,
    Response,
    Envelope,
    Ack,
    Probe,
    Keepalive,
    Reconnect,
    Route,
    Lease,
}

/// Redaction-safe pipeline stage. Variants intentionally carry no provider
/// endpoint, address, capability, or payload data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransportStage {
    Factory,
    Connect,
    Handshake,
    RouteStale,
    RouteRefreshed,
    RouteAdvertised,
    RouteApplied,
    Message,
    Receipt,
    LeaseAcquired,
    LeaseReleased,
    ReconnectPreferredDialer,
    ReconnectRecovery,
    ReconnectDurableDemand,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationPhase {
    Started,
    Completed,
    Failed,
    TimedOut,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransportEvent {
    pub cursor: u64,
    pub layer: TransportLayer,
    pub direction: Option<TransportDirection>,
    pub operation: TransportOperation,
    pub stage: Option<TransportStage>,
    pub phase: OperationPhase,
    pub correlation_id: Option<OpaqueId>,
    pub at: Timestamp,
    pub latency_ms: Option<u64>,
    pub error_code: Option<ErrorCode>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ChannelSnapshot {
    pub tx_sequence: u64,
    pub rx_sequence: u64,
    pub in_flight: u32,
    pub queued: u32,
    pub last_tx_at: Option<Timestamp>,
    pub last_rx_at: Option<Timestamp>,
    pub last_success_at: Option<Timestamp>,
    pub latency_ms: Option<u64>,
    pub consecutive_failures: u32,
    pub reconnect_attempt: u32,
    pub last_error: Option<ErrorCode>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ConnectivitySnapshot {
    /// Provider-neutral communication activity used by new presentation and
    /// diagnostics code.
    pub communication: ChannelSnapshot,
    /// Provider-neutral pairing-service activity used by new UI and policy.
    pub pairing_service: ChannelSnapshot,
    /// Legacy relay projection. It mirrors `pairing_service` while older
    /// bridge consumers still deserialize it.
    pub relay: ChannelSnapshot,
    pub peer: ChannelSnapshot,
    pub peers_ready: u32,
    pub peers_total: u32,
    pub event_cursor: u64,
}

#[derive(Default)]
struct Ledger {
    cursor: u64,
    events: VecDeque<TransportEvent>,
    communication: ChannelSnapshot,
    pairing_service: ChannelSnapshot,
    peer: ChannelSnapshot,
    peer_states: BTreeMap<OpaqueId, bool>,
    last_probes: BTreeMap<String, (ProbeStatus, Option<u64>)>,
}

#[derive(Clone, Default)]
pub struct ConnectivityObserver {
    inner: Arc<Mutex<Ledger>>,
}

impl ConnectivityObserver {
    pub fn record(
        &self,
        layer: TransportLayer,
        direction: Option<TransportDirection>,
        operation: TransportOperation,
        phase: OperationPhase,
        correlation_id: Option<OpaqueId>,
        at: Timestamp,
        latency_ms: Option<u64>,
        error_code: Option<ErrorCode>,
    ) {
        self.record_with_stage(
            layer,
            direction,
            operation,
            phase,
            correlation_id,
            at,
            latency_ms,
            error_code,
            None,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_with_stage(
        &self,
        layer: TransportLayer,
        direction: Option<TransportDirection>,
        operation: TransportOperation,
        phase: OperationPhase,
        correlation_id: Option<OpaqueId>,
        at: Timestamp,
        latency_ms: Option<u64>,
        error_code: Option<ErrorCode>,
        stage: Option<TransportStage>,
    ) {
        let Ok(mut ledger) = self.inner.lock() else { return };
        ledger.cursor = ledger.cursor.saturating_add(1);
        let cursor = ledger.cursor;
        let channel = match layer {
            TransportLayer::Communication => &mut ledger.communication,
            TransportLayer::PairingService => &mut ledger.pairing_service,
            TransportLayer::Peer(_) => &mut ledger.peer,
        };
        if phase == OperationPhase::Started {
            channel.in_flight = channel.in_flight.saturating_add(1);
        } else {
            channel.in_flight = channel.in_flight.saturating_sub(1);
        }
        match direction {
            // TX is observable when an operation is emitted, not when the
            // corresponding round-trip later completes. Keeping TX on
            // `Completed` collapsed request and response into one UI frame.
            Some(TransportDirection::Tx) if phase == OperationPhase::Started => {
                channel.tx_sequence = channel.tx_sequence.saturating_add(1);
                channel.last_tx_at = Some(at);
            }
            Some(TransportDirection::Rx) if phase == OperationPhase::Completed => {
                channel.rx_sequence = channel.rx_sequence.saturating_add(1);
                channel.last_rx_at = Some(at);
            }
            _ => {}
        }
        match phase {
            OperationPhase::Completed => {
                channel.last_success_at = Some(at);
                channel.latency_ms = latency_ms.or(channel.latency_ms);
                channel.consecutive_failures = 0;
                channel.last_error = None;
            }
            OperationPhase::Failed | OperationPhase::TimedOut => {
                channel.consecutive_failures = channel.consecutive_failures.saturating_add(1);
                channel.last_error = error_code;
            }
            OperationPhase::Started => {}
        }
        if operation == TransportOperation::Reconnect && phase == OperationPhase::Started {
            channel.reconnect_attempt = channel.reconnect_attempt.saturating_add(1);
        }
        ledger.events.push_back(TransportEvent {
            cursor,
            layer,
            direction,
            operation,
            stage,
            phase,
            correlation_id,
            at,
            latency_ms,
            error_code,
        });
        while ledger.events.len() > EVENT_CAPACITY {
            let _ = ledger.events.pop_front();
        }
    }

    pub fn record_probe(&self, probe: &ProbeResult) {
        let probe_key = format!("{:?}:{:?}", probe.target, probe.kind);
        if let Ok(mut ledger) = self.inner.lock() {
            let signature = (probe.status, probe.latency_ms);
            if ledger.last_probes.get(&probe_key) == Some(&signature) {
                return;
            }
            ledger.last_probes.insert(probe_key, signature);
        }
        let layer = match probe.target {
            ProbeTarget::Communication | ProbeTarget::IncomingReachability => {
                TransportLayer::Communication
            }
            ProbeTarget::PairingService | ProbeTarget::Relay => TransportLayer::PairingService,
            ProbeTarget::Peer => TransportLayer::Peer(None),
            _ => return,
        };
        let phase = match probe.status {
            ProbeStatus::Healthy => OperationPhase::Completed,
            // Checking is in-flight work. Counting it as success used to make
            // LEDs flash green before any relay response had arrived.
            ProbeStatus::Checking | ProbeStatus::Unknown => OperationPhase::Started,
            ProbeStatus::Degraded | ProbeStatus::Unreachable | ProbeStatus::Failed => {
                OperationPhase::Failed
            }
            ProbeStatus::Disabled => return,
        };
        let direction = match probe.status {
            ProbeStatus::Checking => Some(TransportDirection::Tx),
            ProbeStatus::Unknown => None,
            ProbeStatus::Healthy
            | ProbeStatus::Degraded
            | ProbeStatus::Unreachable
            | ProbeStatus::Failed => Some(TransportDirection::Rx),
            ProbeStatus::Disabled => None,
        };
        self.record(
            layer,
            direction,
            TransportOperation::Probe,
            phase,
            None,
            probe.measured_at,
            probe.latency_ms,
            (phase == OperationPhase::Failed).then(|| ErrorCode::new("connectivity.probe_failed")),
        );
    }

    pub fn set_peer_ready(&self, peer_id: OpaqueId, ready: bool) {
        if let Ok(mut ledger) = self.inner.lock() {
            ledger.peer_states.insert(peer_id, ready);
        }
    }

    pub fn set_queued(&self, layer: TransportLayer, queued: u32) {
        if let Ok(mut ledger) = self.inner.lock() {
            match layer {
                TransportLayer::Communication => {
                    ledger.communication.queued = queued;
                }
                TransportLayer::PairingService => {
                    ledger.pairing_service.queued = queued;
                }
                TransportLayer::Peer(_) => ledger.peer.queued = queued,
            }
        }
    }

    pub fn snapshot(&self) -> ConnectivitySnapshot {
        let Ok(ledger) = self.inner.lock() else { return ConnectivitySnapshot::default() };
        ConnectivitySnapshot {
            communication: ledger.communication,
            pairing_service: ledger.pairing_service,
            relay: ledger.pairing_service,
            peer: ledger.peer,
            peers_ready: u32::try_from(ledger.peer_states.values().filter(|ready| **ready).count())
                .unwrap_or(u32::MAX),
            peers_total: u32::try_from(ledger.peer_states.len()).unwrap_or(u32::MAX),
            event_cursor: ledger.cursor,
        }
    }

    pub fn events_after(&self, cursor: u64) -> Vec<TransportEvent> {
        self.inner.lock().map_or_else(
            |_| Vec::new(),
            |ledger| ledger.events.iter().filter(|event| event.cursor > cursor).cloned().collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tx_rx_and_failures_are_projected_without_payloads() {
        let observer = ConnectivityObserver::default();
        observer.record(
            TransportLayer::PairingService,
            Some(TransportDirection::Tx),
            TransportOperation::Request,
            OperationPhase::Started,
            None,
            Timestamp::UNIX_EPOCH,
            None,
            None,
        );
        observer.record(
            TransportLayer::PairingService,
            Some(TransportDirection::Tx),
            TransportOperation::Request,
            OperationPhase::Completed,
            None,
            Timestamp::UNIX_EPOCH,
            Some(12),
            None,
        );
        let snapshot = observer.snapshot();
        assert_eq!(snapshot.relay.tx_sequence, 1);
        assert_eq!(snapshot.relay.in_flight, 0);
        assert_eq!(snapshot.relay.latency_ms, Some(12));
        assert_eq!(observer.events_after(0).len(), 2);
    }

    #[test]
    fn pairing_service_probe_drives_real_tx_then_rx_activity() {
        let observer = ConnectivityObserver::default();
        observer.record_probe(&ProbeResult {
            target: ProbeTarget::PairingService,
            kind: torca_probing::ProbeKind::Connectivity,
            status: ProbeStatus::Checking,
            diagnostic_code: "PAIRING_SERVICE_CHECKING".into(),
            latency_ms: None,
            measured_at: Timestamp::UNIX_EPOCH,
        });
        observer.record_probe(&ProbeResult {
            target: ProbeTarget::PairingService,
            kind: torca_probing::ProbeKind::Connectivity,
            status: ProbeStatus::Healthy,
            diagnostic_code: "PAIRING_SERVICE_READY".into(),
            latency_ms: Some(9),
            measured_at: Timestamp::UNIX_EPOCH,
        });
        let snapshot = observer.snapshot();
        assert_eq!(snapshot.relay.tx_sequence, 1);
        assert_eq!(snapshot.relay.rx_sequence, 1);
        assert_eq!(snapshot.relay.latency_ms, Some(9));
    }
}
