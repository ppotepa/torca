//! Bounded, redaction-safe workload accounting for diagnostics.
//!
//! This module deliberately records only counters and host samples. It owns
//! neither timers nor network policy; RuntimeOwner owns scheduling and
//! `torca-runtime-policy` owns admission decisions.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use torca_runtime_policy::BatteryProfile;

/// A bounded abstract work metric. Values are counts, not physical energy.
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

/// Compact counter snapshot exposed to the debug console and incident bundle.
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

    /// Stable regression score, not a physical energy estimate.
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

/// Platform-owned sample. Missing values are unknown, never inferred.
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

/// Event-triggered platform adapter; it must not create a polling loop.
pub trait PlatformEnergyProvider: Send + Sync {
    fn sample(&self) -> PlatformEnergySample;
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BatteryDiagnosticSnapshot {
    pub profile: BatteryProfile,
    pub work: BatterySnapshot,
    pub platform: PlatformEnergySample,
}

/// Why a feature was allowed to wake the runtime.
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
pub struct BatteryEvent {
    pub at: Instant,
    pub metric: BatteryMetric,
    pub amount: u64,
    pub reason: WakeReason,
}

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
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&self, metric: BatteryMetric, amount: u64, reason: WakeReason) {
        let Ok(mut state) = self.inner.lock() else { return };
        increment(&mut state.snapshot, metric, amount);
        state.recent.push_back(BatteryEvent { at: Instant::now(), metric, amount, reason });
        if state.recent.len() > 128 {
            state.recent.pop_front();
        }
    }

    pub fn record_suppressed(&self, amount: u64) {
        self.record(BatteryMetric::SuppressedWork, amount, WakeReason::PolicySuppressed);
    }

    pub fn snapshot(&self) -> BatterySnapshot {
        self.inner.lock().map(|state| state.snapshot).unwrap_or_default()
    }

    pub fn recent(&self) -> Vec<BatteryEvent> {
        self.inner.lock().map(|state| state.recent.iter().copied().collect()).unwrap_or_default()
    }

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

    pub fn begin(&self, metric: BatteryMetric, reason: WakeReason) -> BatterySpan {
        BatterySpan {
            ledger: self.clone(),
            metric,
            reason,
            started: Instant::now(),
            finished: false,
        }
    }

    pub fn reset(&self) {
        if let Ok(mut state) = self.inner.lock() {
            state.snapshot = BatterySnapshot::default();
            state.recent.clear();
            state.platform = PlatformEnergySample::default();
        }
    }
}

pub struct BatterySpan {
    ledger: BatteryLedger,
    metric: BatteryMetric,
    reason: WakeReason,
    started: Instant,
    finished: bool,
}

impl BatterySpan {
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
