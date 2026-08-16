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
    AttentionContext, AttentionSurface, ConnectionLease, DemandReason, EvidenceKind, FocusLease,
    Freshness, PolicyEvent, ResourceScope, RuntimeEventHub, RuntimeEventHubStats, RuntimeGovernor,
    RuntimePolicySnapshot, WorkClass, WorkDemand, WorkPermit,
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
}

/// Runtime behavior profile. Policy profiles never weaken durable delivery.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum BatteryProfile {
    #[default]
    AlwaysAvailable,
    Balanced,
    BatterySaver,
    Diagnostics,
}

/// User-facing availability preference.  This is deliberately separate from
/// [`BatteryProfile`]: `Diagnostics` is a temporary runtime override, while
/// this enum is safe to persist and expose in settings.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RequestedBatteryMode {
    #[default]
    Automatic,
    AlwaysAvailable,
    Balanced,
    BatterySaver,
}

/// User-visible promise for background delivery.  The durations are hints;
/// Android Doze and Windows power policy may defer a wake beyond them.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum BackgroundSyncCadence {
    #[default]
    Instant,
    FifteenMinutes,
    ThirtyMinutes,
    Hourly,
    TwoHours,
    OnOpen,
}

impl BackgroundSyncCadence {
    pub fn approximate_interval(self) -> Option<Duration> {
        match self {
            Self::Instant => None,
            Self::FifteenMinutes => Some(Duration::from_secs(15 * 60)),
            Self::ThirtyMinutes => Some(Duration::from_secs(30 * 60)),
            Self::Hourly => Some(Duration::from_secs(60 * 60)),
            Self::TwoHours => Some(Duration::from_secs(2 * 60 * 60)),
            Self::OnOpen => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MeteredTransferPolicy {
    AllowAll,
    #[default]
    PauseLarge,
    PauseAll,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum VisualActivityPolicy {
    Full,
    FocusedOnly,
    Static,
    #[default]
    FollowSystem,
}

/// Durable user preference.  It contains intent, not executor-specific
/// timers; runtime policy derives deadlines from it and current system state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BatteryPreferences {
    pub mode: RequestedBatteryMode,
    pub background_sync: BackgroundSyncCadence,
    pub allow_delayed_background_delivery: bool,
    pub metered_transfers: MeteredTransferPolicy,
    pub visual_activity: VisualActivityPolicy,
}

impl Default for BatteryPreferences {
    fn default() -> Self {
        Self {
            mode: RequestedBatteryMode::Automatic,
            background_sync: BackgroundSyncCadence::Instant,
            allow_delayed_background_delivery: false,
            metered_transfers: MeteredTransferPolicy::PauseLarge,
            visual_activity: VisualActivityPolicy::FollowSystem,
        }
    }
}

/// Event-driven platform state. `None` means the host cannot provide the
/// field; policy must not infer a physical battery state from missing data.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SystemEnergyState {
    pub foreground: bool,
    pub charging: Option<bool>,
    pub battery_percent: Option<u8>,
    pub power_saver: Option<bool>,
    pub metered_network: Option<bool>,
    pub validated_network: Option<bool>,
    pub display_visible: Option<bool>,
    pub data_stall_suspected: bool,
}

impl SystemEnergyState {
    pub fn foreground(&self) -> bool {
        self.foreground
    }
    pub fn with_foreground(mut self, value: bool) -> Self {
        self.foreground = value;
        self
    }
    pub fn with_charging(mut self, value: Option<bool>) -> Self {
        self.charging = value;
        self
    }
    pub fn with_power_saver(mut self, value: Option<bool>) -> Self {
        self.power_saver = value;
        self
    }
    pub fn with_metered_network(mut self, value: Option<bool>) -> Self {
        self.metered_network = value;
        self
    }
    pub fn with_validated_network(mut self, value: Option<bool>) -> Self {
        self.validated_network = value;
        self
    }
    pub fn with_data_stall(mut self, value: bool) -> Self {
        self.data_stall_suspected = value;
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PolicyOverrideReason {
    UserPreference,
    ForegroundActivity,
    Charging,
    PowerSaver,
    CriticalBattery,
    DurableLease,
    Diagnostics,
    NetworkStall,
}

/// Result of reducing user intent, platform state and runtime demand.  The
/// executors consume this value; the battery crate never opens sockets or
/// changes Tor itself.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EffectiveBatteryPolicy {
    pub profile: BatteryProfile,
    pub reason: PolicyOverrideReason,
    pub tor_dormancy_allowed: bool,
    pub background_sync: BackgroundSyncCadence,
    pub metered_transfers: MeteredTransferPolicy,
    pub visual_activity: VisualActivityPolicy,
}

impl BatteryPreferences {
    pub fn from_wire(
        mode: &str,
        background_sync: &str,
        allow_delayed_background_delivery: bool,
        metered_transfers: &str,
        visual_activity: &str,
    ) -> Self {
        Self {
            mode: RequestedBatteryMode::from_wire(mode),
            background_sync: BackgroundSyncCadence::from_wire(background_sync),
            allow_delayed_background_delivery,
            metered_transfers: MeteredTransferPolicy::from_wire(metered_transfers),
            visual_activity: VisualActivityPolicy::from_wire(visual_activity),
        }
    }

    pub fn wire(self) -> (&'static str, &'static str, bool, &'static str, &'static str) {
        (
            self.mode.wire(),
            self.background_sync.wire(),
            self.allow_delayed_background_delivery,
            self.metered_transfers.wire(),
            self.visual_activity.wire(),
        )
    }

    pub fn effective(
        self,
        system: SystemEnergyState,
        has_critical_lease: bool,
        diagnostics_override: bool,
    ) -> EffectiveBatteryPolicy {
        let reason = if diagnostics_override {
            PolicyOverrideReason::Diagnostics
        } else if has_critical_lease {
            PolicyOverrideReason::DurableLease
        } else if system.foreground {
            PolicyOverrideReason::ForegroundActivity
        } else if system.charging == Some(true) {
            PolicyOverrideReason::Charging
        } else if system.power_saver == Some(true) {
            PolicyOverrideReason::PowerSaver
        } else if system.battery_percent.is_some_and(|value| value <= 15) {
            PolicyOverrideReason::CriticalBattery
        } else {
            PolicyOverrideReason::UserPreference
        };

        let profile = if diagnostics_override || has_critical_lease || system.foreground {
            BatteryProfile::AlwaysAvailable
        } else {
            match self.mode {
                RequestedBatteryMode::AlwaysAvailable => BatteryProfile::AlwaysAvailable,
                RequestedBatteryMode::Balanced => BatteryProfile::Balanced,
                RequestedBatteryMode::BatterySaver => BatteryProfile::BatterySaver,
                RequestedBatteryMode::Automatic => {
                    if system.power_saver == Some(true)
                        || system.battery_percent.is_some_and(|value| value <= 20)
                    {
                        BatteryProfile::BatterySaver
                    } else {
                        BatteryProfile::Balanced
                    }
                }
            }
        };

        let tor_dormancy_allowed = !diagnostics_override
            && !has_critical_lease
            && !system.foreground
            && self.allow_delayed_background_delivery
            && self.background_sync != BackgroundSyncCadence::Instant
            && self.background_sync != BackgroundSyncCadence::OnOpen;

        EffectiveBatteryPolicy {
            profile,
            reason,
            tor_dormancy_allowed,
            background_sync: self.background_sync,
            metered_transfers: self.metered_transfers,
            visual_activity: self.visual_activity,
        }
    }
}

impl RequestedBatteryMode {
    pub fn from_wire(value: &str) -> Self {
        match value {
            "always_available" => Self::AlwaysAvailable,
            "balanced" => Self::Balanced,
            "battery_saver" => Self::BatterySaver,
            _ => Self::Automatic,
        }
    }

    pub fn wire(self) -> &'static str {
        match self {
            Self::Automatic => "automatic",
            Self::AlwaysAvailable => "always_available",
            Self::Balanced => "balanced",
            Self::BatterySaver => "battery_saver",
        }
    }
}

impl BackgroundSyncCadence {
    pub fn from_wire(value: &str) -> Self {
        match value {
            "fifteen_minutes" => Self::FifteenMinutes,
            "thirty_minutes" => Self::ThirtyMinutes,
            "hourly" => Self::Hourly,
            "two_hours" => Self::TwoHours,
            "on_open" => Self::OnOpen,
            _ => Self::Instant,
        }
    }

    pub fn wire(self) -> &'static str {
        match self {
            Self::Instant => "instant",
            Self::FifteenMinutes => "fifteen_minutes",
            Self::ThirtyMinutes => "thirty_minutes",
            Self::Hourly => "hourly",
            Self::TwoHours => "two_hours",
            Self::OnOpen => "on_open",
        }
    }
}

impl MeteredTransferPolicy {
    pub fn from_wire(value: &str) -> Self {
        match value {
            "allow_all" => Self::AllowAll,
            "pause_all" => Self::PauseAll,
            _ => Self::PauseLarge,
        }
    }

    pub fn wire(self) -> &'static str {
        match self {
            Self::AllowAll => "allow_all",
            Self::PauseLarge => "pause_large",
            Self::PauseAll => "pause_all",
        }
    }
}

impl VisualActivityPolicy {
    pub fn from_wire(value: &str) -> Self {
        match value {
            "full" => Self::Full,
            "focused_only" => Self::FocusedOnly,
            "static" => Self::Static,
            _ => Self::FollowSystem,
        }
    }

    pub fn wire(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::FocusedOnly => "focused_only",
            Self::Static => "static",
            Self::FollowSystem => "follow_system",
        }
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransferDecision {
    Allow,
    PauseMetered,
    PauseBatterySaver,
}

/// Small policy facade used by executors for discretionary work. Reliable
/// delivery must continue even when this facade denies cosmetic work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BatteryPolicy {
    profile: BatteryProfile,
    metered_transfers: MeteredTransferPolicy,
}

impl BatteryPolicy {
    pub fn new(profile: BatteryProfile) -> Self {
        Self { profile, metered_transfers: MeteredTransferPolicy::PauseLarge }
    }

    pub fn profile(self) -> BatteryProfile {
        self.profile
    }

    pub fn set_profile(&mut self, profile: BatteryProfile) {
        self.profile = profile;
    }

    pub fn with_metered_transfers(mut self, policy: MeteredTransferPolicy) -> Self {
        self.metered_transfers = policy;
        self
    }

    /// Central admission decision for attachments. Reliable text/control
    /// traffic is intentionally outside this method and is never suppressed.
    pub fn attachment_decision(self, bytes: u64, metered: bool) -> TransferDecision {
        if self.profile == BatteryProfile::BatterySaver && bytes > 256 * 1024 {
            return TransferDecision::PauseBatterySaver;
        }
        if !metered {
            return TransferDecision::Allow;
        }
        match self.metered_transfers {
            MeteredTransferPolicy::AllowAll => TransferDecision::Allow,
            MeteredTransferPolicy::PauseLarge if bytes > 5 * 1024 * 1024 => {
                TransferDecision::PauseMetered
            }
            MeteredTransferPolicy::PauseAll => TransferDecision::PauseMetered,
            MeteredTransferPolicy::PauseLarge => TransferDecision::Allow,
        }
    }

    /// Whether an optional cosmetic refresh may run now.
    pub fn allows_cosmetic_work(self, user_visible: bool, explicit_diagnostic: bool) -> bool {
        match self.profile {
            BatteryProfile::AlwaysAvailable | BatteryProfile::Diagnostics => {
                user_visible || explicit_diagnostic
            }
            BatteryProfile::Balanced => user_visible || explicit_diagnostic,
            BatteryProfile::BatterySaver => explicit_diagnostic,
        }
    }

    /// Durable work is never suppressed by battery policy.
    pub fn allows_reliable_work(self) -> bool {
        true
    }
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
    fn automatic_mode_is_reliability_first_until_delayed_delivery_is_explicit() {
        let preferences = BatteryPreferences::default();
        let effective = preferences.effective(
            SystemEnergyState {
                foreground: false,
                power_saver: Some(true),
                battery_percent: Some(12),
                ..SystemEnergyState::default()
            },
            false,
            false,
        );
        assert_eq!(effective.profile, BatteryProfile::BatterySaver);
        assert!(!effective.tor_dormancy_allowed);
    }

    #[test]
    fn delayed_background_delivery_allows_dormancy_without_active_work() {
        let preferences = BatteryPreferences {
            mode: RequestedBatteryMode::Balanced,
            background_sync: BackgroundSyncCadence::ThirtyMinutes,
            allow_delayed_background_delivery: true,
            ..BatteryPreferences::default()
        };
        let effective = preferences.effective(SystemEnergyState::default(), false, false);
        assert!(effective.tor_dormancy_allowed);
        assert_eq!(
            effective.background_sync.approximate_interval(),
            Some(Duration::from_secs(1800))
        );
    }

    #[test]
    fn critical_lease_keeps_tor_active_even_in_saver_mode() {
        let preferences = BatteryPreferences {
            mode: RequestedBatteryMode::BatterySaver,
            background_sync: BackgroundSyncCadence::OnOpen,
            allow_delayed_background_delivery: true,
            ..BatteryPreferences::default()
        };
        let effective = preferences.effective(SystemEnergyState::default(), true, false);
        assert_eq!(effective.profile, BatteryProfile::AlwaysAvailable);
        assert_eq!(effective.reason, PolicyOverrideReason::DurableLease);
        assert!(!effective.tor_dormancy_allowed);
    }

    #[test]
    fn preferences_wire_round_trip_is_stable() {
        let original = BatteryPreferences {
            mode: RequestedBatteryMode::BatterySaver,
            background_sync: BackgroundSyncCadence::Hourly,
            allow_delayed_background_delivery: true,
            metered_transfers: MeteredTransferPolicy::PauseAll,
            visual_activity: VisualActivityPolicy::FocusedOnly,
        };
        let (mode, sync, delayed, metered, visual) = original.wire();
        assert_eq!(BatteryPreferences::from_wire(mode, sync, delayed, metered, visual), original);
    }
}
