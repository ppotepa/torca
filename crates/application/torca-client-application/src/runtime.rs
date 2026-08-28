use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::PathBuf;
use std::sync::Mutex;

use base64::Engine as _;

use torca_attachments::AttachmentId;
use torca_bootstrap::{BootstrapSnapshot, BootstrapState, BootstrapStepId, BootstrapStepState};
use torca_contacts::ContactId;
use torca_conversations::ConversationId;
use torca_delivery::ReactionPayload;
use torca_foundation::{
    ClassifiedError, CommandId, ErrorCategory, ErrorCode, ErrorDescriptor, OpaqueId, RetryAdvice,
    Timestamp,
};
use torca_identity::{IdentityId, ProfileName};
use torca_messaging::{MessageBody, MessageId, MessageReaction, ReplyReference};
use torca_pairing::{PairingCode, PairingSessionId, PairingState};
use torca_pairing_protocol::PairingBootstrapDescriptor;
use torca_radio_coordinator::{HostRadioLifecycle, RadioProjection, SharedRadioCoordinator};
use torca_runtime_policy::AttentionContext;
use torca_runtime_policy::{BatteryPreferences, SystemEnergyState};

use crate::{
    ApplicationReadModels, ApplicationSnapshotContext, AttachmentSendRequest,
    ClientApplicationHandle, CommunicationState, EngineCommand, EngineError, EngineResult,
    InMemoryPendingOperationStore, NetworkSnapshot, PendingOperation, PendingOperationKind,
    PendingOperationStore, RuntimeDriverError, RuntimeHandle, pending_operation_id,
};

fn parse_avatar_envelope(json: &str) -> Result<torca_client_engine::AvatarGenomeRecord, String> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|_| "invalid avatar envelope".to_owned())?;
    let encoded = value
        .get("compressedGenome")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "avatar envelope payload missing".to_owned())?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|_| "avatar envelope payload is not valid base64".to_owned())?;
    if bytes.is_empty() || bytes.len() > 32 * 1024 {
        return Err("avatar envelope payload exceeds 32 KiB".into());
    }
    let hash_hex = value
        .get("genomeHash")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "avatar genome hash missing".to_owned())?;
    if hash_hex.len() != 64 {
        return Err("avatar genome hash has invalid length".into());
    }
    let mut hash = [0_u8; 32];
    for (index, chunk) in hash_hex.as_bytes().chunks_exact(2).enumerate() {
        hash[index] = u8::from_str_radix(std::str::from_utf8(chunk).unwrap_or("00"), 16)
            .map_err(|_| "avatar genome hash is not hexadecimal".to_owned())?;
    }
    Ok(torca_client_engine::AvatarGenomeRecord {
        genome_hash: hash,
        schema_version: value.get("schema").and_then(serde_json::Value::as_u64).unwrap_or(1) as u8,
        generator_version: value
            .get("generatorVersion")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        catalog_version: value
            .get("catalogVersion")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        compressed_genome: bytes,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ApplicationCommand {
    SetAttention {
        context: AttentionContext,
    },
    SetNotifications {
        enabled: bool,
    },
    SetReadReceipts {
        enabled: bool,
    },
    SetBatteryPreferences {
        mode: String,
        background_sync: String,
        allow_delayed_background_delivery: bool,
        metered_transfers: String,
        visual_activity: String,
    },
    SetContactAvailability {
        contact_id: OpaqueId,
        mode: String,
    },
    AcknowledgeNewContacts,
    UpdateProfile {
        display_name: String,
        avatar_envelope_json: Option<String>,
        at_ms: i64,
    },
    CreatePairing {
        session_id: OpaqueId,
    },
    JoinPairing {
        session_id: OpaqueId,
        code: String,
        ticket: Option<[u8; 16]>,
        bootstrap: Option<PairingBootstrapDescriptor>,
    },
    ApprovePairing {
        session_id: OpaqueId,
    },
    RejectPairing {
        session_id: OpaqueId,
    },
    CancelPairing {
        session_id: OpaqueId,
    },
    RenameContact {
        contact_id: OpaqueId,
        display_name: String,
    },
    VerifyContact {
        contact_id: OpaqueId,
    },
    ResetContactVerification {
        contact_id: OpaqueId,
    },
    BlockContact {
        contact_id: OpaqueId,
    },
    UnblockContact {
        contact_id: OpaqueId,
    },
    RemoveContact {
        contact_id: OpaqueId,
    },
    StartConversation {
        contact_id: OpaqueId,
    },
    ClearConversationHistory {
        conversation_id: OpaqueId,
    },
    ArchiveConversation {
        conversation_id: OpaqueId,
        at_ms: i64,
    },
    RestoreConversation {
        conversation_id: OpaqueId,
        at_ms: i64,
    },
    QueueMessage {
        message_id: OpaqueId,
        conversation_id: OpaqueId,
        body: String,
        reply_to_message_id: Option<OpaqueId>,
        at_ms: i64,
    },
    RetryMessage {
        message_id: OpaqueId,
        at_ms: i64,
    },
    CancelMessage {
        message_id: OpaqueId,
        at_ms: i64,
    },
    EditMessage {
        message_id: OpaqueId,
        body: String,
        at_ms: i64,
    },
    SetMessageReaction {
        message_id: OpaqueId,
        conversation_id: OpaqueId,
        actor_id: OpaqueId,
        emoji: String,
        active: bool,
        at_ms: i64,
    },
    MarkConversationRead {
        conversation_id: OpaqueId,
    },
    QueueAttachment {
        attachment_id: OpaqueId,
        message_id: OpaqueId,
        conversation_id: OpaqueId,
        source_path: String,
        preview_source_path: Option<String>,
        name: String,
        media_type: String,
        size: u64,
        at_ms: i64,
    },
    RetryAttachment {
        attachment_id: OpaqueId,
    },
    CancelAttachment {
        attachment_id: OpaqueId,
    },
    ExportAttachment {
        attachment_id: OpaqueId,
        destination_path: String,
    },
    ExportAttachmentPreview {
        attachment_id: OpaqueId,
        destination_path: String,
    },
    SetRadioEnabled {
        contact_id: OpaqueId,
        enabled: bool,
        at_ms: i64,
    },
    ConfigureRadioAudio {
        input_device_id: Option<String>,
        output_device_id: Option<String>,
    },
    BeginRadioTransmission {
        contact_id: OpaqueId,
    },
    EndRadioTransmission {
        contact_id: OpaqueId,
    },
    RefreshProviderRoute,
    RefreshSnapshot,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApplicationCommandResult {
    pub kind: &'static str,
    pub resource_id: Option<OpaqueId>,
    pub invite_uri: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApplicationError {
    message: String,
    descriptor: ErrorDescriptor,
}

impl ApplicationError {
    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            descriptor: ErrorDescriptor::new(
                ErrorCode::new("application.invalid_input"),
                ErrorCategory::InvalidInput,
                RetryAdvice::Never,
            ),
        }
    }

    pub fn operation_failed(message: String) -> Self {
        Self {
            message,
            descriptor: ErrorDescriptor::new(
                ErrorCode::new("application.operation_failed"),
                ErrorCategory::Internal,
                RetryAdvice::Never,
            ),
        }
    }

    /// Returns the non-user-facing cause for structured diagnostics. Bridge
    /// responses still expose only the stable error code; callers must never
    /// render this value directly to users.
    pub fn diagnostic_message(&self) -> &str {
        &self.message
    }
}

impl core::fmt::Display for ApplicationError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(&self.message)
    }
}
impl std::error::Error for ApplicationError {}
impl ClassifiedError for ApplicationError {
    fn descriptor(&self) -> ErrorDescriptor {
        self.descriptor
    }
}

/// Small in-memory guard against accidental message floods.
///
/// It is intentionally not a delivery queue: accepted messages remain
/// durable and retryable. The limiter only protects the local command boundary
/// from double taps, runaway automation and notification floods.
#[derive(Default)]
struct MessageRateLimiter {
    per_conversation: BTreeMap<OpaqueId, VecDeque<i64>>,
    global: VecDeque<i64>,
}

impl MessageRateLimiter {
    const WINDOW_MS: i64 = 1_000;
    const PER_CONVERSATION_LIMIT: usize = 8;
    const GLOBAL_LIMIT: usize = 40;

    fn allow(&mut self, conversation_id: OpaqueId, at_ms: i64) -> bool {
        let cutoff = at_ms.saturating_sub(Self::WINDOW_MS);
        self.global.retain(|value| *value > cutoff);
        let entries = self.per_conversation.entry(conversation_id).or_default();
        entries.retain(|value| *value > cutoff);
        if entries.len() >= Self::PER_CONVERSATION_LIMIT || self.global.len() >= Self::GLOBAL_LIMIT
        {
            return false;
        }
        entries.push_back(at_ms);
        self.global.push_back(at_ms);
        true
    }
}

pub struct ClientApplicationRuntime {
    application: ClientApplicationHandle,
    runtime: Option<RuntimeHandle>,
    radio: Option<SharedRadioCoordinator>,
    bootstrap: Mutex<BootstrapState>,
    read_models: Option<ApplicationReadModels>,
    pending: Mutex<Box<dyn PendingOperationStore>>,
    message_rate_limiter: Mutex<MessageRateLimiter>,
    /// Messages may be queued while the native provider is still being
    /// composed. Keep the wake as desired state and replay it when the
    /// runtime handle is attached; otherwise a fresh profile can remain
    /// locally queued until an unrelated lifecycle event or restart.
    pending_delivery_wakes: Mutex<BTreeSet<OpaqueId>>,
}

impl ClientApplicationRuntime {
    /// Returns the minimal snapshot context required to derive notification
    /// metadata. Unlike `snapshot_context`, this never touches network,
    /// pending-operation or Radio projections.
    pub fn notification_snapshot_context(&self) -> Result<ApplicationSnapshotContext, EngineError> {
        let application = self.application.overview()?;
        let (identity_fingerprint, identity_fingerprints, safety_numbers) =
            ApplicationSnapshotContext::security_projection(&application);
        Ok(ApplicationSnapshotContext {
            application,
            network: stopped_network_snapshot(compiled_provider()),
            attachments: Vec::new(),
            bootstrap: self.bootstrap_snapshot()?,
            identity_fingerprint,
            identity_fingerprints,
            safety_numbers,
            pending_operations: Vec::new(),
            radio: None,
        })
    }

    /// Returns the local content-addressed avatar envelope for an explicit
    /// targeted query. The regular snapshot never contains the compressed
    /// genome payload.
    pub fn avatar_genome_json(&self, identity_id: Option<&str>) -> Result<String, EngineError> {
        let record = if let Some(identity_id) = identity_id.filter(|value| !value.is_empty()) {
            let opaque = identity_id.parse::<OpaqueId>().map_err(|_| EngineError::Identity)?;
            self.application
                .avatar_genome_for_identity(IdentityId::from_opaque(opaque))?
                .ok_or(EngineError::Unavailable)?
        } else {
            self.application.overview()?.avatar_genome.ok_or(EngineError::InvalidState)?
        };
        Ok(serde_json::json!({
            "schema": record.schema_version,
            "generatorVersion": record.generator_version,
            "catalogVersion": record.catalog_version,
            "genomeHash": record.genome_hash.iter().map(|byte| format!("{byte:02x}")).collect::<String>(),
            "compressedGenome": base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(record.compressed_genome),
        }).to_string())
    }

    pub fn new(application: ClientApplicationHandle) -> Self {
        Self {
            application,
            runtime: None,
            radio: None,
            bootstrap: Mutex::new(BootstrapState::new()),
            read_models: None,
            pending: Mutex::new(Box::new(InMemoryPendingOperationStore::default())),
            message_rate_limiter: Mutex::new(MessageRateLimiter::default()),
            pending_delivery_wakes: Mutex::new(BTreeSet::new()),
        }
    }

    pub fn attach_runtime(&mut self, runtime: RuntimeHandle) {
        let replay = self
            .pending_delivery_wakes
            .lock()
            .map(|mut pending| std::mem::take(&mut *pending).into_iter().collect::<Vec<_>>())
            .unwrap_or_default();
        for message_id in replay {
            let at = current_timestamp().unwrap_or(Timestamp::UNIX_EPOCH);
            let _ = runtime.queue_outbound(
                MessageId::from_opaque(message_id),
                CommandId::from_opaque(message_id),
                at,
            );
            wake_delivery_for_message(&runtime, &self.application, message_id);
        }
        self.runtime = Some(runtime);
    }

    fn wake_delivery_or_defer(&self, message_id: OpaqueId) {
        if let Some(runtime) = self.runtime.as_ref() {
            wake_delivery_for_message(runtime, &self.application, message_id);
        } else if let Ok(mut pending) = self.pending_delivery_wakes.lock() {
            pending.insert(message_id);
        }
    }

    fn queue_outbound_or_defer(&self, message_id: OpaqueId, at: Timestamp) {
        if let Some(runtime) = self.runtime.as_ref() {
            if runtime
                .queue_outbound(
                    MessageId::from_opaque(message_id),
                    CommandId::from_opaque(message_id),
                    at,
                )
                .is_err()
                && let Ok(mut pending) = self.pending_delivery_wakes.lock()
            {
                pending.insert(message_id);
            }
            wake_delivery_for_message(runtime, &self.application, message_id);
        } else if let Ok(mut pending) = self.pending_delivery_wakes.lock() {
            pending.insert(message_id);
        }
    }

    pub fn attach_radio(&mut self, radio: SharedRadioCoordinator) {
        self.radio = Some(radio);
    }

    pub fn set_battery_policy_inputs(
        &self,
        preferences: BatteryPreferences,
        system: SystemEnergyState,
    ) -> Result<(), RuntimeDriverError> {
        if let Some(runtime) = &self.runtime {
            runtime.set_battery_policy_inputs(preferences, system)?;
        }
        Ok(())
    }

    pub fn set_foreground(&self, foreground: bool) -> Result<(), RuntimeDriverError> {
        if let Some(runtime) = &self.runtime {
            runtime.set_foreground(foreground)?;
        }
        Ok(())
    }

    pub fn set_instant_contact_demand(
        &self,
        contact_id: ContactId,
        enabled: bool,
    ) -> Result<(), RuntimeDriverError> {
        if let Some(runtime) = &self.runtime {
            runtime.set_instant_contact_demand(contact_id, enabled)?;
        }
        Ok(())
    }

    pub fn radio_lifecycle(&self, lifecycle: HostRadioLifecycle) -> Result<(), ApplicationError> {
        self.radio
            .as_ref()
            .ok_or_else(|| ApplicationError::operation_failed("radio runtime is not ready".into()))?
            .lifecycle(lifecycle)
            .map_err(|error| ApplicationError::operation_failed(error.to_string()))
    }

    pub fn radio_projection(&self) -> Option<RadioProjection> {
        let now = current_timestamp().ok()?;
        self.radio.as_ref()?.projection(now).ok()
    }

    pub fn radio_wake_count(&self) -> u64 {
        self.radio.as_ref().map_or(0, SharedRadioCoordinator::media_wake_count)
    }

    pub fn attach_read_models(&mut self, read_models: ApplicationReadModels) {
        self.read_models = Some(read_models);
    }

    pub fn attach_pending_store(&mut self, store: Box<dyn PendingOperationStore>) {
        self.pending = Mutex::new(store);
    }

    pub fn read_models(&self) -> Option<&ApplicationReadModels> {
        self.read_models.as_ref()
    }

    pub const fn has_runtime(&self) -> bool {
        self.runtime.is_some()
    }

    pub const fn handle(&self) -> &ClientApplicationHandle {
        &self.application
    }

    pub fn network_snapshot(&self) -> Result<Option<NetworkSnapshot>, EngineError> {
        self.runtime
            .as_ref()
            .map(|runtime| runtime.network_snapshot().map_err(|_| EngineError::Unavailable))
            .transpose()
    }

    pub fn diagnostics_json(&self) -> Result<String, EngineError> {
        self.runtime.as_ref().map_or_else(
            || Ok("{\"events\":[]}".into()),
            |runtime| runtime.diagnostics_json().map_err(|_| EngineError::Unavailable),
        )
    }

    pub fn start_battery_observation(&self) -> Result<(), EngineError> {
        self.runtime
            .as_ref()
            .ok_or(EngineError::Unavailable)?
            .start_battery_observation()
            .map_err(|_| EngineError::Unavailable)
    }

    pub fn stop_battery_observation(&self) -> Result<(), EngineError> {
        self.runtime
            .as_ref()
            .ok_or(EngineError::Unavailable)?
            .stop_battery_observation()
            .map_err(|_| EngineError::Unavailable)
    }

    pub fn reset_battery_observation(&self) -> Result<(), EngineError> {
        self.runtime
            .as_ref()
            .ok_or(EngineError::Unavailable)?
            .reset_battery_observation()
            .map_err(|_| EngineError::Unavailable)
    }

    pub fn bootstrap_snapshot(&self) -> Result<BootstrapSnapshot, EngineError> {
        self.bootstrap.lock().map(|state| state.snapshot()).map_err(|_| EngineError::Unavailable)
    }

    pub fn snapshot_context(&self) -> Result<ApplicationSnapshotContext, EngineError> {
        let network = self
            .network_snapshot()?
            .unwrap_or_else(|| stopped_network_snapshot(compiled_provider()));
        let attachments = self
            .runtime
            .as_ref()
            .and_then(|runtime| runtime.attachment_snapshot().ok())
            .unwrap_or_default();
        // The root projection is an overview, never a history query. Message
        // bodies and reactions are loaded only by conversation/search pages.
        let application = self.application.overview()?;
        if let (Some(radio), Ok(now)) = (self.radio.as_ref(), current_timestamp()) {
            for contact in &application.contacts {
                radio.ensure_contact(contact.id(), now);
            }
        }
        let (identity_fingerprint, identity_fingerprints, safety_numbers) =
            ApplicationSnapshotContext::security_projection(&application);
        // A secondary queue projection must never make the complete client
        // snapshot (and therefore application startup) unavailable. The
        // operation worker will retry durable queue access independently.
        let pending_operations =
            self.pending.lock().ok().and_then(|store| store.all().ok()).unwrap_or_default();
        Ok(ApplicationSnapshotContext {
            application,
            network,
            attachments,
            bootstrap: self.bootstrap_snapshot()?,
            identity_fingerprint,
            identity_fingerprints,
            safety_numbers,
            pending_operations,
            radio: self.radio_projection(),
        })
    }

    /// Applies observed runtime facts to the application-owned bootstrap state.
    /// Snapshot projection remains read-only and cannot advance attempts.
    pub fn advance_bootstrap(&self) -> Result<(), EngineError> {
        let app = self.application.overview()?;
        let network = self
            .network_snapshot()?
            .unwrap_or_else(|| stopped_network_snapshot(compiled_provider()));
        let has_identity = app.identity.is_some();
        let has_profile = app.identity.as_ref().and_then(|identity| identity.profile()).is_some();
        let communication_state =
            network.communication.step(torca_transport_api::CommissioningStage::LocalRuntime);
        if let Ok(mut bootstrap) = self.bootstrap.lock() {
            bootstrap.configure_communication_requirements(&network.communication);
        }
        let incoming_reachability_state = network
            .communication
            .step(torca_transport_api::CommissioningStage::IncomingReachability);
        // A provider-neutral bootstrap must only consult a managed
        // rendezvous probe when the selected provider profile requires one.
        // Direct providers (Iroh/WebRTC) own their commissioning state and do
        // not have a relay probe here.  This keeps the gate polymorphic: a
        // future provider can use a managed rendezvous service without being
        // hard-coded as Tor.
        let Ok(mut bootstrap) = self.bootstrap.lock() else {
            return Err(EngineError::Unavailable);
        };

        for step in [
            BootstrapStepId::Preferences,
            BootstrapStepId::NativeBridge,
            BootstrapStepId::Contract,
            BootstrapStepId::SecureStorage,
            BootstrapStepId::Database,
        ] {
            if step_state(&bootstrap, step) != Some(BootstrapStepState::Ready) {
                bootstrap.begin(step);
                bootstrap.complete(step);
            }
        }
        if has_identity
            && step_state(&bootstrap, BootstrapStepId::DeviceIdentity)
                != Some(BootstrapStepState::Ready)
        {
            bootstrap.begin(BootstrapStepId::DeviceIdentity);
            bootstrap.complete(BootstrapStepId::DeviceIdentity);
        }
        if has_identity {
            match communication_state {
                torca_transport_api::CommissioningState::Ready
                | torca_transport_api::CommissioningState::NotRequired => {
                    if step_state(&bootstrap, BootstrapStepId::CommunicationRuntime)
                        != Some(BootstrapStepState::Ready)
                    {
                        bootstrap.begin(BootstrapStepId::CommunicationRuntime);
                        bootstrap.complete(BootstrapStepId::CommunicationRuntime);
                    }
                    match incoming_reachability_state {
                        torca_transport_api::CommissioningState::Ready
                        | torca_transport_api::CommissioningState::NotRequired => {
                            if step_state(&bootstrap, BootstrapStepId::IncomingReachability)
                                != Some(BootstrapStepState::Ready)
                            {
                                bootstrap.begin(BootstrapStepId::IncomingReachability);
                                bootstrap.complete(BootstrapStepId::IncomingReachability);
                            }
                        }
                        torca_transport_api::CommissioningState::Failed
                        | torca_transport_api::CommissioningState::Degraded => {
                            bootstrap.begin(BootstrapStepId::IncomingReachability);
                            bootstrap.degrade(
                                BootstrapStepId::IncomingReachability,
                                "INCOMING_REACHABILITY_UNAVAILABLE",
                            );
                        }
                        torca_transport_api::CommissioningState::Pending => {
                            if matches!(
                                step_state(&bootstrap, BootstrapStepId::IncomingReachability),
                                Some(BootstrapStepState::Pending | BootstrapStepState::Blocked)
                            ) {
                                bootstrap.begin(BootstrapStepId::IncomingReachability);
                                bootstrap.verify(BootstrapStepId::IncomingReachability);
                            }
                        }
                    }
                    if step_state(&bootstrap, BootstrapStepId::Rendezvous)
                        != Some(BootstrapStepState::Ready)
                    {
                        bootstrap.begin(BootstrapStepId::Rendezvous);
                        bootstrap.complete(BootstrapStepId::Rendezvous);
                    }
                }
                torca_transport_api::CommissioningState::Failed
                | torca_transport_api::CommissioningState::Degraded => {
                    let code =
                        if communication_state == torca_transport_api::CommissioningState::Failed {
                            "COMMUNICATION_RUNTIME_FAILED"
                        } else {
                            "COMMUNICATION_RUNTIME_DEGRADED"
                        };
                    if step_state(&bootstrap, BootstrapStepId::CommunicationRuntime)
                        != Some(BootstrapStepState::Failed)
                    {
                        bootstrap.begin(BootstrapStepId::CommunicationRuntime);
                        bootstrap.fail(BootstrapStepId::CommunicationRuntime, code);
                    }
                }
                torca_transport_api::CommissioningState::Pending => {
                    if matches!(
                        step_state(&bootstrap, BootstrapStepId::CommunicationRuntime),
                        Some(BootstrapStepState::Pending | BootstrapStepState::Blocked)
                    ) {
                        bootstrap.begin(BootstrapStepId::CommunicationRuntime);
                        bootstrap.verify(BootstrapStepId::CommunicationRuntime);
                    }
                }
            }
        }
        if has_profile
            && step_state(&bootstrap, BootstrapStepId::UserProfile)
                != Some(BootstrapStepState::Ready)
        {
            bootstrap.begin(BootstrapStepId::UserProfile);
            bootstrap.complete(BootstrapStepId::UserProfile);
        }
        Ok(())
    }

    pub fn bootstrap_identity(
        &self,
        identity_id: OpaqueId,
        at_ms: i64,
    ) -> Result<ApplicationCommandResult, String> {
        let at = timestamp(at_ms)?;
        let value = self
            .application
            .dispatch(EngineCommand::CreateIdentity {
                identity_id: IdentityId::from_opaque(identity_id),
                profile: None,
                at,
            })
            .map_err(string_error)?;
        Ok(ApplicationCommandResult {
            kind: result_kind(&value),
            resource_id: None,
            invite_uri: None,
        })
    }

    pub fn execute(
        &self,
        command: ApplicationCommand,
    ) -> Result<ApplicationCommandResult, ApplicationError> {
        self.execute_inner(command).map_err(ApplicationError::operation_failed)
    }

    fn execute_inner(
        &self,
        command: ApplicationCommand,
    ) -> Result<ApplicationCommandResult, String> {
        let mut resource_id = match &command {
            ApplicationCommand::SetAttention { .. } => None,
            ApplicationCommand::CreatePairing { session_id }
            | ApplicationCommand::JoinPairing { session_id, .. }
            | ApplicationCommand::ApprovePairing { session_id }
            | ApplicationCommand::RejectPairing { session_id }
            | ApplicationCommand::CancelPairing { session_id } => Some(*session_id),
            ApplicationCommand::StartConversation { contact_id } => Some(*contact_id),
            ApplicationCommand::SetContactAvailability { contact_id, .. } => Some(*contact_id),
            ApplicationCommand::SetRadioEnabled { contact_id, .. }
            | ApplicationCommand::BeginRadioTransmission { contact_id }
            | ApplicationCommand::EndRadioTransmission { contact_id } => Some(*contact_id),
            ApplicationCommand::ConfigureRadioAudio { .. } => None,
            ApplicationCommand::QueueAttachment { attachment_id, .. }
            | ApplicationCommand::RetryAttachment { attachment_id }
            | ApplicationCommand::CancelAttachment { attachment_id }
            | ApplicationCommand::ExportAttachment { attachment_id, .. }
            | ApplicationCommand::ExportAttachmentPreview { attachment_id, .. } => {
                Some(*attachment_id)
            }
            _ => None,
        };
        let mut invite_uri = None;
        let kind = match command {
            ApplicationCommand::SetAttention { context } => {
                self.runtime
                    .as_ref()
                    .ok_or_else(|| "runtime unavailable".to_owned())?
                    .set_attention(context);
                "attention_updated"
            }
            ApplicationCommand::SetNotifications { .. } => "notifications_updated",
            ApplicationCommand::SetReadReceipts { .. } => "read_receipts_updated",
            ApplicationCommand::SetBatteryPreferences { .. } => "battery_preferences_updated",
            ApplicationCommand::SetContactAvailability { contact_id, mode } => {
                let enabled = mode == "instant";
                runtime_command(
                    self.set_instant_contact_demand(ContactId::from_opaque(contact_id), enabled),
                )?;
                "contact_availability_updated"
            }
            ApplicationCommand::AcknowledgeNewContacts => "contacts_acknowledged",
            ApplicationCommand::UpdateProfile { display_name, avatar_envelope_json, at_ms } => {
                let display_name = ProfileName::new(display_name).map_err(string_error)?;
                let value = self
                    .application
                    .dispatch(EngineCommand::UpdateProfile { display_name, at: timestamp(at_ms)? })
                    .map_err(string_error)?;
                if let Some(json) = avatar_envelope_json {
                    let record = parse_avatar_envelope(&json).map_err(string_error)?;
                    let _ = self
                        .application
                        .dispatch(EngineCommand::SetAvatarGenome { record, at: timestamp(at_ms)? })
                        .map_err(string_error)?;
                }
                result_kind(&value)
            }
            ApplicationCommand::CreatePairing { session_id } => {
                // The operation's own transport result is authoritative.  A
                // separate health sample may be stale or may have used a
                // different provider session, so it must never prevent an explicit
                // user pairing attempt from reaching the runtime.
                match self.runtime.as_ref().map(|runtime| {
                    runtime.create_pairing(PairingSessionId::from_opaque(session_id))
                }) {
                    Some(Ok(invitation)) => {
                        invite_uri = Some(invitation.uri);
                        "pairing_started"
                    }
                    Some(Err(error)) if retryable_runtime_error(&error) => {
                        self.enqueue_pending(session_id, PendingOperationKind::CreatePairing)?;
                        "pairing_queued"
                    }
                    Some(Err(error)) => return Err(error.to_string()),
                    None => {
                        self.enqueue_pending(session_id, PendingOperationKind::CreatePairing)?;
                        "pairing_queued"
                    }
                }
            }
            ApplicationCommand::JoinPairing { session_id, code, ticket, bootstrap } => {
                let code = PairingCode::new(code).map_err(string_error)?;
                match self.runtime.as_ref().map(|runtime| {
                    runtime.join_pairing_with_bootstrap(
                        PairingSessionId::from_opaque(session_id),
                        code.clone(),
                        ticket,
                        bootstrap.clone(),
                    )
                }) {
                    Some(Ok(())) => "pairing_joined",
                    Some(Err(error)) if retryable_runtime_error(&error) => {
                        self.enqueue_pending(
                            session_id,
                            PendingOperationKind::JoinPairing {
                                code: code.as_str().into(),
                                ticket,
                                bootstrap: bootstrap.clone(),
                            },
                        )?;
                        "pairing_queued"
                    }
                    Some(Err(error)) => return Err(error.to_string()),
                    None => {
                        self.enqueue_pending(
                            session_id,
                            PendingOperationKind::JoinPairing {
                                code: code.as_str().into(),
                                ticket,
                                bootstrap,
                            },
                        )?;
                        "pairing_queued"
                    }
                }
            }
            ApplicationCommand::ApprovePairing { session_id } => {
                if self
                    .run_or_enqueue_operation(session_id, PendingOperationKind::ApprovePairing)?
                {
                    "pairing_updated"
                } else {
                    "pairing_update_queued"
                }
            }
            ApplicationCommand::RejectPairing { session_id } => {
                if self.run_or_enqueue_operation(session_id, PendingOperationKind::RejectPairing)? {
                    "pairing_rejected"
                } else {
                    "pairing_rejection_queued"
                }
            }
            ApplicationCommand::CancelPairing { session_id } => {
                match self.run_or_enqueue_operation(session_id, PendingOperationKind::CancelPairing)
                {
                    Ok(true) => {
                        self.clear_pairing_pending(session_id)?;
                        "pairing_cancelled"
                    }
                    Ok(false) => "pairing_cancellation_queued",
                    Err(_error) if self.has_pairing_pending(session_id) => {
                        // A create/join can be queued before an engine session
                        // exists. Cancellation must still remove that local
                        // job instead of reporting a permanent Pairing error.
                        self.clear_pairing_pending(session_id)?;
                        "pairing_cancelled"
                    }
                    Err(error) => return Err(error),
                }
            }
            ApplicationCommand::RenameContact { contact_id, display_name } => {
                if self.run_or_enqueue_operation(
                    contact_id,
                    PendingOperationKind::RenameContact { display_name },
                )? {
                    "contact_renamed"
                } else {
                    "contact_rename_queued"
                }
            }
            ApplicationCommand::VerifyContact { contact_id } => {
                if self.run_or_enqueue_operation(contact_id, PendingOperationKind::VerifyContact)? {
                    "contact_verified"
                } else {
                    "contact_verification_queued"
                }
            }
            ApplicationCommand::ResetContactVerification { contact_id } => {
                if self.run_or_enqueue_operation(
                    contact_id,
                    PendingOperationKind::ResetContactVerification,
                )? {
                    "contact_verification_reset"
                } else {
                    "contact_verification_reset_queued"
                }
            }
            ApplicationCommand::BlockContact { contact_id } => {
                if self.run_or_enqueue_operation(contact_id, PendingOperationKind::BlockContact)? {
                    "contact_blocked"
                } else {
                    "contact_block_queued"
                }
            }
            ApplicationCommand::UnblockContact { contact_id } => {
                if self
                    .run_or_enqueue_operation(contact_id, PendingOperationKind::UnblockContact)?
                {
                    "contact_unblocked"
                } else {
                    "contact_unblock_queued"
                }
            }
            ApplicationCommand::RemoveContact { contact_id } => {
                if self.run_or_enqueue_operation(contact_id, PendingOperationKind::RemoveContact)? {
                    let _ = self
                        .application
                        .dispatch(EngineCommand::RemoveContact {
                            contact_id: ContactId::from_opaque(contact_id),
                        })
                        .map_err(string_error)?;
                    "contact_removed"
                } else {
                    "contact_removal_queued"
                }
            }
            ApplicationCommand::StartConversation { contact_id } => {
                let conversation_id = ConversationId::from_opaque(contact_id);
                let _ = self
                    .application
                    .dispatch(EngineCommand::EnsureConversation {
                        contact_id: ContactId::from_opaque(contact_id),
                        conversation_id,
                        at: current_timestamp()?,
                    })
                    .map_err(string_error)?;
                resource_id = Some(conversation_id.to_opaque());
                "conversation_started"
            }
            ApplicationCommand::ClearConversationHistory { conversation_id } => {
                if self.run_or_enqueue_operation(
                    conversation_id,
                    PendingOperationKind::ClearConversationHistory,
                )? {
                    "conversation_history_cleared"
                } else {
                    "conversation_history_clear_queued"
                }
            }
            ApplicationCommand::ArchiveConversation { conversation_id, at_ms } => {
                let value = self
                    .application
                    .dispatch(EngineCommand::ArchiveConversation {
                        conversation_id: ConversationId::from_opaque(conversation_id),
                        at: timestamp(at_ms)?,
                    })
                    .map_err(string_error)?;
                result_kind(&value)
            }
            ApplicationCommand::RestoreConversation { conversation_id, at_ms } => {
                let value = self
                    .application
                    .dispatch(EngineCommand::RestoreConversation {
                        conversation_id: ConversationId::from_opaque(conversation_id),
                        at: timestamp(at_ms)?,
                    })
                    .map_err(string_error)?;
                result_kind(&value)
            }
            ApplicationCommand::QueueMessage {
                message_id,
                conversation_id,
                body,
                reply_to_message_id,
                at_ms,
            } => {
                let allowed = self
                    .message_rate_limiter
                    .lock()
                    .map_err(|_| "message rate limiter unavailable".to_owned())?
                    .allow(conversation_id, at_ms);
                if !allowed {
                    return Err("message rate limit exceeded; retry in a moment".to_owned());
                }
                let value = self
                    .application
                    .dispatch(EngineCommand::QueueMessage {
                        message_id: MessageId::from_opaque(message_id),
                        conversation_id: ConversationId::from_opaque(conversation_id),
                        body: MessageBody::new(body).map_err(string_error)?,
                        reply_to: reply_to_message_id
                            .map(|id| ReplyReference { message_id: MessageId::from_opaque(id) }),
                        at: timestamp(at_ms)?,
                    })
                    .map_err(string_error)?;
                // Persisting a message is a local operation.  A temporarily unavailable
                // network runtime must not reject or lose the user's message; the durable
                // delivery store will be drained when the runtime becomes available.
                self.queue_outbound_or_defer(message_id, timestamp(at_ms)?);
                result_kind(&value)
            }
            ApplicationCommand::RetryMessage { message_id, at_ms } => {
                let value = self
                    .application
                    .dispatch(EngineCommand::RetryMessage {
                        message_id: MessageId::from_opaque(message_id),
                        at: timestamp(at_ms)?,
                    })
                    .map_err(string_error)?;
                self.wake_delivery_or_defer(message_id);
                result_kind(&value)
            }
            ApplicationCommand::CancelMessage { message_id, at_ms } => {
                let value = self
                    .application
                    .dispatch(EngineCommand::CancelMessage {
                        message_id: MessageId::from_opaque(message_id),
                        at: timestamp(at_ms)?,
                    })
                    .map_err(string_error)?;
                if let Some(runtime) = self.runtime.as_ref() {
                    runtime.release_delivery(message_id);
                    runtime.wake_delivery();
                }
                result_kind(&value)
            }
            ApplicationCommand::EditMessage { message_id, body, at_ms } => {
                let value = self
                    .application
                    .dispatch(EngineCommand::EditMessage {
                        message_id: MessageId::from_opaque(message_id),
                        body: MessageBody::new(body).map_err(string_error)?,
                        at: timestamp(at_ms)?,
                    })
                    .map_err(string_error)?;
                self.wake_delivery_or_defer(message_id);
                result_kind(&value)
            }
            ApplicationCommand::SetMessageReaction {
                message_id,
                conversation_id,
                actor_id,
                emoji,
                active,
                at_ms,
            } => {
                let reaction_emoji = emoji.clone();
                let reaction = MessageReaction::new(
                    MessageId::from_opaque(message_id),
                    ConversationId::from_opaque(conversation_id),
                    actor_id,
                    emoji,
                    active,
                    timestamp(at_ms)?,
                )
                .map_err(string_error)?;
                let value = self
                    .application
                    .dispatch(EngineCommand::SetMessageReaction { reaction })
                    .map_err(string_error)?;
                if let Some(runtime) = self.runtime.as_ref() {
                    if let Some(conversation) = self
                        .application
                        .overview()
                        .map_err(string_error)?
                        .conversations
                        .into_iter()
                        .find(|conversation| conversation.id().to_opaque() == conversation_id)
                    {
                        runtime
                            .queue_reaction(
                                conversation.contact_id(),
                                ReactionPayload {
                                    reaction_id: MessageReaction::deterministic_id(
                                        MessageId::from_opaque(message_id),
                                        actor_id,
                                        &reaction_emoji,
                                    ),
                                    message_id,
                                    conversation_id,
                                    actor_id,
                                    emoji: reaction_emoji,
                                    active,
                                    at: timestamp(at_ms)?,
                                },
                                timestamp(at_ms)?,
                            )
                            .map_err(string_error)?;
                    }
                }
                result_kind(&value)
            }
            ApplicationCommand::MarkConversationRead { conversation_id } => {
                let applied = self.run_or_enqueue_operation(
                    conversation_id,
                    PendingOperationKind::MarkConversationRead,
                )?;
                // Mark-read creates durable control-outbox work. Wake the
                // communication runtime immediately instead of waiting for
                // its periodic maintenance tick, so the sender receives the
                // read receipt promptly.
                self.runtime()?.wake_delivery();
                if applied { "conversation_read" } else { "conversation_read_queued" }
            }
            ApplicationCommand::QueueAttachment {
                attachment_id,
                message_id,
                conversation_id,
                source_path,
                preview_source_path,
                name,
                media_type,
                size,
                at_ms: _,
            } => {
                runtime_command(self.runtime()?.queue_attachment(AttachmentSendRequest {
                    attachment_id,
                    message_id,
                    conversation_id,
                    source_path,
                    preview_source_path,
                    name,
                    media_type,
                    size,
                }))?;
                self.runtime()?.wake_delivery();
                "attachment_queued"
            }
            ApplicationCommand::RetryAttachment { attachment_id } => {
                runtime_command(self.runtime()?.retry_attachment(attachment_id))?;
                "attachment_retried"
            }
            ApplicationCommand::CancelAttachment { attachment_id } => {
                runtime_command(self.runtime()?.cancel_attachment(attachment_id))?;
                "attachment_cancelled"
            }
            ApplicationCommand::ExportAttachment { attachment_id, destination_path } => {
                self.runtime()?
                    .export_attachment(
                        AttachmentId::from_opaque(attachment_id),
                        PathBuf::from(destination_path),
                    )
                    .map_err(string_error)?;
                "attachment_exported"
            }
            ApplicationCommand::ExportAttachmentPreview { attachment_id, destination_path } => {
                self.runtime()?
                    .export_attachment_preview(
                        AttachmentId::from_opaque(attachment_id),
                        PathBuf::from(destination_path),
                    )
                    .map_err(string_error)?;
                "attachment_preview_exported"
            }
            ApplicationCommand::SetRadioEnabled { contact_id, enabled, at_ms } => {
                let contact = ContactId::from_opaque(contact_id);
                let previous_active = self.radio_projection().and_then(|projection| {
                    projection.active_contact_id.filter(|previous| *previous != contact)
                });
                self.radio()?
                    .set_enabled(contact, enabled, timestamp(at_ms)?)
                    .map_err(string_error)?;
                if let Some(runtime) = self.runtime.as_ref() {
                    if enabled {
                        if let Some(previous) = previous_active {
                            // RadioCoordinator atomically disables the old
                            // channel when a new one is enabled. Mirror that
                            // ownership transition in the policy lease book
                            // so the old peer cannot keep probe demand alive.
                            runtime_command(runtime.set_radio_demand(previous, false))?;
                        }
                    }
                    runtime_command(runtime.set_radio_demand(contact, enabled))?;
                }
                if enabled { "radio_enabled" } else { "radio_disabled" }
            }
            ApplicationCommand::ConfigureRadioAudio { input_device_id, output_device_id } => {
                self.radio()?
                    .configure_audio_devices(
                        input_device_id.as_deref(),
                        output_device_id.as_deref(),
                    )
                    .map_err(string_error)?;
                "radio_audio_configured"
            }
            ApplicationCommand::BeginRadioTransmission { contact_id } => {
                let contact = ContactId::from_opaque(contact_id);
                let request_id = self.radio()?.begin_transmission(contact).map_err(string_error);
                let request_id = match request_id {
                    Ok(request_id) => request_id,
                    Err(error) => {
                        if let Some(runtime) = self.runtime.as_ref() {
                            runtime_command(runtime.set_radio_transmission(contact, false))?;
                        }
                        return Err(error);
                    }
                };
                if let Some(runtime) = self.runtime.as_ref() {
                    runtime_command(runtime.set_radio_transmission(contact, true))?;
                }
                resource_id = Some(request_id.to_opaque());
                "radio_transmission_requested"
            }
            ApplicationCommand::EndRadioTransmission { contact_id } => {
                let contact = ContactId::from_opaque(contact_id);
                self.radio()?.end_transmission(contact).map_err(string_error)?;
                if let Some(runtime) = self.runtime.as_ref() {
                    runtime_command(runtime.set_radio_transmission(contact, false))?;
                }
                "radio_transmission_ended"
            }
            ApplicationCommand::RefreshProviderRoute => {
                self.runtime()?.refresh_provider_route().map_err(string_error)?;
                "provider_route_refresh_requested"
            }
            ApplicationCommand::RefreshSnapshot => {
                let _ = self.application.snapshot().map_err(string_error)?;
                "snapshot"
            }
        };
        Ok(ApplicationCommandResult { kind, resource_id, invite_uri })
    }

    fn runtime(&self) -> Result<&RuntimeHandle, String> {
        self.runtime.as_ref().ok_or_else(|| "secure network runtime is not ready".into())
    }

    fn radio(&self) -> Result<&SharedRadioCoordinator, String> {
        self.radio.as_ref().ok_or_else(|| "radio runtime is not ready".into())
    }

    fn enqueue_pending(
        &self,
        resource_id: OpaqueId,
        kind: PendingOperationKind,
    ) -> Result<(), String> {
        let now_ms = current_timestamp()?.to_unix_millis();
        self.pending
            .lock()
            .map_err(|_| "pending operation store is unavailable")?
            .enqueue(PendingOperation {
                id: pending_operation_id(resource_id, &kind),
                resource_id,
                kind,
                attempts: 0,
                next_attempt_at_ms: now_ms,
                created_at_ms: now_ms,
                last_error: None,
            })
            .map_err(|_| "pending operation could not be saved".into())
    }

    fn has_pairing_pending(&self, resource_id: OpaqueId) -> bool {
        self.pending.lock().ok().and_then(|store| store.all().ok()).is_some_and(|operations| {
            operations.iter().any(|operation| {
                operation.resource_id == resource_id
                    && matches!(
                        &operation.kind,
                        PendingOperationKind::CreatePairing
                            | PendingOperationKind::JoinPairing { .. }
                            | PendingOperationKind::ApprovePairing
                            | PendingOperationKind::RejectPairing
                            | PendingOperationKind::CancelPairing
                    )
            })
        })
    }

    fn clear_pairing_pending(&self, resource_id: OpaqueId) -> Result<(), String> {
        let mut store =
            self.pending.lock().map_err(|_| "pending operation store is unavailable")?;
        let kinds = [
            PendingOperationKind::CreatePairing,
            PendingOperationKind::JoinPairing {
                code: String::new(),
                ticket: None,
                bootstrap: None,
            },
            PendingOperationKind::ApprovePairing,
            PendingOperationKind::RejectPairing,
            PendingOperationKind::CancelPairing,
        ];
        for kind in kinds {
            store
                .complete(pending_operation_id(resource_id, &kind))
                .map_err(|_| "pending pairing operation could not be removed")?;
        }
        Ok(())
    }

    fn run_or_enqueue_operation(
        &self,
        resource_id: OpaqueId,
        kind: PendingOperationKind,
    ) -> Result<bool, String> {
        let result = self.runtime.as_ref().map(|runtime| match &kind {
            PendingOperationKind::ApprovePairing => {
                runtime.approve_pairing(PairingSessionId::from_opaque(resource_id))
            }
            PendingOperationKind::RejectPairing => {
                runtime.reject_pairing(PairingSessionId::from_opaque(resource_id))
            }
            PendingOperationKind::CancelPairing => {
                runtime.cancel_pairing(PairingSessionId::from_opaque(resource_id))
            }
            PendingOperationKind::RenameContact { display_name } => {
                runtime.rename_contact(ContactId::from_opaque(resource_id), display_name.clone())
            }
            PendingOperationKind::VerifyContact => {
                runtime.verify_contact(ContactId::from_opaque(resource_id))
            }
            PendingOperationKind::ResetContactVerification => {
                runtime.reset_contact_verification(ContactId::from_opaque(resource_id))
            }
            PendingOperationKind::BlockContact => {
                runtime.block_contact(ContactId::from_opaque(resource_id))
            }
            PendingOperationKind::UnblockContact => {
                runtime.unblock_contact(ContactId::from_opaque(resource_id))
            }
            PendingOperationKind::RemoveContact => {
                runtime.remove_contact(ContactId::from_opaque(resource_id))
            }
            PendingOperationKind::ClearConversationHistory => {
                runtime.clear_conversation_history(ConversationId::from_opaque(resource_id))
            }
            PendingOperationKind::MarkConversationRead => {
                runtime.mark_conversation_read(resource_id)
            }
            PendingOperationKind::CreatePairing | PendingOperationKind::JoinPairing { .. } => {
                unreachable!("create and join preserve command payloads")
            }
        });
        if matches!(result.as_ref(), Some(Ok(()))) {
            return Ok(true);
        }
        if let Some(Err(error)) = result {
            if !retryable_runtime_error(&error) {
                return Err(error.to_string());
            }
        }
        self.enqueue_pending(resource_id, kind)?;
        Ok(false)
    }

    pub fn advance_pending_operations(&self) -> Result<usize, ApplicationError> {
        let Some(runtime) = self.runtime.as_ref() else {
            return Ok(0);
        };
        // A due durable operation executes through the same runtime transport
        // as an explicit command.  Health projection is observational only:
        // it can be stale or use a different provider session and therefore cannot
        // veto recovery of a queued pairing operation.
        let now_ms =
            current_timestamp().map_err(ApplicationError::operation_failed)?.to_unix_millis();
        let operations = self
            .pending
            .lock()
            .map_err(|_| {
                ApplicationError::operation_failed("pending operation store is unavailable".into())
            })?
            .due(now_ms, 1)
            .map_err(|_| {
                ApplicationError::operation_failed("pending operations could not be loaded".into())
            })?;
        let application = self.application.overview().map_err(|error| {
            ApplicationError::operation_failed(format!(
                "pending operation reconciliation failed: {error}"
            ))
        })?;
        let mut completed = 0;
        for operation in operations {
            let existing = application
                .pairings
                .iter()
                .find(|pairing| pairing.id().to_opaque() == operation.resource_id);
            let already_applied = match (&operation.kind, existing) {
                (PendingOperationKind::CreatePairing, Some(_)) => true,
                // A join command creates an empty `Open` session before the
                // provider transport is available.  That session is not
                // proof that the join was sent: treating it as already
                // applied permanently drops the durable retry and leaves the
                // pair stuck on both sides.  Only a state transition carrying
                // the creator proposal proves that the join reached the
                // provider.
                (PendingOperationKind::JoinPairing { .. }, Some(pairing)) => {
                    pairing.state() != PairingState::Open
                }
                (PendingOperationKind::ApprovePairing, Some(pairing)) => pairing.local_approved(),
                (PendingOperationKind::RejectPairing, Some(pairing)) => {
                    pairing.state() == PairingState::Rejected
                }
                (PendingOperationKind::CancelPairing, Some(pairing)) => {
                    pairing.state() == PairingState::Cancelled
                }
                _ => false,
            };
            if already_applied {
                self.pending
                    .lock()
                    .map_err(|_| {
                        ApplicationError::operation_failed(
                            "pending operation store is unavailable".into(),
                        )
                    })?
                    .complete(operation.id)
                    .map_err(|_| {
                        ApplicationError::operation_failed(
                            "pending operation completion could not be saved".into(),
                        )
                    })?;
                completed += 1;
                continue;
            }
            let result = match &operation.kind {
                PendingOperationKind::CreatePairing => runtime
                    .create_pairing(PairingSessionId::from_opaque(operation.resource_id))
                    .map(|_| ()),
                PendingOperationKind::JoinPairing { code, ticket, bootstrap } => {
                    PairingCode::new(code.clone())
                        .map_err(|_| RuntimeDriverError::Pairing)
                        .and_then(|code| {
                            runtime.join_pairing_with_bootstrap(
                                PairingSessionId::from_opaque(operation.resource_id),
                                code,
                                *ticket,
                                bootstrap.clone(),
                            )
                        })
                }
                PendingOperationKind::ApprovePairing => {
                    runtime.approve_pairing(PairingSessionId::from_opaque(operation.resource_id))
                }
                PendingOperationKind::RejectPairing => {
                    runtime.reject_pairing(PairingSessionId::from_opaque(operation.resource_id))
                }
                PendingOperationKind::CancelPairing => {
                    runtime.cancel_pairing(PairingSessionId::from_opaque(operation.resource_id))
                }
                PendingOperationKind::RenameContact { display_name } => runtime.rename_contact(
                    ContactId::from_opaque(operation.resource_id),
                    display_name.clone(),
                ),
                PendingOperationKind::VerifyContact => {
                    runtime.verify_contact(ContactId::from_opaque(operation.resource_id))
                }
                PendingOperationKind::ResetContactVerification => runtime
                    .reset_contact_verification(ContactId::from_opaque(operation.resource_id)),
                PendingOperationKind::BlockContact => {
                    runtime.block_contact(ContactId::from_opaque(operation.resource_id))
                }
                PendingOperationKind::UnblockContact => {
                    runtime.unblock_contact(ContactId::from_opaque(operation.resource_id))
                }
                PendingOperationKind::RemoveContact => {
                    runtime.remove_contact(ContactId::from_opaque(operation.resource_id))
                }
                PendingOperationKind::ClearConversationHistory => runtime
                    .clear_conversation_history(ConversationId::from_opaque(operation.resource_id)),
                PendingOperationKind::MarkConversationRead => {
                    runtime.mark_conversation_read(operation.resource_id)
                }
            };
            let is_cancel = matches!(&operation.kind, PendingOperationKind::CancelPairing);
            let mut store = self.pending.lock().map_err(|_| {
                ApplicationError::operation_failed("pending operation store is unavailable".into())
            })?;
            match result {
                Ok(()) => {
                    if matches!(&operation.kind, PendingOperationKind::RemoveContact) {
                        let _ = self
                            .application
                            .dispatch(EngineCommand::RemoveContact {
                                contact_id: ContactId::from_opaque(operation.resource_id),
                            })
                            .map_err(|error| {
                                ApplicationError::operation_failed(error.to_string())
                            })?;
                    }
                    store.complete(operation.id).map_err(|_| {
                        ApplicationError::operation_failed(
                            "pending operation completion could not be saved".into(),
                        )
                    })?;
                    if is_cancel {
                        drop(store);
                        self.clear_pairing_pending(operation.resource_id)
                            .map_err(ApplicationError::operation_failed)?;
                    }
                    completed += 1;
                }
                Err(error) => {
                    if !retryable_runtime_error(&error) {
                        // A terminal protocol/session error must not remain in
                        // the durable queue forever. The next snapshot will no
                        // longer advertise a misleading "waiting for network"
                        // operation.
                        store.complete(operation.id).map_err(|_| {
                            ApplicationError::operation_failed(
                                "terminal pending operation could not be removed".into(),
                            )
                        })?;
                        if is_cancel {
                            drop(store);
                            self.clear_pairing_pending(operation.resource_id)
                                .map_err(ApplicationError::operation_failed)?;
                        }
                        continue;
                    }
                    let attempts = operation.attempts.saturating_add(1);
                    let shift = attempts.min(5);
                    let delay_ms = 1_000_i64 << shift;
                    store
                        .reschedule(
                            operation.id,
                            attempts,
                            now_ms.saturating_add(delay_ms),
                            &error.to_string(),
                        )
                        .map_err(|_| {
                            ApplicationError::operation_failed(
                                "pending operation retry could not be saved".into(),
                            )
                        })?;
                }
            }
        }
        Ok(completed)
    }

    /// Returns the next durable operation deadline without loading or running
    /// the operation. The native actor uses this to block until useful work is
    /// due instead of waking once per second while the queue is empty.
    pub fn next_pending_operation_delay(&self) -> Option<std::time::Duration> {
        // Work can be persisted before the provider runtime is composed. There
        // is no useful operation to perform until the handle is attached; an
        // immediate deadline here would make the native actor spin in
        // `maintain()` while startup is still in progress.
        self.runtime.as_ref()?;
        let now_ms = current_timestamp().ok()?.to_unix_millis();
        let next_ms = self.pending.lock().ok()?.next_due_at_ms().ok()??;
        let delta_ms = next_ms.saturating_sub(now_ms);
        Some(std::time::Duration::from_millis(u64::try_from(delta_ms).unwrap_or(0)))
    }
}

fn step_state(bootstrap: &BootstrapState, id: BootstrapStepId) -> Option<BootstrapStepState> {
    bootstrap.snapshot().steps.into_iter().find(|step| step.id == id).map(|step| step.state)
}

fn stopped_network_snapshot(provider: torca_foundation::ProviderId) -> NetworkSnapshot {
    NetworkSnapshot {
        communication: torca_transport_api::ProviderCommissioning {
            provider,
            steps: vec![
                torca_transport_api::CommissioningStep {
                    stage: torca_transport_api::CommissioningStage::LocalRuntime,
                    state: torca_transport_api::CommissioningState::Pending,
                    required_for_local_shell: true,
                    required_for_pairing: true,
                },
                torca_transport_api::CommissioningStep {
                    stage: torca_transport_api::CommissioningStage::IncomingReachability,
                    state: torca_transport_api::CommissioningState::NotRequired,
                    required_for_local_shell: false,
                    required_for_pairing: true,
                },
            ],
            endpoint_summary: None,
            route_state: torca_transport_api::ProviderRouteState::Unavailable,
            pairing_bootstrap: None,
        },
        tor: CommunicationState::Stopped,
        peers: BTreeMap::new(),
        peer_health: BTreeMap::new(),
        contact_names: BTreeMap::new(),
        contact_verifications: BTreeMap::new(),
        peer_activity: BTreeMap::new(),
        probes: Vec::new(),
        connectivity: torca_connectivity::ConnectivitySnapshot::default(),
        rendezvous_info: None,
        relay_info: None,
    }
}

fn compiled_provider() -> torca_foundation::ProviderId {
    torca_foundation::ProviderId::new("iroh").expect("production provider id")
}

fn timestamp(value: i64) -> Result<Timestamp, String> {
    Timestamp::from_unix_millis(value).map_err(string_error)
}

fn current_timestamp() -> Result<Timestamp, String> {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| "system clock is before unix epoch")?
        .as_millis();
    Timestamp::from_unix_millis(i64::try_from(millis).map_err(|_| "system clock overflow")?)
        .map_err(|_| "invalid system timestamp".into())
}

fn runtime_command<T>(result: Result<T, RuntimeDriverError>) -> Result<(), String> {
    match result {
        Ok(_) | Err(RuntimeDriverError::Pending) => Ok(()),
        Err(error) => Err(string_error(error)),
    }
}

fn result_kind(result: &EngineResult) -> &'static str {
    match result {
        EngineResult::IdentityCreated => "identity_created",
        EngineResult::ProfileUpdated => "profile_updated",
        EngineResult::PairingStarted => "pairing_started",
        EngineResult::PairingJoined => "pairing_joined",
        EngineResult::PairingUpdated => "pairing_updated",
        EngineResult::PairingRejected => "pairing_rejected",
        EngineResult::PairingCancelled => "pairing_cancelled",
        EngineResult::PairingCompleted { .. } => "pairing_completed",
        EngineResult::PairingRemoved => "pairing_removed",
        EngineResult::ConversationStarted { .. } => "conversation_started",
        EngineResult::ConversationUpdated { .. } => "conversation_updated",
        EngineResult::ContactRemoved { .. } => "contact_removed",
        EngineResult::MessageQueued { .. } => "message_queued",
        EngineResult::MessageUpdated { .. } => "message_updated",
        EngineResult::ReactionUpdated { .. } => "reaction_updated",
        EngineResult::ReceiptApplied { .. } => "receipt_applied",
    }
}

/// Carries the recipient identity alongside a durable delivery wake whenever
/// the engine can resolve it cheaply. A missing or legacy record keeps the
/// compatibility wake so correctness never depends on this optimisation.
fn wake_delivery_for_message(
    runtime: &RuntimeHandle,
    application: &ClientApplicationHandle,
    message_id: OpaqueId,
) {
    if let Ok(Some(contact_id)) = application.message_contact(message_id) {
        runtime.wake_delivery_for_contact(message_id, contact_id);
    } else {
        runtime.wake_delivery_for(message_id);
    }
}

fn retryable_runtime_error(error: &RuntimeDriverError) -> bool {
    matches!(error.descriptor().retry_advice(), RetryAdvice::Immediate | RetryAdvice::Backoff)
}

fn string_error(error: impl core::fmt::Display) -> String {
    error.to_string()
}

#[cfg(test)]
mod message_rate_limiter_tests {
    use super::MessageRateLimiter;
    use torca_foundation::OpaqueId;

    #[test]
    fn limits_bursts_per_conversation() {
        let mut limiter = MessageRateLimiter::default();
        let contact = OpaqueId::from_u128(1);
        for index in 0..MessageRateLimiter::PER_CONVERSATION_LIMIT {
            assert!(limiter.allow(contact, i64::try_from(index).expect("test index fits in i64"),));
        }
        assert!(!limiter.allow(contact, 100));
        assert!(limiter.allow(contact, 1_001));
    }

    #[test]
    fn separate_conversations_have_independent_budgets() {
        let mut limiter = MessageRateLimiter::default();
        let first = OpaqueId::from_u128(1);
        let second = OpaqueId::from_u128(2);
        for index in 0..MessageRateLimiter::PER_CONVERSATION_LIMIT {
            assert!(limiter.allow(first, i64::try_from(index).expect("test index fits in i64"),));
        }
        assert!(limiter.allow(second, 100));
    }
}
