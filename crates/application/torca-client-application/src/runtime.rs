use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

use torca_attachments::AttachmentId;
use torca_bootstrap::{BootstrapSnapshot, BootstrapState, BootstrapStepId, BootstrapStepState};
use torca_contacts::ContactId;
use torca_conversations::ConversationId;
use torca_foundation::{
    ClassifiedError, ErrorCategory, ErrorCode, ErrorDescriptor, OpaqueId, RetryAdvice, Timestamp,
};
use torca_identity::{IdentityId, ProfileName};
use torca_messaging::{MessageBody, MessageId, ReplyReference};
use torca_pairing::{PairingCode, PairingSessionId, PairingState};
use torca_probing::{ProbeStatus, ProbeTarget};

use crate::{
    ApplicationReadModels, ApplicationSnapshotContext, AttachmentSendRequest,
    ClientApplicationHandle, EngineCommand, EngineError, EngineResult,
    InMemoryPendingOperationStore, NetworkSnapshot, PendingOperation, PendingOperationKind,
    PendingOperationStore, RuntimeDriverError, RuntimeHandle, TorState, pending_operation_id,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ApplicationCommand {
    SetNotifications {
        enabled: bool,
    },
    AcknowledgeNewContacts,
    UpdateProfile {
        display_name: String,
        at_ms: i64,
    },
    CreatePairing {
        session_id: OpaqueId,
    },
    JoinPairing {
        session_id: OpaqueId,
        code: String,
        ticket: Option<[u8; 16]>,
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
    MarkConversationRead {
        conversation_id: OpaqueId,
    },
    QueueAttachment {
        attachment_id: OpaqueId,
        message_id: OpaqueId,
        conversation_id: OpaqueId,
        source_path: String,
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

    pub fn from_message(message: String) -> Self {
        let descriptor = match message.as_str() {
            "RELAY_NOT_READY" => ErrorDescriptor::new(
                ErrorCode::new("relay.not_ready"),
                ErrorCategory::Unavailable,
                RetryAdvice::Backoff,
            ),
            "RELAY_DEGRADED" => ErrorDescriptor::new(
                ErrorCode::new("relay.degraded"),
                ErrorCategory::Unavailable,
                RetryAdvice::Backoff,
            ),
            "PROFILE_NOT_READY" => ErrorDescriptor::new(
                ErrorCode::new("profile.not_ready"),
                ErrorCategory::Conflict,
                RetryAdvice::Never,
            ),
            "secure network runtime is not ready" => ErrorDescriptor::new(
                ErrorCode::new("runtime.not_ready"),
                ErrorCategory::Unavailable,
                RetryAdvice::Backoff,
            ),
            "network readiness is unavailable" => ErrorDescriptor::new(
                ErrorCode::new("network.unavailable"),
                ErrorCategory::Unavailable,
                RetryAdvice::Backoff,
            ),
            _ => ErrorDescriptor::new(
                ErrorCode::new("application.operation_failed"),
                ErrorCategory::Internal,
                RetryAdvice::Never,
            ),
        };
        Self { message, descriptor }
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

pub struct ClientApplicationRuntime {
    application: ClientApplicationHandle,
    runtime: Option<RuntimeHandle>,
    bootstrap: Mutex<BootstrapState>,
    read_models: Option<ApplicationReadModels>,
    pending: Mutex<Box<dyn PendingOperationStore>>,
}

impl ClientApplicationRuntime {
    pub fn new(application: ClientApplicationHandle) -> Self {
        Self {
            application,
            runtime: None,
            bootstrap: Mutex::new(BootstrapState::new()),
            read_models: None,
            pending: Mutex::new(Box::new(InMemoryPendingOperationStore::default())),
        }
    }

    pub fn attach_runtime(&mut self, runtime: RuntimeHandle) {
        self.runtime = Some(runtime);
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
            .map(|runtime| {
                runtime
                    .network_snapshot()
                    .map_err(|_| EngineError("network snapshot unavailable".into()))
            })
            .transpose()
    }

    pub fn diagnostics_json(&self) -> Result<String, EngineError> {
        self.runtime.as_ref().map_or_else(
            || Ok("{\"events\":[]}".into()),
            |runtime| {
                runtime
                    .diagnostics_json()
                    .map_err(|_| EngineError("diagnostics unavailable".into()))
            },
        )
    }

    pub fn bootstrap_snapshot(&self) -> Result<BootstrapSnapshot, EngineError> {
        self.bootstrap
            .lock()
            .map(|state| state.snapshot())
            .map_err(|_| EngineError("bootstrap state unavailable".into()))
    }

    pub fn snapshot_context(&self) -> Result<ApplicationSnapshotContext, EngineError> {
        let network = self.network_snapshot()?.unwrap_or_else(stopped_network_snapshot);
        let attachments = self
            .runtime
            .as_ref()
            .and_then(|runtime| runtime.attachment_snapshot().ok())
            .unwrap_or_default();
        let application = self.application.snapshot()?;
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
        })
    }

    /// Applies observed runtime facts to the application-owned bootstrap state.
    /// Snapshot projection remains read-only and cannot advance attempts.
    pub fn advance_bootstrap(&self) -> Result<(), EngineError> {
        let app = self.application.overview()?;
        let network = self.network_snapshot()?.unwrap_or_else(stopped_network_snapshot);
        let has_identity = app.identity.is_some();
        let has_profile = app.identity.as_ref().and_then(|identity| identity.profile()).is_some();
        let tor_state = format!("{:?}", network.tor).to_lowercase();
        let onion_status = network
            .probes
            .iter()
            .find(|probe| probe.target == ProbeTarget::OnionService)
            .map(|probe| probe.status)
            .unwrap_or(ProbeStatus::Unknown);
        let relay_status = network
            .probes
            .iter()
            .find(|probe| probe.target == ProbeTarget::Relay)
            .map(|probe| probe.status)
            .unwrap_or(ProbeStatus::Unknown);
        let Ok(mut bootstrap) = self.bootstrap.lock() else {
            return Err(EngineError("bootstrap state unavailable".into()));
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
            match tor_state.as_str() {
                "ready" => {
                    if step_state(&bootstrap, BootstrapStepId::Tor)
                        != Some(BootstrapStepState::Ready)
                    {
                        bootstrap.begin(BootstrapStepId::Tor);
                        bootstrap.complete(BootstrapStepId::Tor);
                    }
                    match onion_status {
                        ProbeStatus::Healthy => {
                            if step_state(&bootstrap, BootstrapStepId::OnionService)
                                != Some(BootstrapStepState::Ready)
                            {
                                bootstrap.begin(BootstrapStepId::OnionService);
                                bootstrap.complete(BootstrapStepId::OnionService);
                            }
                        }
                        ProbeStatus::Failed | ProbeStatus::Unreachable | ProbeStatus::Degraded => {
                            bootstrap.begin(BootstrapStepId::OnionService);
                            bootstrap.degrade(BootstrapStepId::OnionService, "ONION_UNREACHABLE");
                        }
                        ProbeStatus::Checking | ProbeStatus::Unknown | ProbeStatus::Disabled => {
                            if matches!(
                                step_state(&bootstrap, BootstrapStepId::OnionService),
                                Some(BootstrapStepState::Pending | BootstrapStepState::Blocked)
                            ) {
                                bootstrap.begin(BootstrapStepId::OnionService);
                                bootstrap.verify(BootstrapStepId::OnionService);
                            }
                        }
                    }
                    match relay_status {
                        ProbeStatus::Healthy => {
                            if step_state(&bootstrap, BootstrapStepId::Relay)
                                != Some(BootstrapStepState::Ready)
                            {
                                bootstrap.begin(BootstrapStepId::Relay);
                                bootstrap.complete(BootstrapStepId::Relay);
                            }
                        }
                        ProbeStatus::Failed | ProbeStatus::Unreachable | ProbeStatus::Degraded => {
                            if step_state(&bootstrap, BootstrapStepId::Relay)
                                != Some(BootstrapStepState::Degraded)
                            {
                                bootstrap.begin(BootstrapStepId::Relay);
                            }
                            bootstrap.degrade(BootstrapStepId::Relay, "RELAY_UNREACHABLE");
                        }
                        ProbeStatus::Checking | ProbeStatus::Unknown | ProbeStatus::Disabled => {
                            if matches!(
                                step_state(&bootstrap, BootstrapStepId::Relay),
                                Some(BootstrapStepState::Pending | BootstrapStepState::Blocked)
                            ) {
                                bootstrap.begin(BootstrapStepId::Relay);
                                bootstrap.verify(BootstrapStepId::Relay);
                            }
                        }
                    }
                }
                "failed" | "degraded" => {
                    let code = if tor_state == "failed" {
                        "TOR_RUNTIME_FAILED"
                    } else {
                        "TOR_RUNTIME_DEGRADED"
                    };
                    if step_state(&bootstrap, BootstrapStepId::Tor)
                        != Some(BootstrapStepState::Failed)
                    {
                        bootstrap.begin(BootstrapStepId::Tor);
                        bootstrap.fail(BootstrapStepId::Tor, code);
                    }
                }
                _ => {
                    if matches!(
                        step_state(&bootstrap, BootstrapStepId::Tor),
                        Some(BootstrapStepState::Pending | BootstrapStepState::Blocked)
                    ) {
                        bootstrap.begin(BootstrapStepId::Tor);
                        bootstrap.verify(BootstrapStepId::Tor);
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
        self.execute_inner(command).map_err(ApplicationError::from_message)
    }

    fn execute_inner(
        &self,
        command: ApplicationCommand,
    ) -> Result<ApplicationCommandResult, String> {
        let mut resource_id = match &command {
            ApplicationCommand::CreatePairing { session_id }
            | ApplicationCommand::JoinPairing { session_id, .. }
            | ApplicationCommand::ApprovePairing { session_id }
            | ApplicationCommand::RejectPairing { session_id }
            | ApplicationCommand::CancelPairing { session_id } => Some(*session_id),
            ApplicationCommand::StartConversation { contact_id } => Some(*contact_id),
            _ => None,
        };
        let mut invite_uri = None;
        let kind = match command {
            ApplicationCommand::SetNotifications { .. } => "notifications_updated",
            ApplicationCommand::AcknowledgeNewContacts => "contacts_acknowledged",
            ApplicationCommand::UpdateProfile { display_name, at_ms } => {
                let display_name = ProfileName::new(display_name).map_err(string_error)?;
                let value = self
                    .application
                    .dispatch(EngineCommand::UpdateProfile { display_name, at: timestamp(at_ms)? })
                    .map_err(string_error)?;
                result_kind(&value)
            }
            ApplicationCommand::CreatePairing { session_id } => {
                // The operation's own transport result is authoritative.  A
                // separate health sample may be stale or may have used a
                // different Tor stream, so it must never prevent an explicit
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
            ApplicationCommand::JoinPairing { session_id, code, ticket } => {
                let code = PairingCode::new(code).map_err(string_error)?;
                match self.runtime.as_ref().map(|runtime| {
                    runtime.join_pairing_with_ticket(
                        PairingSessionId::from_opaque(session_id),
                        code.clone(),
                        ticket,
                    )
                }) {
                    Some(Ok(())) => "pairing_joined",
                    Some(Err(error)) if retryable_runtime_error(&error) => {
                        self.enqueue_pending(
                            session_id,
                            PendingOperationKind::JoinPairing {
                                code: code.as_str().into(),
                                ticket,
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
            ApplicationCommand::QueueMessage {
                message_id,
                conversation_id,
                body,
                reply_to_message_id,
                at_ms,
            } => {
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
                if let Some(runtime) = self.runtime.as_ref() {
                    runtime.wake_delivery();
                }
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
                if let Some(runtime) = self.runtime.as_ref() {
                    runtime.wake_delivery();
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
            PendingOperationKind::JoinPairing { code: String::new(), ticket: None },
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
        // it can be stale or use a different Tor stream and therefore cannot
        // veto recovery of a queued pairing operation.
        let now_ms = current_timestamp().map_err(ApplicationError::from_message)?.to_unix_millis();
        let operations = self
            .pending
            .lock()
            .map_err(|_| {
                ApplicationError::from_message("pending operation store is unavailable".into())
            })?
            .due(now_ms, 1)
            .map_err(|_| {
                ApplicationError::from_message("pending operations could not be loaded".into())
            })?;
        let application = self.application.overview().map_err(|error| {
            ApplicationError::from_message(format!(
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
                (
                    PendingOperationKind::CreatePairing | PendingOperationKind::JoinPairing { .. },
                    Some(_),
                ) => true,
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
                        ApplicationError::from_message(
                            "pending operation store is unavailable".into(),
                        )
                    })?
                    .complete(operation.id)
                    .map_err(|_| {
                        ApplicationError::from_message(
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
                PendingOperationKind::JoinPairing { code, ticket } => {
                    PairingCode::new(code.clone())
                        .map_err(|_| RuntimeDriverError::Pairing)
                        .and_then(|code| {
                            runtime.join_pairing_with_ticket(
                                PairingSessionId::from_opaque(operation.resource_id),
                                code,
                                *ticket,
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
                ApplicationError::from_message("pending operation store is unavailable".into())
            })?;
            match result {
                Ok(()) => {
                    if matches!(&operation.kind, PendingOperationKind::RemoveContact) {
                        let _ = self
                            .application
                            .dispatch(EngineCommand::RemoveContact {
                                contact_id: ContactId::from_opaque(operation.resource_id),
                            })
                            .map_err(|error| ApplicationError::from_message(error.to_string()))?;
                    }
                    store.complete(operation.id).map_err(|_| {
                        ApplicationError::from_message(
                            "pending operation completion could not be saved".into(),
                        )
                    })?;
                    if is_cancel {
                        drop(store);
                        self.clear_pairing_pending(operation.resource_id)
                            .map_err(ApplicationError::from_message)?;
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
                            ApplicationError::from_message(
                                "terminal pending operation could not be removed".into(),
                            )
                        })?;
                        if is_cancel {
                            drop(store);
                            self.clear_pairing_pending(operation.resource_id)
                                .map_err(ApplicationError::from_message)?;
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
                            ApplicationError::from_message(
                                "pending operation retry could not be saved".into(),
                            )
                        })?;
                }
            }
        }
        Ok(completed)
    }
}

fn step_state(bootstrap: &BootstrapState, id: BootstrapStepId) -> Option<BootstrapStepState> {
    bootstrap.snapshot().steps.into_iter().find(|step| step.id == id).map(|step| step.state)
}

fn stopped_network_snapshot() -> NetworkSnapshot {
    NetworkSnapshot {
        tor: TorState::Stopped,
        onion_address: None,
        peers: BTreeMap::new(),
        peer_health: BTreeMap::new(),
        contact_names: BTreeMap::new(),
        contact_verifications: BTreeMap::new(),
        peer_activity: BTreeMap::new(),
        probes: Vec::new(),
        connectivity: torca_connectivity::ConnectivitySnapshot::default(),
        relay_info: None,
    }
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
        EngineResult::ContactRemoved { .. } => "contact_removed",
        EngineResult::MessageQueued { .. } => "message_queued",
        EngineResult::MessageUpdated { .. } => "message_updated",
        EngineResult::ReceiptApplied { .. } => "receipt_applied",
    }
}

fn retryable_runtime_error(error: &RuntimeDriverError) -> bool {
    matches!(error.descriptor().retry_advice(), RetryAdvice::Immediate | RetryAdvice::Backoff)
}

fn string_error(error: impl core::fmt::Display) -> String {
    error.to_string()
}
