//! Single background owner for Tor, pairing, peer sessions and durable delivery.

mod attachments;
pub use attachments::{AttachmentSendRequest, AttachmentView};

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, SyncSender, TrySendError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use torca_attachments::AttachmentId;
use torca_battery::{
    AttentionContext, DemandReason, PolicyEvent, ResourceScope, RuntimeGovernor, WorkClass,
    WorkDemand,
};
use torca_battery::{
    BatteryMetric, BatteryPolicy, BatteryProfile, MeteredTransferPolicy, WakeReason,
};
use torca_client_engine::{EngineCommand, EngineHandle};
use torca_connectivity::{
    ConnectivityObserver, ConnectivitySnapshot, PeerProbeCandidate, PeerProbeSupervisor,
    RelayHealthHandle, RelayHealthPort, RelayHealthSnapshot, RelayHealthWorker,
};
use torca_contacts::{ContactId, ContactStatus};
use torca_conversations::ConversationId;
use torca_delivery::ReactionPayload;
use torca_diagnostics::{
    Component, DiagnosticBuffer, DiagnosticCode, DiagnosticEvent, HealthState, RuntimeCounter,
};
use torca_foundation::{
    ClassifiedError, ErrorCategory, ErrorCode, ErrorDescriptor, OpaqueId, RetryAdvice, Timestamp,
};
use torca_messaging::{MessageBody, MessageId, MessageStatus};
use torca_pairing::{PairingCode, PairingSessionId};
use torca_probing::{ProbeKind, ProbeResult, ProbeStatus, ProbeSupervisor, ProbeTarget};

const COMMAND_WAIT: Duration = Duration::from_secs(10);
const QUERY_WAIT: Duration = Duration::from_secs(5);
const SHUTDOWN_WAIT: Duration = Duration::from_secs(15);
const MAILBOX_CAPACITY: usize = 256;
const ENQUEUE_WAIT: Duration = Duration::from_secs(2);

// The runtime stays in one module namespace so the public API and private
// ownership boundaries remain unchanged. Each included file owns one runtime
// responsibility and can evolve independently without growing this root.
include!("actor/model.rs");
include!("actor/ports.rs");
include!("actor/mailbox.rs");
include!("actor/owner.rs");
include!("actor/scheduler.rs");
include!("actor/command_dispatch.rs");
include!("actor/support.rs");
