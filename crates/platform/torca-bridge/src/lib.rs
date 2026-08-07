//! Stable typed boundary between Flutter hosts and the Rust ClientEngine.

use torca_client_engine::{ClientSnapshot, EngineCommand, EngineError, EngineHandle, EngineResult};
use torca_conversations::ConversationId;
use torca_foundation::{OpaqueId, Timestamp};
use torca_identity::{IdentityId, Profile, ProfileName};
use torca_messaging::{MessageBody, MessageId};
use torca_pairing::{PairingCode, PairingSessionId};

/// Version of the cross-language contract.
pub const CONTRACT_VERSION: u16 = 3;

/// Primitive bridge commands suitable for generated language bindings.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BridgeCommand {
    CreateIdentity { identity_id_hex: String, display_name: String, at_ms: i64 },
    StartPairing { session_id_hex: String, code: String, expires_at_ms: i64 },
    JoinPairing { session_id_hex: String, code: String, expires_at_ms: i64 },
    ApprovePairing { session_id_hex: String, at_ms: i64 },
    RejectPairing { session_id_hex: String },
    CancelPairing { session_id_hex: String },
    QueueMessage { message_id_hex: String, conversation_id_hex: String, body: String, at_ms: i64 },
    RefreshSnapshot,
}
/// Primitive bridge result.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BridgeResult {
    pub ok: bool,
    pub kind: String,
    pub error: Option<String>,
}
/// Presentation-safe snapshot DTO.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BridgeSnapshot {
    pub contract_version: u16,
    pub identity_name: Option<String>,
    pub pairings: Vec<BridgePairing>,
    pub contacts: Vec<BridgeContact>,
    pub conversations: Vec<BridgeConversation>,
    pub messages: Vec<BridgeMessage>,
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
pub struct BridgeContact {
    pub id: String,
    pub onion_address: String,
    pub status: String,
}
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BridgeConversation {
    pub id: String,
    pub contact_id: String,
    pub status: String,
}
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BridgeMessage {
    pub id: String,
    pub conversation_id: String,
    pub body: String,
    pub direction: String,
    pub status: String,
}

/// Engine-backed bridge facade.
pub struct EngineBridge {
    engine: EngineHandle,
}
impl EngineBridge {
    /// Creates a bridge.
    pub const fn new(engine: EngineHandle) -> Self {
        Self { engine }
    }
    /// Executes a typed bridge command.
    pub fn execute(&self, command: BridgeCommand) -> BridgeResult {
        let result = match command {
            BridgeCommand::CreateIdentity { identity_id_hex, display_name, at_ms } => {
                parse_id(&identity_id_hex)
                    .and_then(|id| {
                        ProfileName::new(display_name).map_err(string_error).map(|name| (id, name))
                    })
                    .and_then(|(id, name)| {
                        timestamp(at_ms).map(|at| EngineCommand::CreateIdentity {
                            identity_id: IdentityId::from_opaque(id),
                            profile: Profile::new(name, None),
                            at,
                        })
                    })
                    .and_then(|command| self.engine.dispatch(command).map_err(string_error))
            }
            BridgeCommand::StartPairing { session_id_hex, code, expires_at_ms } => {
                pairing_open_command(session_id_hex, code, expires_at_ms, false)
                    .and_then(|command| self.engine.dispatch(command).map_err(string_error))
            }
            BridgeCommand::JoinPairing { session_id_hex, code, expires_at_ms } => {
                pairing_open_command(session_id_hex, code, expires_at_ms, true)
                    .and_then(|command| self.engine.dispatch(command).map_err(string_error))
            }
            BridgeCommand::ApprovePairing { session_id_hex, at_ms } => {
                parse_pairing_id(&session_id_hex)
                    .and_then(|session_id| {
                        timestamp(at_ms).map(|at| EngineCommand::ApprovePairing { session_id, at })
                    })
                    .and_then(|command| self.engine.dispatch(command).map_err(string_error))
            }
            BridgeCommand::RejectPairing { session_id_hex } => parse_pairing_id(&session_id_hex)
                .map(|session_id| EngineCommand::RejectPairing { session_id })
                .and_then(|command| self.engine.dispatch(command).map_err(string_error)),
            BridgeCommand::CancelPairing { session_id_hex } => parse_pairing_id(&session_id_hex)
                .map(|session_id| EngineCommand::CancelPairing { session_id })
                .and_then(|command| self.engine.dispatch(command).map_err(string_error)),
            BridgeCommand::QueueMessage { message_id_hex, conversation_id_hex, body, at_ms } => {
                parse_id(&message_id_hex)
                    .and_then(|message_id| {
                        parse_id(&conversation_id_hex)
                            .map(|conversation_id| (message_id, conversation_id))
                    })
                    .and_then(|(message_id, conversation_id)| {
                        MessageBody::new(body)
                            .map_err(string_error)
                            .map(|body| (message_id, conversation_id, body))
                    })
                    .and_then(|(message_id, conversation_id, body)| {
                        timestamp(at_ms).map(|at| EngineCommand::QueueMessage {
                            message_id: MessageId::from_opaque(message_id),
                            conversation_id: ConversationId::from_opaque(conversation_id),
                            body,
                            reply_to: None,
                            at,
                        })
                    })
                    .and_then(|command| self.engine.dispatch(command).map_err(string_error))
            }
            BridgeCommand::RefreshSnapshot => {
                return match self.snapshot() {
                    Ok(_) => BridgeResult { ok: true, kind: "snapshot".into(), error: None },
                    Err(error) => {
                        BridgeResult { ok: false, kind: "error".into(), error: Some(error.0) }
                    }
                };
            }
        };
        match result {
            Ok(value) => BridgeResult { ok: true, kind: result_kind(&value).into(), error: None },
            Err(error) => BridgeResult { ok: false, kind: "error".into(), error: Some(error) },
        }
    }
    /// Reads the current snapshot.
    pub fn snapshot(&self) -> Result<BridgeSnapshot, EngineError> {
        self.engine.snapshot().map(map_snapshot)
    }
}

fn pairing_open_command(
    session_id_hex: String,
    code: String,
    expires_at_ms: i64,
    joining: bool,
) -> Result<EngineCommand, String> {
    parse_pairing_id(&session_id_hex)
        .and_then(|session_id| {
            PairingCode::new(code).map_err(string_error).map(|code| (session_id, code))
        })
        .and_then(|(session_id, code)| {
            timestamp(expires_at_ms).map(|expires_at| {
                if joining {
                    EngineCommand::JoinPairing { session_id, code, expires_at }
                } else {
                    EngineCommand::StartPairing { session_id, code, expires_at }
                }
            })
        })
}
fn parse_pairing_id(value: &str) -> Result<PairingSessionId, String> {
    parse_id(value).map(PairingSessionId::from_opaque)
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
fn result_kind(value: &EngineResult) -> &'static str {
    match value {
        EngineResult::IdentityCreated => "identity_created",
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
fn map_snapshot(snapshot: ClientSnapshot) -> BridgeSnapshot {
    BridgeSnapshot {
        contract_version: CONTRACT_VERSION,
        identity_name: snapshot
            .identity
            .map(|identity| identity.profile().display_name().as_str().to_owned()),
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
            .map(|contact| BridgeContact {
                id: contact.id().to_string(),
                onion_address: contact.route().onion_address().to_owned(),
                status: format!("{:?}", contact.status()).to_lowercase(),
            })
            .collect(),
        conversations: snapshot
            .conversations
            .into_iter()
            .map(|conversation| BridgeConversation {
                id: conversation.id().to_string(),
                contact_id: conversation.contact_id().to_string(),
                status: format!("{:?}", conversation.status()).to_lowercase(),
            })
            .collect(),
        messages: snapshot
            .messages
            .into_iter()
            .map(|message| BridgeMessage {
                id: message.id().to_string(),
                conversation_id: message.conversation_id().to_string(),
                body: message.body().as_str().to_owned(),
                direction: format!("{:?}", message.direction()).to_lowercase(),
                status: format!("{:?}", message.status()).to_lowercase(),
            })
            .collect(),
    }
}
/// Returns the deterministic generated Dart contract source.
pub fn dart_contract_source() -> &'static str {
    include_str!("../schema/torca_contract.dart")
}
