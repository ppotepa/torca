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
use torca_pairing::{PairingCode, PairingSessionId};
use torca_probing::{ProbeStatus, ProbeTarget};

use crate::{
    ApplicationReadModels, ApplicationSnapshotContext, AttachmentSendRequest,
    ClientApplicationHandle, EngineCommand, EngineError, EngineResult, NetworkSnapshot,
    RuntimeDriverError, RuntimeHandle, TorState, TransportActivitySnapshot,
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
}

impl ClientApplicationRuntime {
    pub fn new(application: ClientApplicationHandle) -> Self {
        Self {
            application,
            runtime: None,
            bootstrap: Mutex::new(BootstrapState::new()),
            read_models: None,
        }
    }

    pub fn attach_runtime(&mut self, runtime: RuntimeHandle) {
        self.runtime = Some(runtime);
    }

    pub fn attach_read_models(&mut self, read_models: ApplicationReadModels) {
        self.read_models = Some(read_models);
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
        Ok(ApplicationSnapshotContext {
            application,
            network,
            attachments,
            bootstrap: self.bootstrap_snapshot()?,
            identity_fingerprint,
            identity_fingerprints,
            safety_numbers,
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
        let has_onion = network.onion_address.is_some();
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
                    if has_onion
                        && step_state(&bootstrap, BootstrapStepId::OnionService)
                            != Some(BootstrapStepState::Ready)
                    {
                        bootstrap.begin(BootstrapStepId::OnionService);
                        bootstrap.complete(BootstrapStepId::OnionService);
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
                let network = self.required_network()?;
                ClientApplicationHandle::profile_setup_allowed(&network).map_err(str::to_owned)?;
                let display_name = ProfileName::new(display_name).map_err(string_error)?;
                let value = self
                    .application
                    .dispatch(EngineCommand::UpdateProfile { display_name, at: timestamp(at_ms)? })
                    .map_err(string_error)?;
                result_kind(&value)
            }
            ApplicationCommand::CreatePairing { session_id } => {
                let network = self.required_network()?;
                ClientApplicationHandle::pairing_creation_allowed(&network)
                    .map_err(str::to_owned)?;
                let invitation = self
                    .runtime()?
                    .create_pairing(PairingSessionId::from_opaque(session_id))
                    .map_err(|error| error.to_string())?;
                invite_uri = Some(invitation.uri);
                "pairing_started"
            }
            ApplicationCommand::JoinPairing { session_id, code, ticket } => {
                let network = self.required_network()?;
                ClientApplicationHandle::pairing_join_allowed(&network).map_err(str::to_owned)?;
                let code = PairingCode::new(code).map_err(string_error)?;
                runtime_command(self.runtime()?.join_pairing_with_ticket(
                    PairingSessionId::from_opaque(session_id),
                    code,
                    ticket,
                ))?;
                "pairing_joined"
            }
            ApplicationCommand::ApprovePairing { session_id } => {
                runtime_command(
                    self.runtime()?.approve_pairing(PairingSessionId::from_opaque(session_id)),
                )?;
                "pairing_updated"
            }
            ApplicationCommand::RejectPairing { session_id } => {
                runtime_command(
                    self.runtime()?.reject_pairing(PairingSessionId::from_opaque(session_id)),
                )?;
                "pairing_rejected"
            }
            ApplicationCommand::CancelPairing { session_id } => {
                runtime_command(
                    self.runtime()?.cancel_pairing(PairingSessionId::from_opaque(session_id)),
                )?;
                "pairing_cancelled"
            }
            ApplicationCommand::RenameContact { contact_id, display_name } => {
                runtime_command(
                    self.runtime()?
                        .rename_contact(ContactId::from_opaque(contact_id), display_name),
                )?;
                "contact_renamed"
            }
            ApplicationCommand::VerifyContact { contact_id } => {
                runtime_command(
                    self.runtime()?.verify_contact(ContactId::from_opaque(contact_id)),
                )?;
                "contact_verified"
            }
            ApplicationCommand::ResetContactVerification { contact_id } => {
                runtime_command(
                    self.runtime()?.reset_contact_verification(ContactId::from_opaque(contact_id)),
                )?;
                "contact_verification_reset"
            }
            ApplicationCommand::BlockContact { contact_id } => {
                runtime_command(self.runtime()?.block_contact(ContactId::from_opaque(contact_id)))?;
                "contact_blocked"
            }
            ApplicationCommand::UnblockContact { contact_id } => {
                runtime_command(
                    self.runtime()?.unblock_contact(ContactId::from_opaque(contact_id)),
                )?;
                "contact_unblocked"
            }
            ApplicationCommand::RemoveContact { contact_id } => {
                runtime_command(
                    self.runtime()?.remove_contact(ContactId::from_opaque(contact_id)),
                )?;
                "contact_removed"
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
                runtime_command(
                    self.runtime()?
                        .clear_conversation_history(ConversationId::from_opaque(conversation_id)),
                )?;
                "conversation_history_cleared"
            }
            ApplicationCommand::QueueMessage {
                message_id,
                conversation_id,
                body,
                reply_to_message_id,
                at_ms,
            } => {
                let runtime = self.runtime()?;
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
                runtime.wake_delivery();
                result_kind(&value)
            }
            ApplicationCommand::RetryMessage { message_id, at_ms } => {
                let runtime = self.runtime()?;
                let value = self
                    .application
                    .dispatch(EngineCommand::RetryMessage {
                        message_id: MessageId::from_opaque(message_id),
                        at: timestamp(at_ms)?,
                    })
                    .map_err(string_error)?;
                runtime.wake_delivery();
                result_kind(&value)
            }
            ApplicationCommand::MarkConversationRead { conversation_id } => {
                runtime_command(self.runtime()?.mark_conversation_read(conversation_id))?;
                "conversation_read"
            }
            ApplicationCommand::QueueAttachment {
                attachment_id,
                message_id,
                conversation_id,
                source_path,
                name,
                media_type,
                size,
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

    fn required_network(&self) -> Result<NetworkSnapshot, String> {
        self.runtime()?.network_snapshot().map_err(|_| "network readiness is unavailable".into())
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
        tor_activity: TransportActivitySnapshot::default(),
        relay_activity: TransportActivitySnapshot::default(),
        peer_activity: BTreeMap::new(),
        probes: Vec::new(),
        connectivity: torca_connectivity::ConnectivitySnapshot::default(),
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
        EngineResult::MessageQueued { .. } => "message_queued",
        EngineResult::MessageUpdated { .. } => "message_updated",
        EngineResult::ReceiptApplied { .. } => "receipt_applied",
    }
}

fn string_error(error: impl core::fmt::Display) -> String {
    error.to_string()
}
