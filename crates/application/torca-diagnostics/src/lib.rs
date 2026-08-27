//! Bounded redacted diagnostics, health snapshots and deterministic fault injection.

mod battery;
pub use battery::{
    BatteryDiagnosticSnapshot, BatteryEvent, BatteryLedger, BatteryMetric, BatterySnapshot,
    BatterySpan, PlatformEnergyProvider, PlatformEnergySample, WakeReason,
};

use core::fmt;
use std::collections::{BTreeMap, VecDeque};
use std::fmt::Write as _;

use torca_foundation::{OpaqueId, Timestamp};
use torca_runtime_policy::{BatteryProfile, RuntimePolicySnapshot};

/// Backwards-compatible name for the unified battery ledger snapshot.
pub type RuntimeCounters = BatterySnapshot;
/// Backwards-compatible name for the unified battery metric enum.
pub type RuntimeCounter = BatteryMetric;

/// A redaction-safe explanation for a RuntimeOwner turn.  This is deliberately
/// independent from feature metrics: it answers *why the actor woke*, not how
/// much work a feature subsequently performed.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeWakeSource {
    Command,
    ProviderDeadline,
    PairingDeadline,
    DeliveryDeadline,
    RadioDeadline,
    PeerDeadline,
    RelayDeadline,
    LeaseExpiry,
    BackgroundGrace,
    NetworkChange,
    Platform,
    NativeRevision,
    Debug,
}

/// Redaction-safe view of the only application-owned deadline registry.
/// Resource identifiers intentionally do not cross this boundary.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RuntimeScheduleSnapshot {
    pub active_deadlines: u64,
    pub next_deadline_in_ms: Option<u64>,
    pub sources: BTreeMap<RuntimeWakeSource, u64>,
    pub zero_delay_deadlines: u64,
    pub identical_deadline_replacements: u64,
    pub peer_recovery_generation: u64,
    pub peer_recovery_attempts: u64,
    pub peer_recovery_exhausted: bool,
}

/// Bounded observation data for a user-started diagnostics interval.  It is a
/// counter delta, not a physical battery measurement.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BatteryObservation {
    pub active: bool,
    pub counters: BatterySnapshot,
    pub wake_sources: BTreeMap<RuntimeWakeSource, u64>,
}

#[derive(Clone, Debug, Default)]
struct ObservationState {
    active: bool,
    baseline: BatterySnapshot,
    wake_baseline: BTreeMap<RuntimeWakeSource, u64>,
}

/// Application component represented in health reports.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Component {
    Engine,
    Storage,
    Crypto,
    Relay,
    Peer,
    Communication,
    Bridge,
    Platform,
}
/// Health state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HealthState {
    Starting,
    Ready,
    Degraded,
    Failed,
    Stopped,
}
/// Redaction-safe short diagnostic code.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticCode(String);
impl DiagnosticCode {
    /// Creates an uppercase ASCII code.
    pub fn new(value: impl Into<String>) -> Result<Self, DiagnosticError> {
        let value = value.into().to_ascii_uppercase();
        if value.is_empty()
            || value.len() > 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
        {
            return Err(DiagnosticError::InvalidCode);
        }
        Ok(Self(value))
    }
    /// Returns the code.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
/// Bounded diagnostic detail that rejects common secret shapes and control characters.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RedactedDetail(String);
impl RedactedDetail {
    /// Maximum detail length.
    pub const MAX_BYTES: usize = 512;
    /// Validates a redacted detail.
    pub fn new(value: impl Into<String>) -> Result<Self, DiagnosticError> {
        let value = value.into();
        let lower = value.to_ascii_lowercase();
        if value.len() > Self::MAX_BYTES
            || value.chars().any(char::is_control)
            || ["private_key", "secret=", "capability=", "plaintext="]
                .iter()
                .any(|needle| lower.contains(needle))
        {
            return Err(DiagnosticError::SensitiveDetail);
        }
        Ok(Self(value))
    }
    /// Returns detail text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
/// Structured diagnostic event.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticEvent {
    pub event_id: OpaqueId,
    pub at: Timestamp,
    pub component: Component,
    pub state: HealthState,
    pub code: DiagnosticCode,
    pub detail: Option<RedactedDetail>,
}
/// Current component health record.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComponentHealth {
    pub component: Component,
    pub state: HealthState,
    pub code: DiagnosticCode,
    pub updated_at: Timestamp,
}
/// Immutable health snapshot.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HealthSnapshot {
    pub components: Vec<ComponentHealth>,
}
/// Diagnostics failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DiagnosticError {
    InvalidCode,
    SensitiveDetail,
    UnknownFailPoint,
}
impl fmt::Display for DiagnosticError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for DiagnosticError {}

/// Bounded in-memory diagnostics buffer.
pub struct DiagnosticBuffer {
    capacity: usize,
    events: VecDeque<DiagnosticEvent>,
    health: BTreeMap<Component, ComponentHealth>,
    battery: BatteryLedger,
    profile: BatteryProfile,
    policy: Option<RuntimePolicySnapshot>,
    schedule: RuntimeScheduleSnapshot,
    wake_sources: BTreeMap<RuntimeWakeSource, u64>,
    observation: ObservationState,
    /// Provider metadata is diagnostic context, not a policy input. Keeping
    /// it here makes exported soak/support bundles self-describing without
    /// exposing provider library types or endpoint material.
    communication_provider: Option<String>,
    provider_profile: Option<String>,
    provider_runtime: torca_transport_api::ProviderRuntimeDiagnostics,
}

impl DiagnosticBuffer {
    /// Creates a buffer with at least one event slot.
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            events: VecDeque::new(),
            health: BTreeMap::new(),
            battery: BatteryLedger::new(),
            profile: BatteryProfile::AlwaysAvailable,
            policy: None,
            schedule: RuntimeScheduleSnapshot::default(),
            wake_sources: BTreeMap::new(),
            observation: ObservationState::default(),
            communication_provider: None,
            provider_profile: None,
            provider_runtime: torca_transport_api::ProviderRuntimeDiagnostics::default(),
        }
    }
    /// Records an event and updates health.
    pub fn record(&mut self, event: DiagnosticEvent) {
        self.health.insert(
            event.component,
            ComponentHealth {
                component: event.component,
                state: event.state,
                code: event.code.clone(),
                updated_at: event.at,
            },
        );
        self.events.push_back(event);
        while self.events.len() > self.capacity {
            self.events.pop_front();
        }
    }
    /// Returns the current health snapshot.
    pub fn health(&self) -> HealthSnapshot {
        HealthSnapshot { components: self.health.values().cloned().collect() }
    }
    pub fn count(&mut self, counter: RuntimeCounter) {
        self.record_battery(counter, 1, WakeReason::Scheduler);
    }
    /// Adds a monotonic counter delta in one ledger operation. Runtime loops
    /// must use this for persisted deltas instead of iterating once per event.
    pub fn count_by(&mut self, counter: RuntimeCounter, amount: u64) {
        if amount > 0 {
            self.record_battery(counter, amount, WakeReason::Scheduler);
        }
    }
    /// Records a feature-owned metric with its explicit wake reason.
    pub fn record_battery(&mut self, metric: BatteryMetric, amount: u64, reason: WakeReason) {
        self.battery.record(metric, amount, reason);
    }
    /// Records the explicit cause that woke the runtime actor.
    pub fn record_runtime_wake(&mut self, source: RuntimeWakeSource) {
        *self.wake_sources.entry(source).or_default() += 1;
    }
    /// Starts a new diagnostics interval without resetting process counters.
    pub fn start_battery_observation(&mut self) {
        self.observation = ObservationState {
            active: true,
            baseline: self.battery.snapshot(),
            wake_baseline: self.wake_sources.clone(),
        };
    }
    /// Stops the current interval while retaining its final delta for export.
    pub fn stop_battery_observation(&mut self) {
        self.observation.active = false;
    }
    /// Resets the interval baseline and starts a new observation.
    pub fn reset_battery_observation(&mut self) {
        self.start_battery_observation();
    }
    /// Returns deltas since the last observation start.  Counter subtraction
    /// is saturating so a future bounded ledger implementation remains safe.
    pub fn battery_observation(&self) -> BatteryObservation {
        let current = self.battery.snapshot();
        BatteryObservation {
            active: self.observation.active,
            counters: battery_snapshot_delta(current, self.observation.baseline),
            wake_sources: self
                .wake_sources
                .iter()
                .map(|(source, count)| {
                    (
                        *source,
                        count.saturating_sub(
                            *self.observation.wake_baseline.get(source).unwrap_or(&0),
                        ),
                    )
                })
                .filter(|(_, count)| *count > 0)
                .collect(),
        }
    }
    pub fn counters(&self) -> RuntimeCounters {
        self.battery.snapshot()
    }
    /// Returns the shared battery ledger for feature executors.
    pub fn battery(&self) -> BatteryLedger {
        self.battery.clone()
    }
    /// Changes the local policy profile; delivery correctness remains owned by
    /// feature executors and is never disabled by this setting.
    pub fn set_battery_profile(&mut self, profile: BatteryProfile) {
        self.profile = profile;
    }
    pub fn battery_profile(&self) -> BatteryProfile {
        self.profile
    }
    /// Associates redaction-safe provider identity/profile with this buffer.
    /// The profile is an artifact/deployment label (for example `direct`),
    /// never an endpoint, relay URL or secret.
    pub fn set_provider_context(
        &mut self,
        provider: impl Into<String>,
        profile: Option<impl Into<String>>,
    ) {
        self.communication_provider = Some(provider.into());
        self.provider_profile = profile.map(Into::into);
    }
    /// Updates provider-owned runtime facts without exposing provider library
    /// types or endpoint material to the diagnostics boundary.
    pub fn set_provider_runtime(
        &mut self,
        runtime: torca_transport_api::ProviderRuntimeDiagnostics,
    ) {
        self.provider_runtime = runtime;
    }
    /// Stores the latest redaction-safe scheduler projection. Resource IDs are
    /// intentionally absent; this is the source for the Why Awake view.
    pub fn set_policy_snapshot(&mut self, snapshot: RuntimePolicySnapshot) {
        self.policy = Some(snapshot);
    }
    /// Stores the RuntimeOwner deadline registry independently from policy.
    /// This avoids pretending that attention/demand policy itself owns timers.
    pub fn set_runtime_schedule(&mut self, snapshot: RuntimeScheduleSnapshot) {
        self.schedule = snapshot;
    }
    /// Updates the platform sample after a lifecycle transition or explicit
    /// diagnostics request; no polling is performed here.
    pub fn set_platform_energy_sample(&mut self, sample: PlatformEnergySample) {
        self.battery.set_platform_sample(sample);
    }
    /// Exports deterministic JSON without private message or key fields.
    pub fn export_json(&self) -> String {
        let mut output = String::from("{\"events\":[");
        for (index, event) in self.events.iter().enumerate() {
            if index > 0 {
                output.push(',');
            }
            let _ = write!(
                output,
                "{{\"id\":\"{}\",\"at_ms\":{},\"component\":\"{:?}\",\"state\":\"{:?}\",\"code\":\"{}\"",
                event.event_id,
                event.at.to_unix_millis(),
                event.component,
                event.state,
                event.code.as_str()
            );
            if let Some(detail) = &event.detail {
                output.push_str(",\"detail\":\"");
                push_json_string(detail.as_str(), &mut output);
                output.push('"');
            }
            output.push('}');
        }
        let counters = self.battery.snapshot();
        let platform = self.battery.platform_sample();
        let why_awake = self
            .policy
            .as_ref()
            .map_or_else(|| "null".to_owned(), |policy| {
                let reasons = policy
                    .active_lease_reasons
                    .iter()
                    .map(|(reason, count)| format!("\"{reason:?}\":{count}"))
                    .collect::<Vec<_>>()
                    .join(",");
                let work = self
                    .schedule
                    .sources
                    .iter()
                    .map(|(source, count)| format!("\"{source:?}\":{count}"))
                    .collect::<Vec<_>>()
                    .join(",");
                format!(
                    "{{\"activeLeases\":{},\"activeDemands\":{},\"scheduledDeadlines\":{},\"nextDeadlineInMs\":{},\"zeroDelayDeadlines\":{},\"identicalDeadlineReplacements\":{},\"peerRecoveryGeneration\":{},\"peerRecoveryAttempts\":{},\"peerRecoveryExhausted\":{},\"networkGeneration\":{},\"focusActive\":{},\"focusRemainingMs\":{},\"leaseReasons\":{{{reasons}}},\"scheduledWork\":{{{work}}}}}",
                    policy.active_leases,
                    policy.active_demands,
                    self.schedule.active_deadlines,
                    self.schedule.next_deadline_in_ms.map_or_else(|| "null".into(), |value| value.to_string()),
                    self.schedule.zero_delay_deadlines,
                    self.schedule.identical_deadline_replacements,
                    self.schedule.peer_recovery_generation,
                    self.schedule.peer_recovery_attempts,
                    self.schedule.peer_recovery_exhausted,
                    policy.network_generation,
                    policy.focus_active,
                    policy.focus_remaining_ms.map_or_else(|| "null".into(), |value| value.to_string()),
                )
            });
        let observation = self.battery_observation();
        let wake_sources = observation
            .wake_sources
            .iter()
            .map(|(source, count)| format!("\"{source:?}\":{count}"))
            .collect::<Vec<_>>()
            .join(",");
        let observation_counters = format!(
            "\"schedulerWakeups\":{},\"peerProbes\":{},\"rendezvousProbes\":{},\"ffiWakes\":{},\"dbReads\":{},\"dbWrites\":{},\"peerDials\":{},\"providerDials\":{},\"rendezvousDials\":{}",
            observation.counters.scheduler_wakeups,
            observation.counters.peer_probes,
            observation.counters.rendezvous_probes,
            observation.counters.ffi_wakes,
            observation.counters.db_reads,
            observation.counters.db_writes,
            observation.counters.peer_dials,
            observation.counters.provider_dials,
            observation.counters.rendezvous_dials,
        );
        let _ = write!(
            output,
            "],\"communicationProvider\":{},\"providerProfile\":{},\"providerRuntime\":{},\"batteryProfile\":\"{:?}\",\"platform\":{{\"batteryPercent\":{},\"charging\":{},\"powerSaver\":{},\"meteredNetwork\":{},\"processCpuMs\":{},\"uidTxBytes\":{},\"uidRxBytes\":{}}},\"whyAwake\":{},\"observation\":{{\"active\":{},\"wakeSources\":{{{}}},\"counters\":{{{}}},\"totalWork\":{},\"energyScore\":{}}},\"counters\":{{\"schedulerWakeups\":{},\"snapshotBuilds\":{},\"peerProbes\":{},\"rendezvousProbes\":{},\"ffiWakes\":{},\"dbReads\":{},\"dbWrites\":{},\"blobWrites\":{},\"projectionEvents\":{},\"radioWakeups\":{},\"providerDials\":{},\"rendezvousDials\":{},\"peerDials\":{},\"handshakes\":{},\"txFrames\":{},\"rxFrames\":{},\"attachmentChunksTx\":{},\"attachmentChunksRx\":{},\"suppressedWork\":{},\"totalWork\":{},\"energyScore\":{}}}}}",
            optional_json_string(self.communication_provider.as_deref()),
            optional_json_string(self.provider_profile.as_deref()),
            serde_json::to_string(&self.provider_runtime).unwrap_or_else(|_| "{}".to_owned()),
            self.profile,
            optional_json_u8(platform.battery_percent),
            optional_json_bool(platform.charging),
            optional_json_bool(platform.power_saver),
            optional_json_bool(platform.metered_network),
            optional_json_u64(platform.process_cpu_ms),
            optional_json_u64(platform.uid_tx_bytes),
            optional_json_u64(platform.uid_rx_bytes),
            why_awake,
            observation.active,
            wake_sources,
            observation_counters,
            observation.counters.total_work(),
            observation.counters.energy_score(),
            counters.scheduler_wakeups,
            counters.snapshot_builds,
            counters.peer_probes,
            counters.rendezvous_probes,
            counters.ffi_wakes,
            counters.db_reads,
            counters.db_writes,
            counters.blob_writes,
            counters.projection_events,
            counters.radio_wakeups,
            counters.provider_dials,
            counters.rendezvous_dials,
            counters.peer_dials,
            counters.handshakes,
            counters.tx_frames,
            counters.rx_frames,
            counters.attachment_chunks_tx,
            counters.attachment_chunks_rx,
            counters.suppressed_work,
            counters.total_work(),
            counters.energy_score(),
        );
        output
    }
}

fn battery_snapshot_delta(current: BatterySnapshot, baseline: BatterySnapshot) -> BatterySnapshot {
    BatterySnapshot {
        scheduler_wakeups: current.scheduler_wakeups.saturating_sub(baseline.scheduler_wakeups),
        snapshot_builds: current.snapshot_builds.saturating_sub(baseline.snapshot_builds),
        peer_probes: current.peer_probes.saturating_sub(baseline.peer_probes),
        rendezvous_probes: current.rendezvous_probes.saturating_sub(baseline.rendezvous_probes),
        ffi_wakes: current.ffi_wakes.saturating_sub(baseline.ffi_wakes),
        db_reads: current.db_reads.saturating_sub(baseline.db_reads),
        db_writes: current.db_writes.saturating_sub(baseline.db_writes),
        blob_writes: current.blob_writes.saturating_sub(baseline.blob_writes),
        projection_events: current.projection_events.saturating_sub(baseline.projection_events),
        radio_wakeups: current.radio_wakeups.saturating_sub(baseline.radio_wakeups),
        provider_dials: current.provider_dials.saturating_sub(baseline.provider_dials),
        rendezvous_dials: current.rendezvous_dials.saturating_sub(baseline.rendezvous_dials),
        peer_dials: current.peer_dials.saturating_sub(baseline.peer_dials),
        handshakes: current.handshakes.saturating_sub(baseline.handshakes),
        tx_frames: current.tx_frames.saturating_sub(baseline.tx_frames),
        rx_frames: current.rx_frames.saturating_sub(baseline.rx_frames),
        attachment_chunks_tx: current
            .attachment_chunks_tx
            .saturating_sub(baseline.attachment_chunks_tx),
        attachment_chunks_rx: current
            .attachment_chunks_rx
            .saturating_sub(baseline.attachment_chunks_rx),
        suppressed_work: current.suppressed_work.saturating_sub(baseline.suppressed_work),
    }
}
fn push_json_string(value: &str, output: &mut String) {
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            value => output.push(value),
        }
    }
}
fn optional_json_bool(value: Option<bool>) -> String {
    value.map_or_else(|| "null".to_owned(), |value| value.to_string())
}
fn optional_json_u8(value: Option<u8>) -> String {
    value.map_or_else(|| "null".to_owned(), |value| value.to_string())
}
fn optional_json_u64(value: Option<u64>) -> String {
    value.map_or_else(|| "null".to_owned(), |value| value.to_string())
}
fn optional_json_string(value: Option<&str>) -> String {
    value.map_or_else(
        || "null".to_owned(),
        |value| {
            let mut output = String::with_capacity(value.len() + 2);
            output.push('"');
            push_json_string(value, &mut output);
            output.push('"');
            output
        },
    )
}

/// Deterministic fail-point registry for integration tests.
#[derive(Clone, Debug, Default)]
pub struct FailPoints {
    values: BTreeMap<String, usize>,
}
impl FailPoints {
    /// Arms a fail point for a number of hits.
    pub fn arm(&mut self, name: impl Into<String>, hits: usize) {
        self.values.insert(name.into(), hits);
    }
    /// Returns true and decrements the remaining count when armed.
    pub fn hit(&mut self, name: &str) -> bool {
        let Some(remaining) = self.values.get_mut(name) else {
            return false;
        };
        if *remaining == 0 {
            return false;
        }
        *remaining -= 1;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn energy_ledger_exports_database_write_counter() {
        let mut diagnostics = DiagnosticBuffer::new(4);
        diagnostics.set_battery_profile(BatteryProfile::BatterySaver);
        diagnostics.set_provider_context("iroh", Some("direct"));
        diagnostics.set_provider_runtime(torca_transport_api::ProviderRuntimeDiagnostics {
            endpoint_generation: Some(4),
            route_generation: Some(5),
            network_generation: Some(2),
            endpoint_active: Some(true),
            route_fresh: Some(true),
            route_state: Some(torca_transport_api::ProviderRouteState::Fresh),
            runtime_threads: Some(2),
            energy_class: Some(torca_transport_api::EnergyClass::Low),
            reachability_demanded: Some(false),
            incoming_reachability: Some("stopped".into()),
            online_probe_attempts: Some(3),
            online_probe_failures: Some(2),
        });
        diagnostics.set_platform_energy_sample(PlatformEnergySample {
            battery_percent: Some(73),
            charging: Some(false),
            ..PlatformEnergySample::default()
        });
        diagnostics.count(RuntimeCounter::DbWrite);
        diagnostics.count(RuntimeCounter::DbWrite);
        diagnostics.count(RuntimeCounter::BlobWrite);
        diagnostics.count(RuntimeCounter::ProjectionEvent);
        assert_eq!(diagnostics.counters().db_writes, 2);
        assert_eq!(diagnostics.counters().blob_writes, 1);
        assert_eq!(diagnostics.counters().projection_events, 1);
        assert!(diagnostics.export_json().contains("\"dbWrites\":2"));
        assert!(diagnostics.export_json().contains("\"blobWrites\":1"));
        assert!(diagnostics.export_json().contains("\"projectionEvents\":1"));
        assert!(diagnostics.export_json().contains("\"batteryProfile\":\"BatterySaver\""));
        assert!(diagnostics.export_json().contains("\"communicationProvider\":\"iroh\""));
        assert!(diagnostics.export_json().contains("\"providerProfile\":\"direct\""));
        assert!(diagnostics.export_json().contains("\"endpointGeneration\":4"));
        assert!(diagnostics.export_json().contains("\"routeGeneration\":5"));
        assert!(diagnostics.export_json().contains("\"routeFresh\":true"));
        assert!(diagnostics.export_json().contains("\"routeState\":\"fresh\""));
        assert!(diagnostics.export_json().contains("\"energyClass\":\"low\""));
        assert!(diagnostics.export_json().contains("\"batteryPercent\":73"));
    }

    #[test]
    fn observation_reports_only_work_after_its_baseline() {
        let mut diagnostics = DiagnosticBuffer::new(4);
        diagnostics.count(RuntimeCounter::DbRead);
        diagnostics.record_runtime_wake(RuntimeWakeSource::Command);
        diagnostics.start_battery_observation();
        diagnostics.count_by(RuntimeCounter::DbRead, 2);
        diagnostics.record_runtime_wake(RuntimeWakeSource::BackgroundGrace);

        let observation = diagnostics.battery_observation();
        assert!(observation.active);
        assert_eq!(observation.counters.db_reads, 2);
        assert_eq!(observation.wake_sources.get(&RuntimeWakeSource::Command), None);
        assert_eq!(observation.wake_sources.get(&RuntimeWakeSource::BackgroundGrace), Some(&1));

        diagnostics.stop_battery_observation();
        assert!(!diagnostics.battery_observation().active);
        assert!(diagnostics.export_json().contains("\"wakeSources\""));
        assert!(diagnostics.export_json().contains("\"counters\":{\"schedulerWakeups\":0"));
        assert!(diagnostics.export_json().contains("\"totalWork\":2"));
    }

    #[test]
    fn why_awake_projection_is_redacted_and_exported() {
        let now = std::time::Instant::now();
        let mut governor = torca_runtime_policy::RuntimeGovernor::new(now);
        governor.acquire_lease(torca_runtime_policy::WorkDemand {
            scope: torca_runtime_policy::ResourceScope::Rendezvous,
            class: torca_runtime_policy::WorkClass::RendezvousProbe,
            reason: torca_runtime_policy::DemandReason::ActivePairing,
            owner: OpaqueId::from_u128(9_999),
            expires_at: now + std::time::Duration::from_secs(30),
        });
        let mut diagnostics = DiagnosticBuffer::new(4);
        diagnostics.set_policy_snapshot(governor.snapshot(now));
        diagnostics.set_runtime_schedule(RuntimeScheduleSnapshot {
            active_deadlines: 1,
            next_deadline_in_ms: Some(42),
            sources: BTreeMap::from([(RuntimeWakeSource::DeliveryDeadline, 1)]),
            ..RuntimeScheduleSnapshot::default()
        });
        let json = diagnostics.export_json();
        assert!(json.contains("\"whyAwake\""));
        assert!(json.contains("\"activeLeases\":1"));
        assert!(json.contains("ActivePairing"));
        assert!(json.contains("DeliveryDeadline"));
        assert!(json.contains("\"nextDeadlineInMs\":42"));
        assert!(!json.contains("9999"));
    }
}
