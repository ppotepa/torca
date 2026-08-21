//! Battery policy, workload accounting and diagnostics primitives.
//!
//! This crate deliberately does not own a socket, database, platform API or
//! timer. Executors remain responsible for correctness; they use this crate
//! to request discretionary work and to record the work they actually did.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub use torca_foundation::OpaqueId;
pub use torca_runtime_policy::{
    AttentionContext, AttentionSurface, BackgroundSyncCadence, BatteryPolicy, BatteryPreferences,
    BatteryProfile, ConnectionLease, ContactAvailabilityMode, DemandReason, EffectiveBatteryPolicy,
    EvidenceKind, FocusLease, Freshness, LeaseLifetime, MeteredTransferPolicy, PolicyEvent,
    PolicyOverrideReason, RequestedBatteryMode, ResourceScope, RuntimeEventHub,
    RuntimeEventHubStats, RuntimeGovernor, RuntimePolicySnapshot, SystemEnergyState,
    TransferDecision, VisualActivityPolicy, WorkClass, WorkDemand,
};

/// A bounded, abstract work metric. Values are counts, not physical energy.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BatteryMetric {
    SchedulerWakeup,
    SnapshotBuild,
    PeerProbe,
    RelayProbe,
    FfiWake,
    DbRead,
    DbWrite,
    BlobWrite,
    ProjectionEvent,
    RadioWake,
    TorDial,
    RelayDial,
    PeerDial,
    Handshake,
    TxFrame,
    RxFrame,
    AttachmentChunkTx,
    AttachmentChunkRx,
    SuppressedWork,
}

/// A compact snapshot exported to diagnostics and the UI.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BatterySnapshot {
    pub scheduler_wakeups: u64,
    pub snapshot_builds: u64,
    pub peer_probes: u64,
    pub relay_probes: u64,
    pub ffi_wakes: u64,
    pub db_reads: u64,
    pub db_writes: u64,
    pub blob_writes: u64,
    pub projection_events: u64,
    pub radio_wakeups: u64,
    pub tor_dials: u64,
    pub relay_dials: u64,
    pub peer_dials: u64,
    pub handshakes: u64,
    pub tx_frames: u64,
    pub rx_frames: u64,
    pub attachment_chunks_tx: u64,
    pub attachment_chunks_rx: u64,
    pub suppressed_work: u64,
}

impl BatterySnapshot {
    /// Returns the sum of all abstract work units.
    pub fn total_work(&self) -> u64 {
        [
            self.scheduler_wakeups,
            self.snapshot_builds,
            self.peer_probes,
            self.relay_probes,
            self.ffi_wakes,
            self.db_reads,
            self.db_writes,
            self.blob_writes,
            self.projection_events,
            self.radio_wakeups,
            self.tor_dials,
            self.relay_dials,
            self.peer_dials,
            self.handshakes,
            self.tx_frames,
            self.rx_frames,
            self.attachment_chunks_tx,
            self.attachment_chunks_rx,
            self.suppressed_work,
        ]
        .into_iter()
        .sum()
    }

    /// Stable regression score, not a physical energy estimate. Expensive
    /// wakeups have larger weights so idle simulations can fail early.
    pub fn energy_score(&self) -> u64 {
        self.scheduler_wakeups
            .saturating_add(self.snapshot_builds.saturating_mul(2))
            .saturating_add(self.peer_probes.saturating_mul(20))
            .saturating_add(self.relay_probes.saturating_mul(25))
            .saturating_add(self.ffi_wakes)
            .saturating_add(self.db_reads.saturating_mul(3))
            .saturating_add(self.db_writes.saturating_mul(4))
            .saturating_add(self.blob_writes.saturating_mul(5))
            .saturating_add(self.projection_events.saturating_mul(2))
            .saturating_add(self.radio_wakeups.saturating_mul(20))
            .saturating_add(self.tor_dials.saturating_mul(100))
            .saturating_add(self.relay_dials.saturating_mul(25))
            .saturating_add(self.peer_dials.saturating_mul(50))
            .saturating_add(self.handshakes.saturating_mul(20))
            .saturating_add(self.tx_frames.saturating_mul(2))
            .saturating_add(self.rx_frames.saturating_mul(2))
            .saturating_add(self.attachment_chunks_tx)
            .saturating_add(self.attachment_chunks_rx)
            .saturating_add(self.suppressed_work)
    }
}

/// Platform-owned sample. `None` means the platform cannot provide that
/// field; the application must never infer physical energy from a missing
/// value.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PlatformEnergySample {
    pub battery_percent: Option<u8>,
    pub charging: Option<bool>,
    pub power_saver: Option<bool>,
    pub metered_network: Option<bool>,
    pub process_cpu_ms: Option<u64>,
    pub uid_tx_bytes: Option<u64>,
    pub uid_rx_bytes: Option<u64>,
}

/// Optional platform adapter. Sampling is initiated by lifecycle transitions,
/// diagnostics, or incident collection; it is never driven by a polling loop.
pub trait PlatformEnergyProvider: Send + Sync {
    fn sample(&self) -> PlatformEnergySample;
}

/// Combined redaction-safe snapshot exposed to diagnostics.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BatteryDiagnosticSnapshot {
    pub profile: BatteryProfile,
    pub work: BatterySnapshot,
    pub platform: PlatformEnergySample,
}

/// Why a feature is allowed to wake the runtime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WakeReason {
    UserVisible,
    DurableDelivery,
    AttachmentTransfer,
    Pairing,
    Radio,
    NetworkChanged,
    Diagnostic,
    Scheduler,
    PolicySuppressed,
}

/// One redaction-safe transition in the in-memory battery ledger.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BatteryEvent {
    pub at: Instant,
    pub metric: BatteryMetric,
    pub amount: u64,
    pub reason: WakeReason,
}

/// Thread-safe aggregator suitable for cheap clones in feature executors.
#[derive(Clone, Default)]
pub struct BatteryLedger {
    inner: Arc<Mutex<LedgerState>>,
}

#[derive(Default)]
struct LedgerState {
    snapshot: BatterySnapshot,
    recent: VecDeque<BatteryEvent>,
    platform: PlatformEnergySample,
}

impl BatteryLedger {
    /// Creates an empty ledger.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records an aggregated metric. The recent transition buffer is bounded.
    pub fn record(&self, metric: BatteryMetric, amount: u64, reason: WakeReason) {
        let Ok(mut state) = self.inner.lock() else { return };
        increment(&mut state.snapshot, metric, amount);
        state.recent.push_back(BatteryEvent { at: Instant::now(), metric, amount, reason });
        if state.recent.len() > 128 {
            state.recent.pop_front();
        }
    }

    /// Records work rejected by the policy. Keeping this separate from
    /// completed work makes diagnostics explain why a feature was delayed.
    pub fn record_suppressed(&self, amount: u64) {
        self.record(BatteryMetric::SuppressedWork, amount, WakeReason::PolicySuppressed);
    }

    /// Returns a copy without exposing mutable internal state.
    pub fn snapshot(&self) -> BatterySnapshot {
        self.inner.lock().map(|state| state.snapshot).unwrap_or_default()
    }

    /// Returns the last bounded transitions for a diagnostics view.
    pub fn recent(&self) -> Vec<BatteryEvent> {
        self.inner.lock().map(|state| state.recent.iter().copied().collect()).unwrap_or_default()
    }

    /// Stores the latest event-triggered platform sample.
    pub fn set_platform_sample(&self, sample: PlatformEnergySample) {
        if let Ok(mut state) = self.inner.lock() {
            state.platform = sample;
        }
    }

    pub fn platform_sample(&self) -> PlatformEnergySample {
        self.inner.lock().map(|state| state.platform).unwrap_or_default()
    }

    pub fn diagnostic_snapshot(&self, profile: BatteryProfile) -> BatteryDiagnosticSnapshot {
        BatteryDiagnosticSnapshot {
            profile,
            work: self.snapshot(),
            platform: self.platform_sample(),
        }
    }

    /// Returns an operation span that records elapsed work without a timer.
    pub fn begin(&self, metric: BatteryMetric, reason: WakeReason) -> BatterySpan {
        BatterySpan {
            ledger: self.clone(),
            metric,
            reason,
            started: Instant::now(),
            finished: false,
        }
    }

    /// Clears counters after an explicit diagnostic baseline capture.
    pub fn reset(&self) {
        if let Ok(mut state) = self.inner.lock() {
            state.snapshot = BatterySnapshot::default();
            state.recent.clear();
            state.platform = PlatformEnergySample::default();
        }
    }
}

/// RAII operation accounting. Duration is intentionally not persisted here;
/// callers can add a separate metric if they need a duration histogram.
pub struct BatterySpan {
    ledger: BatteryLedger,
    metric: BatteryMetric,
    reason: WakeReason,
    started: Instant,
    finished: bool,
}

impl BatterySpan {
    /// Finishes the span and records one completed operation.
    pub fn finish(mut self) -> Duration {
        self.finished = true;
        let elapsed = self.started.elapsed();
        self.ledger.record(self.metric, 1, self.reason);
        elapsed
    }
}

impl Drop for BatterySpan {
    fn drop(&mut self) {
        if !self.finished {
            self.ledger.record(self.metric, 1, self.reason);
        }
    }
}

fn increment(snapshot: &mut BatterySnapshot, metric: BatteryMetric, amount: u64) {
    let target = match metric {
        BatteryMetric::SchedulerWakeup => &mut snapshot.scheduler_wakeups,
        BatteryMetric::SnapshotBuild => &mut snapshot.snapshot_builds,
        BatteryMetric::PeerProbe => &mut snapshot.peer_probes,
        BatteryMetric::RelayProbe => &mut snapshot.relay_probes,
        BatteryMetric::FfiWake => &mut snapshot.ffi_wakes,
        BatteryMetric::DbRead => &mut snapshot.db_reads,
        BatteryMetric::DbWrite => &mut snapshot.db_writes,
        BatteryMetric::BlobWrite => &mut snapshot.blob_writes,
        BatteryMetric::ProjectionEvent => &mut snapshot.projection_events,
        BatteryMetric::RadioWake => &mut snapshot.radio_wakeups,
        BatteryMetric::TorDial => &mut snapshot.tor_dials,
        BatteryMetric::RelayDial => &mut snapshot.relay_dials,
        BatteryMetric::PeerDial => &mut snapshot.peer_dials,
        BatteryMetric::Handshake => &mut snapshot.handshakes,
        BatteryMetric::TxFrame => &mut snapshot.tx_frames,
        BatteryMetric::RxFrame => &mut snapshot.rx_frames,
        BatteryMetric::AttachmentChunkTx => &mut snapshot.attachment_chunks_tx,
        BatteryMetric::AttachmentChunkRx => &mut snapshot.attachment_chunks_rx,
        BatteryMetric::SuppressedWork => &mut snapshot.suppressed_work,
    };
    *target = target.saturating_add(amount);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_ledger_has_no_work() {
        assert_eq!(BatteryLedger::new().snapshot().total_work(), 0);
        assert_eq!(BatteryLedger::new().snapshot().energy_score(), 0);
    }

    #[test]
    fn span_records_once_even_when_dropped() {
        let ledger = BatteryLedger::new();
        {
            let _span = ledger.begin(BatteryMetric::PeerDial, WakeReason::DurableDelivery);
        }
        assert_eq!(ledger.snapshot().peer_dials, 1);
    }

    #[test]
    fn finished_span_is_not_double_counted() {
        let ledger = BatteryLedger::new();
        let span = ledger.begin(BatteryMetric::TxFrame, WakeReason::Radio);
        let _ = span.finish();
        assert_eq!(ledger.snapshot().tx_frames, 1);
    }

    #[test]
    fn foreground_does_not_escalate_automatic_policy_globally() {
        let system = SystemEnergyState::default().with_foreground(true);
        let policy = BatteryPreferences::default().effective(system, false);
        assert_eq!(policy.profile, BatteryProfile::Balanced);
        assert_eq!(policy.reason, PolicyOverrideReason::ForegroundActivity);
        assert!(!policy.tor_dormancy_allowed);
    }

    #[test]
    fn recent_events_are_bounded() {
        let ledger = BatteryLedger::new();
        for _ in 0..256 {
            ledger.record(BatteryMetric::FfiWake, 1, WakeReason::Scheduler);
        }
        assert_eq!(ledger.recent().len(), 128);
    }

    #[test]
    fn battery_saver_only_allows_explicit_diagnostics_for_cosmetic_work() {
        let policy = BatteryPolicy::new(BatteryProfile::BatterySaver);
        assert!(!policy.allows_cosmetic_work(true, false));
        assert!(policy.allows_cosmetic_work(false, true));
        assert!(policy.allows_reliable_work());
    }

    #[test]
    fn attachment_policy_pauses_only_expensive_metered_work() {
        let policy = BatteryPolicy::new(BatteryProfile::Balanced)
            .with_metered_transfers(MeteredTransferPolicy::PauseLarge);
        assert_eq!(policy.attachment_decision(64 * 1024, true), TransferDecision::Allow);
        assert_eq!(
            policy.attachment_decision(6 * 1024 * 1024, true),
            TransferDecision::PauseMetered
        );
        assert_eq!(policy.attachment_decision(6 * 1024 * 1024, false), TransferDecision::Allow);
    }

    #[test]
    fn suppressed_work_is_visible_in_ledger() {
        let ledger = BatteryLedger::new();
        ledger.record(BatteryMetric::SuppressedWork, 3, WakeReason::PolicySuppressed);
        assert_eq!(ledger.snapshot().suppressed_work, 3);
    }

    #[test]
    fn default_automatic_mode_does_not_schedule_background_rendezvous() {
        let preferences = BatteryPreferences::default();
        let effective = preferences.effective(
            SystemEnergyState {
                foreground: false,
                power_saver: Some(false),
                battery_percent: Some(60),
                ..SystemEnergyState::default()
            },
            false,
        );
        assert_eq!(effective.profile, BatteryProfile::Balanced);
        assert!(effective.tor_dormancy_allowed);
        assert_eq!(effective.background_sync, BackgroundSyncCadence::OnOpen);
    }

    #[test]
    fn platform_power_saver_suppresses_discretionary_balanced_work() {
        let effective = BatteryPreferences::default().effective(
            SystemEnergyState { power_saver: Some(true), ..SystemEnergyState::default() },
            false,
        );
        assert_eq!(effective.profile, BatteryProfile::BatterySaver);
        assert_eq!(effective.reason, PolicyOverrideReason::PowerSaver);
    }

    #[test]
    fn delayed_background_delivery_allows_dormancy_without_periodic_cadence() {
        let preferences = BatteryPreferences {
            mode: RequestedBatteryMode::Balanced,
            background_sync: BackgroundSyncCadence::OnOpen,
            allow_delayed_background_delivery: true,
            ..BatteryPreferences::default()
        };
        let effective = preferences.effective(SystemEnergyState::default(), false);
        assert!(effective.tor_dormancy_allowed);
        assert_eq!(effective.background_sync, BackgroundSyncCadence::OnOpen);
    }

    #[test]
    fn durable_work_does_not_globally_promote_battery_profile() {
        let preferences = BatteryPreferences {
            mode: RequestedBatteryMode::BatterySaver,
            background_sync: BackgroundSyncCadence::OnOpen,
            allow_delayed_background_delivery: true,
            ..BatteryPreferences::default()
        };
        let effective = preferences.effective(SystemEnergyState::default(), false);
        assert_eq!(effective.profile, BatteryProfile::BatterySaver);
        assert_eq!(effective.reason, PolicyOverrideReason::UserPreference);
        assert!(effective.tor_dormancy_allowed);
    }

    #[test]
    fn preferences_wire_round_trip_is_stable() {
        let original = BatteryPreferences {
            mode: RequestedBatteryMode::BatterySaver,
            background_sync: BackgroundSyncCadence::OnOpen,
            allow_delayed_background_delivery: true,
            metered_transfers: MeteredTransferPolicy::PauseAll,
            visual_activity: VisualActivityPolicy::FocusedOnly,
        };
        let (mode, sync, delayed, metered, visual) = original.wire();
        let migrated = BatteryPreferences::from_wire(mode, sync, delayed, metered, visual);
        assert_eq!(migrated.mode, original.mode);
        assert_eq!(migrated.background_sync, BackgroundSyncCadence::OnOpen);
        assert_eq!(migrated.metered_transfers, original.metered_transfers);
        assert_eq!(migrated.visual_activity, original.visual_activity);
    }
}
