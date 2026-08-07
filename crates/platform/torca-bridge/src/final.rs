//! Stable typed boundary between Flutter and the process-owned Rust runtime.

use torca_client_engine::{ClientSnapshot, EngineCommand, EngineError, EngineHandle, EngineResult};
use torca_conversations::ConversationId;
use torca_foundation::{OpaqueId, Timestamp};
use torca_identity::{IdentityId, Profile, ProfileName};
use torca_messaging::{MessageBody, MessageId};
use torca_pairing::{PairingCode, PairingSessionId};
use torca_runtime_host::{NetworkSnapshot, RuntimeHostHandle};

pub const CONTRACT_VERSION: u16 = 4;

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BridgeCommand {
    CreateIdentity { identity_id_hex: String, display_name: String, at_ms: i64 },
    CreatePairing { session_id_hex: String },
    JoinPairing { session_id_hex: String, code: String },
    ApprovePairing { session_id_hex: String },
    RejectPairing { session_id_hex: String },
    CancelPairing { session_id_hex: String },
    QueueMessage { message_id_hex: String, conversation_id_hex: String, body: String, at_ms: i64 },
    MarkConversationRead { conversation_id_hex: String },
    RefreshSnapshot,
}
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BridgeResult { pub ok: bool, pub kind: String, pub error: Option<String> }
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BridgeSnapshot {
    pub contract_version: u16,
    pub identity_name: Option<String>,
    pub tor_state: String,
    pub onion_address: Option<String>,
    pub pairings: Vec<BridgePairing>,
    pub contacts: Vec<BridgeContact>,
    pub conversations: Vec<BridgeConversation>,
    pub messages: Vec<BridgeMessage>,
}
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BridgePairing {
    pub id: String, pub code: String, pub role: String, pub state: String,
    pub expires_at_ms: i64, pub local_approved: bool, pub remote_approved: bool,
}
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BridgeContact {
    pub id: String, pub onion_address: String, pub status: String, pub connection_state: String,
}
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BridgeConversation { pub id: String, pub contact_id: String, pub status: String }
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BridgeMessage {
    pub id: String, pub conversation_id: String, pub body: String,
    pub direction: String, pub status: String,
}

pub struct EngineBridge { engine: EngineHandle, runtime: RuntimeHostHandle }
impl EngineBridge {
    pub const fn new(engine: EngineHandle, runtime: RuntimeHostHandle) -> Self { Self { engine, runtime } }

    pub fn execute(&self, command: BridgeCommand) -> BridgeResult {
        let result: Result<&'static str, String> = match command {
            BridgeCommand::CreateIdentity { identity_id_hex, display_name, at_ms } => {
                parse_id(&identity_id_hex)
                    .and_then(|id| ProfileName::new(display_name).map_err(string_error).map(|n| (id, n)))
                    .and_then(|(id, name)| timestamp(at_ms).map(|at| EngineCommand::CreateIdentity {
                        identity_id: IdentityId::from_opaque(id), profile: Profile::new(name, None), at,
                    }))
                    .and_then(|command| self.engine.dispatch(command).map_err(string_error))
                    .map(|value| result_kind(&value))
            }
            BridgeCommand::CreatePairing { session_id_hex } => parse_pairing_id(&session_id_hex)
                .and_then(|id| self.runtime.create_pairing(id).map_err(string_error))
                .map(|_| "pairing_started"),
            BridgeCommand::JoinPairing { session_id_hex, code } => parse_pairing_id(&session_id_hex)
                .and_then(|id| PairingCode::new(code).map_err(string_error).map(|code| (id, code)))
                .and_then(|(id, code)| self.runtime.join_pairing(id, code).map_err(string_error))
                .map(|_| "pairing_joined"),
            BridgeCommand::ApprovePairing { session_id_hex } => parse_pairing_id(&session_id_hex)
                .and_then(|id| self.runtime.approve_pairing(id).map_err(string_error))
                .map(|_| "pairing_updated"),
            BridgeCommand::RejectPairing { session_id_hex } => parse_pairing_id(&session_id_hex)
                .and_then(|id| self.runtime.reject_pairing(id).map_err(string_error))
                .map(|_| "pairing_rejected"),
            BridgeCommand::CancelPairing { session_id_hex } => parse_pairing_id(&session_id_hex)
                .and_then(|id| self.runtime.cancel_pairing(id).map_err(string_error))
                .map(|_| "pairing_cancelled"),
            BridgeCommand::QueueMessage { message_id_hex, conversation_id_hex, body, at_ms } => {
                parse_id(&message_id_hex)
                    .and_then(|message_id| parse_id(&conversation_id_hex).map(|c| (message_id, c)))
                    .and_then(|(message_id, conversation_id)| MessageBody::new(body).map_err(string_error)
                        .map(|body| (message_id, conversation_id, body)))
                    .and_then(|(message_id, conversation_id, body)| timestamp(at_ms).map(|at| EngineCommand::QueueMessage {
                        message_id: MessageId::from_opaque(message_id),
                        conversation_id: ConversationId::from_opaque(conversation_id), body, reply_to: None, at,
                    }))
                    .and_then(|command| self.engine.dispatch(command).map_err(string_error))
                    .map(|value| { self.runtime.wake_delivery(); result_kind(&value) })
            }
            BridgeCommand::MarkConversationRead { conversation_id_hex } => parse_id(&conversation_id_hex)
                .and_then(|id| self.runtime.mark_conversation_read(id).map_err(string_error))
                .map(|_| "conversation_read"),
            BridgeCommand::RefreshSnapshot => self.snapshot().map(|_| "snapshot").map_err(string_error),
        };
        match result {
            Ok(kind) => BridgeResult { ok: true, kind: kind.into(), error: None },
            Err(error) => BridgeResult { ok: false, kind: "error".into(), error: Some(error) },
        }
    }

    pub fn snapshot(&self) -> Result<BridgeSnapshot, EngineError> {
        let app = self.engine.snapshot()?;
        let network = self.runtime.network_snapshot()
            .map_err(|_| EngineError("network snapshot unavailable".into()))?;
        Ok(map_snapshot(app, network))
    }

    pub fn diagnostics_json(&self) -> Result<String, EngineError> {
        self.runtime.diagnostics_json().map_err(|_| EngineError("diagnostics unavailable".into()))
    }
}

fn parse_pairing_id(value: &str) -> Result<PairingSessionId, String> { parse_id(value).map(PairingSessionId::from_opaque) }
fn parse_id(value: &str) -> Result<OpaqueId, String> { value.parse::<OpaqueId>().map_err(string_error) }
fn timestamp(value: i64) -> Result<Timestamp, String> { Timestamp::from_unix_millis(value).map_err(string_error) }
fn string_error(error: impl core::fmt::Display) -> String { error.to_string() }
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
fn map_snapshot(snapshot: ClientSnapshot, network: NetworkSnapshot) -> BridgeSnapshot {
    BridgeSnapshot {
        contract_version: CONTRACT_VERSION,
        identity_name: snapshot.identity.map(|i| i.profile().display_name().as_str().to_owned()),
        tor_state: format!("{:?}", network.tor).to_lowercase(),
        onion_address: network.onion_address,
        pairings: snapshot.pairings.into_iter().map(|p| BridgePairing {
            id: p.id().to_string(), code: p.code().as_str().to_owned(),
            role: format!("{:?}", p.role()).to_lowercase(), state: format!("{:?}", p.state()).to_lowercase(),
            expires_at_ms: p.expires_at().to_unix_millis(), local_approved: p.local_approved(), remote_approved: p.remote_approved(),
        }).collect(),
        contacts: snapshot.contacts.into_iter().map(|c| {
            let connection_state = network.peers.get(&c.id())
                .map_or_else(|| "disconnected".to_owned(), |s| format!("{s:?}").to_lowercase());
            BridgeContact { id: c.id().to_string(), onion_address: c.route().onion_address().to_owned(),
                status: format!("{:?}", c.status()).to_lowercase(), connection_state }
        }).collect(),
        conversations: snapshot.conversations.into_iter().map(|c| BridgeConversation {
            id: c.id().to_string(), contact_id: c.contact_id().to_string(), status: format!("{:?}", c.status()).to_lowercase(),
        }).collect(),
        messages: snapshot.messages.into_iter().map(|m| BridgeMessage {
            id: m.id().to_string(), conversation_id: m.conversation_id().to_string(), body: m.body().as_str().to_owned(),
            direction: format!("{:?}", m.direction()).to_lowercase(), status: format!("{:?}", m.status()).to_lowercase(),
        }).collect(),
    }
}
pub fn dart_contract_source() -> &'static str { include_str!("../schema/torca_contract.dart") }
