//! Single background owner for Tor, pairing, peer sessions and durable delivery.

mod attachments;
pub use attachments::{AttachmentSendRequest, AttachmentView};

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, SyncSender, TrySendError};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use torca_attachments::AttachmentId;
use torca_battery::{BatteryMetric, WakeReason};
use torca_client_engine::{EngineCommand, EngineHandle};
use torca_connectivity::{
    ConnectivityObserver, ConnectivitySnapshot, PeerProbeCandidate, PeerProbeSupervisor,
    RelayHealthHandle, RelayHealthPort, RelayHealthSnapshot, RelayHealthWorker,
};
use torca_contacts::ContactId;
use torca_conversations::ConversationId;
use torca_delivery::ReactionPayload;
use torca_diagnostics::{
    Component, DiagnosticBuffer, DiagnosticCode, DiagnosticEvent, HealthState, RuntimeCounter,
    RuntimeWakeSource,
};
use torca_foundation::{
    ClassifiedError, ErrorCategory, ErrorCode, ErrorDescriptor, OpaqueId, RetryAdvice, Timestamp,
};
use torca_messaging::{MessageBody, MessageId, MessageStatus};
use torca_pairing::{PairingCode, PairingSessionId};
use torca_probing::{ProbeKind, ProbeResult, ProbeStatus, ProbeSupervisor, ProbeTarget};
use torca_runtime_policy::{
    AttentionContext, AttentionSurface, DemandReason, EvidenceKind, PolicyEvent, ResourceScope,
    RuntimeGovernor, WorkClass, WorkDemand,
};
use torca_runtime_policy::{
    BatteryPolicy, BatteryPreferences, BatteryProfile, MeteredTransferPolicy, SystemEnergyState,
};

// Included runtime files use `thread::spawn` and `thread::yield_now`. Keep that
// compact vocabulary while guaranteeing the process-owned runtime thread has a
// stable diagnostic name.
mod thread {
    pub use std::thread::yield_now;

    pub fn spawn<F, T>(f: F) -> std::thread::JoinHandle<T>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        std::thread::Builder::new()
            .name("torca-runtime-owner".into())
            .spawn(f)
            .expect("spawn Torca runtime owner")
    }
}

const COMMAND_WAIT: Duration = Duration::from_secs(10);
const QUERY_WAIT: Duration = Duration::from_secs(5);
const SHUTDOWN_WAIT: Duration = Duration::from_secs(15);
const MAILBOX_CAPACITY: usize = 256;
const ENQUEUE_WAIT: Duration = Duration::from_secs(2);

include!("actor/model.rs");
include!("actor/ports.rs");
include!("actor/mailbox.rs");
include!("actor/state.rs");
include!("actor/owner.rs");
include!("actor/maintenance.rs");
include!("actor/scheduler.rs");
include!("actor/command_dispatch.rs");
include!("actor/support.rs");
