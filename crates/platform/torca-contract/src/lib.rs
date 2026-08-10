//! Stable typed boundary between Flutter and the process-owned Rust runtime.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::Mutex;

use sha2::{Digest, Sha256};
use torca_attachments::AttachmentId;
use torca_bootstrap::{BootstrapPhase, BootstrapState, BootstrapStepId, BootstrapStepState};
use torca_client_engine::{ClientSnapshot, EngineCommand, EngineError, EngineHandle, EngineResult};
use torca_contacts::ContactId;
use torca_conversations::ConversationId;
use torca_foundation::{OpaqueId, Timestamp};
use torca_identity::{IdentityId, ProfileName, PublicIdentity};
use torca_messaging::{
    Message, MessageBody, MessageDirection, MessageId, MessageStatus, ReplyReference,
};
use torca_pairing::{PairingCode, PairingSessionId};
use torca_probing::{ProbeStatus, ProbeTarget};
use torca_runtime::{
    AttachmentSendRequest, AttachmentView, NetworkSnapshot, RuntimeDriverError, RuntimeHandle,
    TorState,
};

pub const CONTRACT_VERSION: u16 = 13;

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BridgeCommand {
    SetNotifications {
        enabled: bool,
    },
    UpdateProfile {
        display_name: String,
        at_ms: i64,
    },
    CreatePairing {
        session_id_hex: String,
    },
    JoinPairing {
        session_id_hex: String,
        code: String,
    },
    ApprovePairing {
        session_id_hex: String,
    },
    RejectPairing {
        session_id_hex: String,
    },
    CancelPairing {
        session_id_hex: String,
    },
    RenameContact {
        contact_id_hex: String,
        display_name: String,
    },
    VerifyContact {
        contact_id_hex: String,
    },
    ResetContactVerification {
        contact_id_hex: String,
    },
    BlockContact {
        contact_id_hex: String,
    },
    UnblockContact {
        contact_id_hex: String,
    },
    RemoveContact {
        contact_id_hex: String,
    },
    ClearConversationHistory {
        conversation_id_hex: String,
    },
    QueueMessage {
        message_id_hex: String,
        conversation_id_hex: String,
        body: String,
        reply_to_message_id_hex: Option<String>,
        at_ms: i64,
    },
    RetryMessage {
        message_id_hex: String,
        at_ms: i64,
    },
    MarkConversationRead {
        conversation_id_hex: String,
    },
    MarkConversationReadWithPolicy {
        conversation_id_hex: String,
        send_receipt: bool,
    },
    QueueAttachment {
        attachment_id_hex: String,
        message_id_hex: String,
        conversation_id_hex: String,
        source_path: String,
        name: String,
        media_type: String,
        size: u64,
    },
    RetryAttachment {
        attachment_id_hex: String,
    },
    CancelAttachment {
        attachment_id_hex: String,
    },
    ExportAttachment {
        attachment_id_hex: String,
        destination_path: String,
    },
    RefreshSnapshot,
}

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BridgeResult {
    pub ok: bool,
    pub kind: String,
    pub error: Option<String>,
}
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BridgeSnapshot {
    pub contract_version: u16,
    pub identity_name: Option<String>,
    pub identity_fingerprint: Option<String>,
    pub tor_state: String,
    pub onion_address: Option<String>,
    pub pairings: Vec<BridgePairing>,
    pub contacts: Vec<BridgeContact>,
    pub conversations: Vec<BridgeConversation>,
    pub messages: Vec<BridgeMessage>,
    pub attachments: Vec<BridgeAttachment>,
    pub bootstrap_phase: String,
    pub bootstrap_steps: Vec<BridgeBootstrapStep>,
}

/// Cursor-addressed, redacted notification emitted by the process runtime.
/// Message bodies, keys, onion addresses and safety numbers never cross this type.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NotificationEvent {
    pub cursor: u64,
    pub event_id: String,
    pub kind: String,
    pub conversation_id: String,
    pub contact_display_name: String,
    pub created_at_ms: i64,
}
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BridgeBootstrapStep {
    pub id: String,
    pub state: String,
    pub code: Option<String>,
    pub progress: u8,
    pub attempt: u32,
    pub started_at_ms: Option<i64>,
    pub last_progress_at_ms: Option<i64>,
    pub retry_at_ms: Option<i64>,
}
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BridgePairing {
    pub id: String,
    pub code: String,
    pub role: String,
    pub state: String,
    pub expires_at_ms: i64,
    pub local_approved: bool,
    pub remote_approved: bool,
}
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BridgePeerHealth {
    pub state: String,
    pub quality: String,
    pub rtt_ms: Option<u64>,
    pub last_success_at_ms: Option<i64>,
    pub consecutive_failures: u32,
    pub reconnect_attempt: u32,
}
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BridgeContact {
    pub id: String,
    pub display_name: String,
    pub onion_address: String,
    pub status: String,
    pub connection_state: String,
    pub safety_number: String,
    pub peer_health: BridgePeerHealth,
    pub verification_status: String,
    pub verified_at_ms: Option<i64>,
}
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BridgeConversation {
    pub id: String,
    pub contact_id: String,
    pub status: String,
    pub unread_count: u32,
    pub last_activity_at_ms: i64,
    pub last_message_body: Option<String>,
    pub last_message_direction: Option<String>,
    pub last_message_status: Option<String>,
}
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BridgeMessage {
    pub id: String,
    pub conversation_id: String,
    pub body: String,
    pub direction: String,
    pub status: String,
    pub reply_to_message_id: Option<String>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub attempt_count: u32,
}
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BridgeMessagePage {
    pub messages: Vec<BridgeMessage>,
    pub has_more: bool,
}
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BridgeAttachment {
    pub id: String,
    pub message_id: String,
    pub name: String,
    pub media_type: String,
    pub size: u64,
    pub status: String,
    pub offset: u64,
}

pub struct ContractRuntime {
    engine: EngineHandle,
    runtime: Option<RuntimeHandle>,
    bootstrap: Mutex<BootstrapState>,
}
impl ContractRuntime {
    pub fn new(engine: EngineHandle) -> Self {
        Self { engine, runtime: None, bootstrap: Mutex::new(BootstrapState::new()) }
    }
    pub fn attach_runtime(&mut self, runtime: RuntimeHandle) {
        self.runtime = Some(runtime);
    }
    pub const fn has_runtime(&self) -> bool {
        self.runtime.is_some()
    }

    /// Creates the durable bootstrap identity. This is runtime-internal and is
    /// intentionally not represented as a wire command.
    pub fn bootstrap_identity(&self, identity_id_hex: &str, at_ms: i64) -> BridgeResult {
        let result = parse_id(identity_id_hex)
            .and_then(|id| timestamp(at_ms).map(|at| (id, at)))
            .and_then(|(id, at)| {
                self.engine
                    .dispatch(EngineCommand::CreateIdentity {
                        identity_id: IdentityId::from_opaque(id),
                        profile: None,
                        at,
                    })
                    .map_err(string_error)
            })
            .map(|value| result_kind(&value).to_owned());
        match result {
            Ok(kind) => BridgeResult { ok: true, kind, error: None },
            Err(error) => BridgeResult {
                ok: false,
                kind: "identity_bootstrap_failed".into(),
                error: Some(error),
            },
        }
    }

    pub fn execute(&self, command: BridgeCommand) -> BridgeResult {
        let result: Result<&'static str, String> = match command {
            BridgeCommand::SetNotifications { .. } => Ok("notifications_updated"),
            BridgeCommand::UpdateProfile { display_name, at_ms } => self
                .profile_setup_allowed()
                .and_then(|()| ProfileName::new(display_name).map_err(string_error))
                .and_then(|display_name| timestamp(at_ms).map(|at| (display_name, at)))
                .and_then(|(display_name, at)| {
                    self.engine
                        .dispatch(EngineCommand::UpdateProfile { display_name, at })
                        .map_err(string_error)
                })
                .map(|value| result_kind(&value)),
            BridgeCommand::CreatePairing { session_id_hex } => self
                .pairing_creation_allowed()
                .and_then(|()| parse_pairing_id(&session_id_hex))
                .and_then(|id| runtime_command(self.runtime()?.create_pairing(id)))
                .map(|_| "pairing_started"),
            BridgeCommand::JoinPairing { session_id_hex, code } => self
                .pairing_creation_allowed()
                .and_then(|()| parse_pairing_id(&session_id_hex))
                .and_then(|id| PairingCode::new(code).map_err(string_error).map(|code| (id, code)))
                .and_then(|(id, code)| runtime_command(self.runtime()?.join_pairing(id, code)))
                .map(|_| "pairing_joined"),
            BridgeCommand::ApprovePairing { session_id_hex } => parse_pairing_id(&session_id_hex)
                .and_then(|id| runtime_command(self.runtime()?.approve_pairing(id)))
                .map(|_| "pairing_updated"),
            BridgeCommand::RejectPairing { session_id_hex } => parse_pairing_id(&session_id_hex)
                .and_then(|id| runtime_command(self.runtime()?.reject_pairing(id)))
                .map(|_| "pairing_rejected"),
            BridgeCommand::CancelPairing { session_id_hex } => parse_pairing_id(&session_id_hex)
                .and_then(|id| runtime_command(self.runtime()?.cancel_pairing(id)))
                .map(|_| "pairing_cancelled"),
            BridgeCommand::RenameContact { contact_id_hex, display_name } => {
                parse_contact_id(&contact_id_hex)
                    .and_then(|id| {
                        runtime_command(self.runtime()?.rename_contact(id, display_name))
                    })
                    .map(|_| "contact_renamed")
            }
            BridgeCommand::VerifyContact { contact_id_hex } => parse_contact_id(&contact_id_hex)
                .and_then(|id| runtime_command(self.runtime()?.verify_contact(id)))
                .map(|_| "contact_verified"),
            BridgeCommand::ResetContactVerification { contact_id_hex } => {
                parse_contact_id(&contact_id_hex)
                    .and_then(|id| runtime_command(self.runtime()?.reset_contact_verification(id)))
                    .map(|_| "contact_verification_reset")
            }
            BridgeCommand::BlockContact { contact_id_hex } => parse_contact_id(&contact_id_hex)
                .and_then(|id| runtime_command(self.runtime()?.block_contact(id)))
                .map(|_| "contact_blocked"),
            BridgeCommand::UnblockContact { contact_id_hex } => parse_contact_id(&contact_id_hex)
                .and_then(|id| runtime_command(self.runtime()?.unblock_contact(id)))
                .map(|_| "contact_unblocked"),
            BridgeCommand::RemoveContact { contact_id_hex } => parse_contact_id(&contact_id_hex)
                .and_then(|id| runtime_command(self.runtime()?.remove_contact(id)))
                .map(|_| "contact_removed"),
            BridgeCommand::ClearConversationHistory { conversation_id_hex } => {
                parse_conversation_id(&conversation_id_hex)
                    .and_then(|id| runtime_command(self.runtime()?.clear_conversation_history(id)))
                    .map(|_| "conversation_history_cleared")
            }
            BridgeCommand::QueueMessage {
                message_id_hex,
                conversation_id_hex,
                body,
                reply_to_message_id_hex,
                at_ms,
            } => self.runtime().and_then(|runtime| {
                let message_id = parse_id(&message_id_hex)?;
                let conversation_id = parse_id(&conversation_id_hex)?;
                let body = MessageBody::new(body).map_err(string_error)?;
                let reply_to = match reply_to_message_id_hex {
                    Some(value) => Some(ReplyReference {
                        message_id: MessageId::from_opaque(parse_id(&value)?),
                    }),
                    None => None,
                };
                let at = timestamp(at_ms)?;
                self.engine
                    .dispatch(EngineCommand::QueueMessage {
                        message_id: MessageId::from_opaque(message_id),
                        conversation_id: ConversationId::from_opaque(conversation_id),
                        body,
                        reply_to,
                        at,
                    })
                    .map_err(string_error)
                    .map(|value| {
                        runtime.wake_delivery();
                        result_kind(&value)
                    })
            }),
            BridgeCommand::RetryMessage { message_id_hex, at_ms } => {
                self.runtime().and_then(|runtime| {
                    let message_id = MessageId::from_opaque(parse_id(&message_id_hex)?);
                    let at = timestamp(at_ms)?;
                    self.engine
                        .dispatch(EngineCommand::RetryMessage { message_id, at })
                        .map_err(string_error)
                        .map(|value| {
                            runtime.wake_delivery();
                            result_kind(&value)
                        })
                })
            }
            BridgeCommand::MarkConversationRead { conversation_id_hex } => {
                parse_id(&conversation_id_hex)
                    .and_then(|id| runtime_command(self.runtime()?.mark_conversation_read(id)))
                    .map(|_| "conversation_read")
            }
            BridgeCommand::MarkConversationReadWithPolicy { conversation_id_hex, send_receipt } => {
                parse_id(&conversation_id_hex)
                    .and_then(|id| {
                        runtime_command(
                            self.runtime()?.mark_conversation_read_with_policy(id, send_receipt),
                        )
                    })
                    .map(|_| "conversation_read")
            }
            BridgeCommand::QueueAttachment {
                attachment_id_hex,
                message_id_hex,
                conversation_id_hex,
                source_path,
                name,
                media_type,
                size,
            } => parse_attachment_request(
                attachment_id_hex,
                message_id_hex,
                conversation_id_hex,
                source_path,
                name,
                media_type,
                size,
            )
            .and_then(|request| runtime_command(self.runtime()?.queue_attachment(request)))
            .map(|_| "attachment_queued"),
            BridgeCommand::RetryAttachment { attachment_id_hex } => parse_id(&attachment_id_hex)
                .and_then(|id| runtime_command(self.runtime()?.retry_attachment(id)))
                .map(|_| "attachment_retried"),
            BridgeCommand::CancelAttachment { attachment_id_hex } => parse_id(&attachment_id_hex)
                .and_then(|id| runtime_command(self.runtime()?.cancel_attachment(id)))
                .map(|_| "attachment_cancelled"),
            BridgeCommand::ExportAttachment { attachment_id_hex, destination_path } => {
                parse_id(&attachment_id_hex)
                    .and_then(|id| {
                        self.runtime()?
                            .export_attachment(
                                AttachmentId::from_opaque(id),
                                PathBuf::from(destination_path),
                            )
                            .map_err(string_error)
                    })
                    .map(|_| "attachment_exported")
            }
            BridgeCommand::RefreshSnapshot => {
                self.snapshot().map(|_| "snapshot").map_err(string_error)
            }
        };
        match result {
            Ok(kind) => BridgeResult { ok: true, kind: kind.into(), error: None },
            Err(error) => BridgeResult { ok: false, kind: "error".into(), error: Some(error) },
        }
    }

    pub fn snapshot(&self) -> Result<BridgeSnapshot, EngineError> {
        self.snapshot_internal(false)
    }
    pub fn full_snapshot(&self) -> Result<BridgeSnapshot, EngineError> {
        self.snapshot_internal(true)
    }

    fn snapshot_internal(&self, include_messages: bool) -> Result<BridgeSnapshot, EngineError> {
        let app = if include_messages {
            self.engine.snapshot()?
        } else {
            self.engine.overview_snapshot()?
        };
        let (network, attachments) = match &self.runtime {
            Some(runtime) => (
                runtime
                    .network_snapshot()
                    .map_err(|_| EngineError("network snapshot unavailable".into()))?,
                runtime.attachment_snapshot().unwrap_or_default(),
            ),
            None => (
                NetworkSnapshot {
                    tor: TorState::Stopped,
                    onion_address: None,
                    peers: BTreeMap::new(),
                    peer_health: BTreeMap::new(),
                    contact_names: BTreeMap::new(),
                    contact_verifications: BTreeMap::new(),
                    probes: Vec::new(),
                },
                Vec::new(),
            ),
        };
        let bootstrap =
            self.bootstrap.lock().map_err(|_| EngineError("bootstrap state unavailable".into()))?;
        Ok(map_snapshot(app, network, attachments, include_messages, &bootstrap))
    }

    /// Applies externally observed bootstrap facts. The native runtime actor
    /// calls this before projecting a snapshot; `snapshot()` itself is strictly
    /// read-only so polling can never advance attempts or mutate the state.
    pub fn advance_bootstrap(&self) -> Result<(), EngineError> {
        let app = self.engine.overview_snapshot()?;
        let network = match &self.runtime {
            Some(runtime) => runtime
                .network_snapshot()
                .map_err(|_| EngineError("network snapshot unavailable".into()))?,
            None => NetworkSnapshot {
                tor: TorState::Stopped,
                onion_address: None,
                peers: BTreeMap::new(),
                peer_health: BTreeMap::new(),
                contact_names: BTreeMap::new(),
                contact_verifications: BTreeMap::new(),
                probes: Vec::new(),
            },
        };
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
                                != Some(BootstrapStepState::Failed)
                            {
                                bootstrap.begin(BootstrapStepId::Relay);
                                bootstrap.fail(BootstrapStepId::Relay, "RELAY_UNREACHABLE");
                            }
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

    pub fn diagnostics_json(&self) -> Result<String, EngineError> {
        match &self.runtime {
            Some(runtime) => runtime
                .diagnostics_json()
                .map_err(|_| EngineError("diagnostics unavailable".into())),
            None => Ok("{\"events\":[]}".into()),
        }
    }
    fn runtime(&self) -> Result<&RuntimeHandle, String> {
        self.runtime.as_ref().ok_or_else(|| "secure network runtime is not ready".into())
    }

    fn pairing_creation_allowed(&self) -> Result<(), String> {
        let relay = self
            .runtime()?
            .network_snapshot()
            .map_err(|_| "relay health is unavailable".to_owned())?
            .probes
            .into_iter()
            .find(|probe| probe.target == ProbeTarget::Relay);
        match relay.map(|probe| probe.status) {
            Some(ProbeStatus::Healthy) => Ok(()),
            Some(ProbeStatus::Degraded | ProbeStatus::Failed | ProbeStatus::Unreachable) => {
                Err("RELAY_DEGRADED".into())
            }
            _ => Err("RELAY_NOT_READY".into()),
        }
    }

    fn profile_setup_allowed(&self) -> Result<(), String> {
        let network = self.runtime().and_then(|runtime| {
            runtime.network_snapshot().map_err(|_| "network readiness is unavailable".to_owned())
        })?;
        if network.tor != TorState::Ready || network.onion_address.is_none() {
            return Err("PROFILE_NOT_READY".into());
        }
        let relay = network
            .probes
            .into_iter()
            .find(|probe| probe.target == ProbeTarget::Relay)
            .map(|probe| probe.status);
        match relay {
            Some(
                ProbeStatus::Healthy
                | ProbeStatus::Degraded
                | ProbeStatus::Failed
                | ProbeStatus::Unreachable,
            ) => Ok(()),
            Some(ProbeStatus::Checking | ProbeStatus::Unknown | ProbeStatus::Disabled) | None => {
                Err("PROFILE_NOT_READY".into())
            }
        }
    }
}

pub fn bridge_message_from_domain(message: Message) -> BridgeMessage {
    BridgeMessage {
        id: message.id().to_string(),
        conversation_id: message.conversation_id().to_string(),
        body: message.body().as_str().to_owned(),
        direction: format!("{:?}", message.direction()).to_lowercase(),
        status: format!("{:?}", message.status()).to_lowercase(),
        reply_to_message_id: message.reply_to().map(|reply| reply.message_id.to_string()),
        created_at_ms: message.created_at().to_unix_millis(),
        updated_at_ms: message.updated_at().to_unix_millis(),
        attempt_count: u32::try_from(message.attempts().len()).unwrap_or(u32::MAX),
    }
}

fn runtime_command<T>(result: Result<T, RuntimeDriverError>) -> Result<(), String> {
    match result {
        Ok(_) | Err(RuntimeDriverError::Pending) => Ok(()),
        Err(error) => Err(string_error(error)),
    }
}
fn parse_attachment_request(
    attachment_id_hex: String,
    message_id_hex: String,
    conversation_id_hex: String,
    source_path: String,
    name: String,
    media_type: String,
    size: u64,
) -> Result<AttachmentSendRequest, String> {
    Ok(AttachmentSendRequest {
        attachment_id: parse_id(&attachment_id_hex)?,
        message_id: parse_id(&message_id_hex)?,
        conversation_id: parse_id(&conversation_id_hex)?,
        source_path,
        name,
        media_type,
        size,
    })
}
fn parse_pairing_id(value: &str) -> Result<PairingSessionId, String> {
    parse_id(value).map(PairingSessionId::from_opaque)
}
fn parse_contact_id(value: &str) -> Result<ContactId, String> {
    parse_id(value).map(ContactId::from_opaque)
}
fn parse_conversation_id(value: &str) -> Result<ConversationId, String> {
    parse_id(value).map(ConversationId::from_opaque)
}
fn parse_id(value: &str) -> Result<OpaqueId, String> {
    value.parse::<OpaqueId>().map_err(string_error)
}
fn timestamp(value: i64) -> Result<Timestamp, String> {
    Timestamp::from_unix_millis(value).map_err(string_error)
}
fn string_error(error: impl core::fmt::Display) -> String {
    error.to_string()
}
fn bootstrap_phase_name(phase: BootstrapPhase) -> &'static str {
    match phase {
        BootstrapPhase::Idle => "idle",
        BootstrapPhase::Starting => "starting",
        BootstrapPhase::ReadyForProfile => "ready_for_profile",
        BootstrapPhase::Ready => "ready",
        BootstrapPhase::Degraded => "degraded",
        BootstrapPhase::Failed => "failed",
    }
}

fn step_state(bootstrap: &BootstrapState, id: BootstrapStepId) -> Option<BootstrapStepState> {
    bootstrap.snapshot().steps.into_iter().find(|step| step.id == id).map(|step| step.state)
}

fn bootstrap_step_id(id: BootstrapStepId) -> &'static str {
    match id {
        BootstrapStepId::Preferences
        | BootstrapStepId::NativeBridge
        | BootstrapStepId::Contract
        | BootstrapStepId::SecureStorage
        | BootstrapStepId::Database => "local_storage",
        BootstrapStepId::DeviceIdentity => "device_identity",
        BootstrapStepId::Tor => "tor_network",
        BootstrapStepId::OnionService => "onion_service",
        BootstrapStepId::Relay => "secure_relay",
        BootstrapStepId::UserProfile => "profile",
    }
}
fn result_kind(value: &EngineResult) -> &'static str {
    match value {
        EngineResult::IdentityCreated => "identity_created",
        EngineResult::ProfileUpdated => "profile_updated",
        EngineResult::PairingStarted => "pairing_started",
        EngineResult::PairingJoined => "pairing_joined",
        EngineResult::PairingUpdated => "pairing_updated",
        EngineResult::PairingRejected => "pairing_rejected",
        EngineResult::PairingCancelled => "pairing_cancelled",
        EngineResult::PairingCompleted { .. } => "pairing_completed",
        EngineResult::MessageQueued { .. } => "message_queued",
        EngineResult::MessageUpdated { .. } => "message_updated",
        EngineResult::ReceiptApplied { .. } => "receipt_applied",
    }
}

fn map_snapshot(
    snapshot: ClientSnapshot,
    network: NetworkSnapshot,
    attachments: Vec<AttachmentView>,
    include_messages: bool,
    bootstrap: &BootstrapState,
) -> BridgeSnapshot {
    let local_public = snapshot.identity.as_ref().map(|identity| identity.public().clone());
    let identity_name = snapshot.identity.as_ref().and_then(|identity| {
        identity.profile().map(|profile| profile.display_name().as_str().to_owned())
    });
    let identity_fingerprint = snapshot.identity.as_ref().map(|identity| {
        let mut hash = Sha256::new();
        hash.update(b"TORCA-FINGERPRINT-V1");
        hash.update(identity.public().key().public_key());
        grouped_hex(&hash.finalize())
    });
    let summaries =
        if include_messages { summarize_messages(&snapshot.messages) } else { BTreeMap::new() };
    let messages = if include_messages {
        snapshot.messages.into_iter().map(bridge_message_from_domain).collect()
    } else {
        Vec::new()
    };
    let tor_state = format!("{:?}", network.tor).to_lowercase();
    let bootstrap_snapshot = bootstrap.snapshot();
    let bootstrap_phase = bootstrap_phase_name(bootstrap_snapshot.phase);
    BridgeSnapshot {
        contract_version: CONTRACT_VERSION,
        identity_name,
        identity_fingerprint,
        tor_state: tor_state.clone(),
        onion_address: network.onion_address,
        bootstrap_phase: bootstrap_phase.into(),
        bootstrap_steps: bootstrap_snapshot
            .steps
            .into_iter()
            .filter(|step| {
                matches!(
                    step.id,
                    BootstrapStepId::SecureStorage
                        | BootstrapStepId::DeviceIdentity
                        | BootstrapStepId::Tor
                        | BootstrapStepId::OnionService
                        | BootstrapStepId::Relay
                )
            })
            .map(|step| BridgeBootstrapStep {
                id: bootstrap_step_id(step.id).into(),
                state: format!("{:?}", step.state).to_lowercase(),
                code: step.diagnostic_code,
                progress: u8::from(step.state == BootstrapStepState::Ready) * 100,
                attempt: step.attempt,
                started_at_ms: None,
                last_progress_at_ms: None,
                retry_at_ms: None,
            })
            .collect(),
        pairings: snapshot
            .pairings
            .into_iter()
            .map(|pairing| BridgePairing {
                id: pairing.id().to_string(),
                code: pairing.code().as_str().to_owned(),
                role: format!("{:?}", pairing.role()).to_lowercase(),
                state: format!("{:?}", pairing.state()).to_lowercase(),
                expires_at_ms: pairing.expires_at().to_unix_millis(),
                local_approved: pairing.local_approved(),
                remote_approved: pairing.remote_approved(),
            })
            .collect(),
        contacts: snapshot
            .contacts
            .into_iter()
            .map(|contact| {
                let connection_state = network.peers.get(&contact.id()).map_or_else(
                    || "disconnected".to_owned(),
                    |state| format!("{state:?}").to_lowercase(),
                );
                let peer_health = network.peer_health.get(&contact.id()).map_or_else(
                    || BridgePeerHealth {
                        state: connection_state.clone(),
                        quality: "unknown".into(),
                        rtt_ms: None,
                        last_success_at_ms: None,
                        consecutive_failures: 0,
                        reconnect_attempt: 0,
                    },
                    |health| BridgePeerHealth {
                        state: format!("{:?}", health.state).to_lowercase(),
                        quality: format!("{:?}", health.quality).to_lowercase(),
                        rtt_ms: health.rtt_ms,
                        last_success_at_ms: health.last_success_at.map(Timestamp::to_unix_millis),
                        consecutive_failures: health.consecutive_failures,
                        reconnect_attempt: health.reconnect_attempt,
                    },
                );
                let safety_number = local_public.as_ref().map_or_else(String::new, |local| {
                    safety_number(local, contact.remote_identity())
                });
                let display_name = network
                    .contact_names
                    .get(&contact.id())
                    .cloned()
                    .unwrap_or_else(|| fallback_contact_name(contact.id()));
                let verification =
                    network.contact_verifications.get(&contact.id()).copied().unwrap_or_default();
                BridgeContact {
                    id: contact.id().to_string(),
                    display_name,
                    onion_address: contact.route().onion_address().to_owned(),
                    status: format!("{:?}", contact.status()).to_lowercase(),
                    connection_state: peer_health.state.clone(),
                    safety_number,
                    peer_health,
                    verification_status: if verification.verified {
                        "verified".into()
                    } else {
                        "unverified".into()
                    },
                    verified_at_ms: verification.verified_at.map(Timestamp::to_unix_millis),
                }
            })
            .collect(),
        conversations: snapshot
            .conversations
            .into_iter()
            .map(|conversation| {
                let summary = summaries.get(&conversation.id());
                BridgeConversation {
                    id: conversation.id().to_string(),
                    contact_id: conversation.contact_id().to_string(),
                    status: format!("{:?}", conversation.status()).to_lowercase(),
                    unread_count: summary.map_or(0, |value| value.0),
                    last_activity_at_ms: summary.map_or(0, |value| value.1),
                    last_message_body: summary.and_then(|value| {
                        value.2.as_ref().map(|message| message.body().as_str().to_owned())
                    }),
                    last_message_direction: summary.and_then(|value| {
                        value
                            .2
                            .as_ref()
                            .map(|message| format!("{:?}", message.direction()).to_lowercase())
                    }),
                    last_message_status: summary.and_then(|value| {
                        value
                            .2
                            .as_ref()
                            .map(|message| format!("{:?}", message.status()).to_lowercase())
                    }),
                }
            })
            .collect(),
        messages,
        attachments: attachments
            .into_iter()
            .map(|attachment| BridgeAttachment {
                id: attachment.id.to_string(),
                message_id: attachment.message_id.to_string(),
                name: attachment.name,
                media_type: attachment.media_type,
                size: attachment.size,
                status: attachment.status,
                offset: attachment.offset,
            })
            .collect(),
    }
}

fn summarize_messages(
    messages: &[Message],
) -> BTreeMap<ConversationId, (u32, i64, Option<Message>)> {
    let mut values = BTreeMap::new();
    for message in messages {
        let entry = values.entry(message.conversation_id()).or_insert((0_u32, 0_i64, None));
        if message.direction() == MessageDirection::Inbound
            && message.status() == MessageStatus::Delivered
        {
            entry.0 = entry.0.saturating_add(1);
        }
        let activity =
            message.updated_at().to_unix_millis().max(message.created_at().to_unix_millis());
        if entry.2.is_none() || activity >= entry.1 {
            entry.1 = activity;
            entry.2 = Some(message.clone());
        }
    }
    values
}
fn fallback_contact_name(id: ContactId) -> String {
    let value = id.to_string();
    let short = value.get(..8).unwrap_or(&value);
    format!("Contact {short}")
}
fn safety_number(local: &PublicIdentity, remote: &PublicIdentity) -> String {
    let (first, second) = if local.identity_id().to_opaque() <= remote.identity_id().to_opaque() {
        (local, remote)
    } else {
        (remote, local)
    };
    let mut hash = Sha256::new();
    hash.update(b"TORCA-SAFETY-NUMBER-V1");
    update_identity_hash(&mut hash, first);
    update_identity_hash(&mut hash, second);
    grouped_hex(&hash.finalize())
}
fn grouped_hex(bytes: &[u8]) -> String {
    bytes
        .chunks(4)
        .map(|chunk| {
            let mut value = String::with_capacity(chunk.len() * 2);
            for byte in chunk {
                let _ = write!(value, "{byte:02X}");
            }
            value
        })
        .collect::<Vec<_>>()
        .join(" ")
}
fn update_identity_hash(hash: &mut Sha256, identity: &PublicIdentity) {
    hash.update(identity.identity_id().to_opaque().as_bytes());
    let key = identity.key().public_key();
    hash.update(u32::try_from(key.len()).unwrap_or(u32::MAX).to_be_bytes());
    hash.update(key);
}
pub fn dart_contract_source() -> &'static str {
    include_str!("../schema/torca_contract.dart")
}
