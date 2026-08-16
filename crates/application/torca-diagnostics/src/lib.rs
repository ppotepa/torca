//! Bounded redacted diagnostics, health snapshots and deterministic fault injection.

use core::fmt;
use std::collections::{BTreeMap, VecDeque};
use std::fmt::Write as _;

use torca_battery::{
    BatteryLedger, BatteryMetric, BatteryProfile, BatterySnapshot, PlatformEnergySample,
    RuntimePolicySnapshot, WakeReason,
};
use torca_foundation::{OpaqueId, Timestamp};

/// Backwards-compatible name for the unified battery ledger snapshot.
pub type RuntimeCounters = BatterySnapshot;
/// Backwards-compatible name for the unified battery metric enum.
pub type RuntimeCounter = BatteryMetric;

/// Application component represented in health reports.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Component {
    Engine,
    Storage,
    Crypto,
    Relay,
    Peer,
    Tor,
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
    /// Stores the latest redaction-safe scheduler projection. Resource IDs are
    /// intentionally absent; this is the source for the Why Awake view.
    pub fn set_policy_snapshot(&mut self, snapshot: RuntimePolicySnapshot) {
        self.policy = Some(snapshot);
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
                let work = policy
                    .scheduled_work_classes
                    .iter()
                    .map(|(class, count)| format!("\"{class:?}\":{count}"))
                    .collect::<Vec<_>>()
                    .join(",");
                format!(
                    "{{\"activeLeases\":{},\"activeDemands\":{},\"scheduledDeadlines\":{},\"nextDeadlineInMs\":{},\"networkGeneration\":{},\"focusActive\":{},\"focusRemainingMs\":{},\"leaseReasons\":{{{reasons}}},\"scheduledWork\":{{{work}}}}}",
                    policy.active_leases,
                    policy.active_demands,
                    policy.scheduled_deadlines,
                    policy.next_deadline_in_ms.map_or_else(|| "null".into(), |value| value.to_string()),
                    policy.network_generation,
                    policy.focus_active,
                    policy.focus_remaining_ms.map_or_else(|| "null".into(), |value| value.to_string()),
                )
            });
        let _ = write!(
            output,
            "],\"batteryProfile\":\"{:?}\",\"platform\":{{\"batteryPercent\":{},\"charging\":{},\"powerSaver\":{},\"meteredNetwork\":{},\"processCpuMs\":{},\"uidTxBytes\":{},\"uidRxBytes\":{}}},\"whyAwake\":{},\"counters\":{{\"schedulerWakeups\":{},\"snapshotBuilds\":{},\"peerProbes\":{},\"relayProbes\":{},\"ffiWakes\":{},\"dbReads\":{},\"dbWrites\":{},\"blobWrites\":{},\"projectionEvents\":{},\"radioWakeups\":{},\"torDials\":{},\"relayDials\":{},\"peerDials\":{},\"handshakes\":{},\"txFrames\":{},\"rxFrames\":{},\"attachmentChunksTx\":{},\"attachmentChunksRx\":{},\"suppressedWork\":{},\"totalWork\":{}}}}}",
            self.profile,
            optional_json_u8(platform.battery_percent),
            optional_json_bool(platform.charging),
            optional_json_bool(platform.power_saver),
            optional_json_bool(platform.metered_network),
            optional_json_u64(platform.process_cpu_ms),
            optional_json_u64(platform.uid_tx_bytes),
            optional_json_u64(platform.uid_rx_bytes),
            why_awake,
            counters.scheduler_wakeups,
            counters.snapshot_builds,
            counters.peer_probes,
            counters.relay_probes,
            counters.ffi_wakes,
            counters.db_reads,
            counters.db_writes,
            counters.blob_writes,
            counters.projection_events,
            counters.radio_wakeups,
            counters.tor_dials,
            counters.relay_dials,
            counters.peer_dials,
            counters.handshakes,
            counters.tx_frames,
            counters.rx_frames,
            counters.attachment_chunks_tx,
            counters.attachment_chunks_rx,
            counters.suppressed_work,
            counters.total_work(),
        );
        output
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
        assert!(diagnostics.export_json().contains("\"batteryPercent\":73"));
    }

    #[test]
    fn why_awake_projection_is_redacted_and_exported() {
        let now = std::time::Instant::now();
        let mut governor = torca_battery::RuntimeGovernor::new(now);
        governor.acquire_lease(torca_battery::WorkDemand {
            scope: torca_battery::ResourceScope::Relay,
            class: torca_battery::WorkClass::RelayProbe,
            reason: torca_battery::DemandReason::ActivePairing,
            owner: torca_battery::OpaqueId::from_u128(42),
            expires_at: now + std::time::Duration::from_secs(30),
        });
        let mut diagnostics = DiagnosticBuffer::new(4);
        diagnostics.set_policy_snapshot(governor.snapshot(now));
        let json = diagnostics.export_json();
        assert!(json.contains("\"whyAwake\""));
        assert!(json.contains("\"activeLeases\":1"));
        assert!(json.contains("ActivePairing"));
        assert!(!json.contains("42"));
    }
}
