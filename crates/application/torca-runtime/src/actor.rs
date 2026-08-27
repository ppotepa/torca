//! Single background owner for provider lifecycle, pairing, peer sessions and durable delivery.

mod attachments;
pub use attachments::{AttachmentSendRequest, AttachmentView};

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, SyncSender, TrySendError};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use torca_attachments::AttachmentId;
use torca_client_engine::{EngineCommand, EngineHandle};
use torca_connectivity::{
    ConnectivityObserver, ConnectivitySnapshot, PeerProbeCandidate, PeerProbeSupervisor,
    RendezvousHealthHandle, RendezvousHealthPort, RendezvousHealthSnapshot, RendezvousHealthWorker,
};
use torca_contacts::ContactId;
use torca_conversations::ConversationId;
use torca_delivery::ReactionPayload;
use torca_diagnostics::{
    BatteryMetric, Component, DiagnosticBuffer, DiagnosticCode, DiagnosticEvent, HealthState,
    RuntimeCounter, RuntimeWakeSource, WakeReason,
};
use torca_foundation::{
    ClassifiedError, CommandId, ErrorCategory, ErrorCode, ErrorDescriptor, OpaqueId, RetryAdvice,
    Timestamp,
};
use torca_messaging::{Message, MessageBody, MessageId, MessageStatus};
use torca_pairing::{PairingCode, PairingSessionId};
use torca_pairing_protocol::PairingBootstrapDescriptor;
pub use torca_presence::PeerAvailability;
use torca_probing::{ProbeKind, ProbeResult, ProbeStatus, ProbeSupervisor, ProbeTarget};
use torca_runtime_policy::{
    AttentionContext, AttentionSurface, DemandReason, EvidenceKind, PolicyEvent, ResourceScope,
    RuntimeGovernor, WorkClass, WorkDemand,
};
use torca_runtime_policy::{
    BatteryPolicy, BatteryPreferences, BatteryProfile, MeteredTransferPolicy, SystemEnergyState,
};

// Included runtime files use `thread::spawn` and `thread::sleep`. Keep that
// compact vocabulary while guaranteeing the process-owned runtime thread has a
// stable diagnostic name.
mod thread {
    pub use std::thread::sleep;

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
