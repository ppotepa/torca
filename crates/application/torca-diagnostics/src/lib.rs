//! Bounded redacted diagnostics, health snapshots and deterministic fault injection.

use core::fmt;
use std::collections::{BTreeMap, VecDeque};
use std::fmt::Write as _;

use torca_foundation::{OpaqueId, Timestamp};

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
    counters: RuntimeCounters,
}

/// Application-controlled work counters used by the battery regression gate.
/// These are deliberately abstract counts, not claims about physical mWh.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RuntimeCounters {
    pub scheduler_wakeups: u64,
    pub snapshot_builds: u64,
    pub peer_probes: u64,
    pub relay_probes: u64,
    pub ffi_wakes: u64,
    pub db_reads: u64,
    pub db_writes: u64,
    pub blob_writes: u64,
    pub radio_wakeups: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeCounter {
    SchedulerWakeup,
    SnapshotBuild,
    PeerProbe,
    RelayProbe,
    FfiWake,
    DbRead,
    DbWrite,
    BlobWrite,
    RadioWake,
}

impl DiagnosticBuffer {
    /// Creates a buffer with at least one event slot.
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            events: VecDeque::new(),
            health: BTreeMap::new(),
            counters: RuntimeCounters::default(),
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
        let value = match counter {
            RuntimeCounter::SchedulerWakeup => &mut self.counters.scheduler_wakeups,
            RuntimeCounter::SnapshotBuild => &mut self.counters.snapshot_builds,
            RuntimeCounter::PeerProbe => &mut self.counters.peer_probes,
            RuntimeCounter::RelayProbe => &mut self.counters.relay_probes,
            RuntimeCounter::FfiWake => &mut self.counters.ffi_wakes,
            RuntimeCounter::DbRead => &mut self.counters.db_reads,
            RuntimeCounter::DbWrite => &mut self.counters.db_writes,
            RuntimeCounter::BlobWrite => &mut self.counters.blob_writes,
            RuntimeCounter::RadioWake => &mut self.counters.radio_wakeups,
        };
        *value = value.saturating_add(1);
    }
    pub fn counters(&self) -> RuntimeCounters {
        self.counters
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
        let counters = self.counters;
        let _ = write!(
            output,
            "],\"counters\":{{\"schedulerWakeups\":{},\"snapshotBuilds\":{},\"peerProbes\":{},\"relayProbes\":{},\"ffiWakes\":{},\"dbReads\":{},\"dbWrites\":{},\"blobWrites\":{},\"radioWakeups\":{}}}}}",
            counters.scheduler_wakeups,
            counters.snapshot_builds,
            counters.peer_probes,
            counters.relay_probes,
            counters.ffi_wakes,
            counters.db_reads,
            counters.db_writes,
            counters.blob_writes,
            counters.radio_wakeups,
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
        diagnostics.count(RuntimeCounter::DbWrite);
        diagnostics.count(RuntimeCounter::DbWrite);
        diagnostics.count(RuntimeCounter::BlobWrite);
        assert_eq!(diagnostics.counters().db_writes, 2);
        assert_eq!(diagnostics.counters().blob_writes, 1);
        assert!(diagnostics.export_json().contains("\"dbWrites\":2"));
        assert!(diagnostics.export_json().contains("\"blobWrites\":1"));
    }
}
