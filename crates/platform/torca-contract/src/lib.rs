//! Stable typed boundary between Flutter and the process-owned Rust runtime.

use serde::Serialize;
use torca_client_application::{
    ApplicationCommand, ApplicationError, ApplicationSnapshotContext, BootstrapPhase,
    BootstrapStepId, BootstrapStepState, PeerConnectionStatus, PeerHealthQuality,
    PendingOperationKind, ProbeStatus, ProbeTarget, TorState,
};
use torca_contacts::{ContactId, ContactStatus};
use torca_conversations::ConversationStatus;
use torca_foundation::{ClassifiedError, OpaqueId, Timestamp};
use torca_messaging::{Message, MessageDirection, MessageStatus};
use torca_pairing::{PairingRole, PairingState};
use torca_pairing_protocol::encode_invite_uri;

pub mod generated {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/generated_contract.rs"));
}

pub const CONTRACT_VERSION: u16 = generated::CONTRACT_VERSION;

/// Serializes the contract-owned notification cursor query used by platform
/// notification consumers. Native hosts must not hand-assemble ABI requests.
#[must_use]
pub fn notification_poll_request_json(request_id: &str, after_cursor: u64) -> String {
    serde_json::json!({
        "schema": 1,
        "requestId": request_id,
        "kind": "query",
        "name": "notifications.poll",
        "payload": { "afterCursor": after_cursor },
    })
    .to_string()
}

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BridgeCommand {
    SetNotifications {
        enabled: bool,
    },
    SetReadReceipts {
        enabled: bool,
    },
    AcknowledgeNewContacts,
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
        ticket: Option<String>,
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
    StartConversation {
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
    QueueAttachment {
        attachment_id_hex: String,
        message_id_hex: String,
        conversation_id_hex: String,
        source_path: String,
        name: String,
        media_type: String,
        size: u64,
        at_ms: i64,
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
    /// Stable machine-readable code; never inferred from the diagnostic message.
    pub error_code: Option<String>,
    pub resource_id: Option<String>,
    pub invite_uri: Option<String>,
}
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeSnapshot {
    pub contract_version: u16,
    #[serde(skip)]
    pub identity_name: Option<String>,
    #[serde(skip)]
    pub identity_fingerprint: Option<String>,
    pub tor_state: String,
    pub transport: BridgeTransportStatus,
    pub onion_address: Option<String>,
    pub pairings: Vec<BridgePairing>,
    pub contacts: Vec<BridgeContact>,
    pub conversations: Vec<BridgeConversation>,
    pub messages: Vec<BridgeMessage>,
    pub attachments: Vec<BridgeAttachment>,
    pub pending_operations: Vec<BridgePendingOperation>,
    #[serde(skip)]
    pub unread_messages_count: u32,
    #[serde(skip)]
    pub new_contacts_count: u32,
    #[serde(skip)]
    pub pairing_attention_count: u32,
    pub bootstrap_phase: String,
    pub bootstrap_steps: Vec<BridgeBootstrapStep>,
}

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgePendingOperation {
    pub id: String,
    pub resource_id: String,
    pub kind: String,
    pub state: String,
    pub dependency: String,
    pub attempts: u32,
    pub next_attempt_at_ms: i64,
    pub created_at_ms: i64,
    pub last_error: Option<String>,
}

/// Cursor-addressed, redacted notification emitted by the process runtime.
/// Message bodies, keys, onion addresses and safety numbers never cross this type.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationEvent {
    pub cursor: u64,
    pub event_id: String,
    pub kind: String,
    pub conversation_id: String,
    pub contact_display_name: String,
    pub created_at_ms: i64,
    pub title: String,
    pub body: String,
}
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
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
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgePairing {
    pub id: String,
    pub code: String,
    pub invite_uri: String,
    pub role: String,
    pub state: String,
    pub expires_at_ms: i64,
    pub local_approved: bool,
    pub remote_approved: bool,
    pub remote_identity_id: Option<String>,
    pub remote_display_name: Option<String>,
    pub remote_fingerprint: Option<String>,
}
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgePeerHealth {
    pub state: String,
    pub quality: String,
    pub rtt_ms: Option<u64>,
    pub last_success_at_ms: Option<i64>,
    pub consecutive_failures: u32,
    pub reconnect_attempt: u32,
    pub last_activity_at_ms: Option<i64>,
    pub activity_sequence: u64,
}
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeTransportIndicator {
    pub state: String,
    pub code: String,
    pub latency_ms: Option<u64>,
    pub last_activity_at_ms: Option<i64>,
    pub activity_sequence: u64,
    pub tx_sequence: u64,
    pub rx_sequence: u64,
    pub in_flight: u32,
    pub queued: u32,
}
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeTransportStatus {
    pub tor: BridgeTransportIndicator,
    pub relay: BridgeTransportIndicator,
    pub peer: BridgeTransportIndicator,
    pub peers_ready: u32,
    pub peers_total: u32,
    pub relay_info: Option<BridgeRelayInfo>,
}

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeRelayInfo {
    pub product_version: String,
    pub build_id: String,
    pub source_commit: String,
    pub protocol_version: u16,
}
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeContact {
    pub id: String,
    pub display_name: String,
    pub onion_address: String,
    pub status: String,
    pub connection_state: String,
    pub presence_state: String,
    pub last_seen_at_ms: Option<i64>,
    pub safety_number: String,
    pub peer_health: BridgePeerHealth,
    pub verification_status: String,
    pub verified_at_ms: Option<i64>,
    pub created_at_ms: i64,
}
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
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
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeMessage {
    pub id: String,
    pub conversation_id: String,
    pub body: String,
    pub direction: String,
    pub status: String,
    pub reply_to_message_id: Option<String>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub sent_at_ms: Option<i64>,
    pub delivered_at_ms: Option<i64>,
    pub read_at_ms: Option<i64>,
    pub attempt_count: u32,
}
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeMessagePage {
    pub messages: Vec<BridgeMessage>,
    pub has_more: bool,
}
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeAttachment {
    pub id: String,
    pub message_id: String,
    pub name: String,
    pub media_type: String,
    pub size: u64,
    pub status: String,
    pub offset: u64,
    pub attempt_count: u32,
    pub updated_at_ms: i64,
    pub direction: String,
    pub last_error_code: Option<String>,
}

pub fn decode_application_command(command: BridgeCommand) -> Result<ApplicationCommand, String> {
    Ok(match command {
        BridgeCommand::SetNotifications { enabled } => {
            ApplicationCommand::SetNotifications { enabled }
        }
        BridgeCommand::SetReadReceipts { enabled } => {
            ApplicationCommand::SetReadReceipts { enabled }
        }
        BridgeCommand::AcknowledgeNewContacts => ApplicationCommand::AcknowledgeNewContacts,
        BridgeCommand::UpdateProfile { display_name, at_ms } => {
            ApplicationCommand::UpdateProfile { display_name, at_ms }
        }
        BridgeCommand::CreatePairing { session_id_hex } => {
            ApplicationCommand::CreatePairing { session_id: parse_id(&session_id_hex)? }
        }
        BridgeCommand::JoinPairing { session_id_hex, code, ticket } => {
            ApplicationCommand::JoinPairing {
                session_id: parse_id(&session_id_hex)?,
                code,
                ticket: ticket.map(|value| parse_ticket(&value)).transpose()?,
            }
        }
        BridgeCommand::ApprovePairing { session_id_hex } => {
            ApplicationCommand::ApprovePairing { session_id: parse_id(&session_id_hex)? }
        }
        BridgeCommand::RejectPairing { session_id_hex } => {
            ApplicationCommand::RejectPairing { session_id: parse_id(&session_id_hex)? }
        }
        BridgeCommand::CancelPairing { session_id_hex } => {
            ApplicationCommand::CancelPairing { session_id: parse_id(&session_id_hex)? }
        }
        BridgeCommand::RenameContact { contact_id_hex, display_name } => {
            ApplicationCommand::RenameContact {
                contact_id: parse_id(&contact_id_hex)?,
                display_name,
            }
        }
        BridgeCommand::VerifyContact { contact_id_hex } => {
            ApplicationCommand::VerifyContact { contact_id: parse_id(&contact_id_hex)? }
        }
        BridgeCommand::ResetContactVerification { contact_id_hex } => {
            ApplicationCommand::ResetContactVerification { contact_id: parse_id(&contact_id_hex)? }
        }
        BridgeCommand::BlockContact { contact_id_hex } => {
            ApplicationCommand::BlockContact { contact_id: parse_id(&contact_id_hex)? }
        }
        BridgeCommand::UnblockContact { contact_id_hex } => {
            ApplicationCommand::UnblockContact { contact_id: parse_id(&contact_id_hex)? }
        }
        BridgeCommand::RemoveContact { contact_id_hex } => {
            ApplicationCommand::RemoveContact { contact_id: parse_id(&contact_id_hex)? }
        }
        BridgeCommand::StartConversation { contact_id_hex } => {
            ApplicationCommand::StartConversation { contact_id: parse_id(&contact_id_hex)? }
        }
        BridgeCommand::ClearConversationHistory { conversation_id_hex } => {
            ApplicationCommand::ClearConversationHistory {
                conversation_id: parse_id(&conversation_id_hex)?,
            }
        }
        BridgeCommand::QueueMessage {
            message_id_hex,
            conversation_id_hex,
            body,
            reply_to_message_id_hex,
            at_ms,
        } => ApplicationCommand::QueueMessage {
            message_id: parse_id(&message_id_hex)?,
            conversation_id: parse_id(&conversation_id_hex)?,
            body,
            reply_to_message_id: reply_to_message_id_hex
                .map(|value| parse_id(&value))
                .transpose()?,
            at_ms,
        },
        BridgeCommand::RetryMessage { message_id_hex, at_ms } => {
            ApplicationCommand::RetryMessage { message_id: parse_id(&message_id_hex)?, at_ms }
        }
        BridgeCommand::MarkConversationRead { conversation_id_hex } => {
            ApplicationCommand::MarkConversationRead {
                conversation_id: parse_id(&conversation_id_hex)?,
            }
        }
        BridgeCommand::QueueAttachment {
            attachment_id_hex,
            message_id_hex,
            conversation_id_hex,
            source_path,
            name,
            media_type,
            size,
            at_ms,
        } => ApplicationCommand::QueueAttachment {
            attachment_id: parse_id(&attachment_id_hex)?,
            message_id: parse_id(&message_id_hex)?,
            conversation_id: parse_id(&conversation_id_hex)?,
            source_path,
            name,
            media_type,
            size,
            at_ms,
        },
        BridgeCommand::RetryAttachment { attachment_id_hex } => {
            ApplicationCommand::RetryAttachment { attachment_id: parse_id(&attachment_id_hex)? }
        }
        BridgeCommand::CancelAttachment { attachment_id_hex } => {
            ApplicationCommand::CancelAttachment { attachment_id: parse_id(&attachment_id_hex)? }
        }
        BridgeCommand::ExportAttachment { attachment_id_hex, destination_path } => {
            ApplicationCommand::ExportAttachment {
                attachment_id: parse_id(&attachment_id_hex)?,
                destination_path,
            }
        }
        BridgeCommand::RefreshSnapshot => ApplicationCommand::RefreshSnapshot,
    })
}

pub fn bridge_result_from_application(
    result: Result<torca_client_application::ApplicationCommandResult, ApplicationError>,
) -> BridgeResult {
    match result {
        Ok(result) => BridgeResult {
            ok: true,
            kind: result.kind.into(),
            error: None,
            error_code: None,
            resource_id: result.resource_id.map(|id| id.to_string()),
            invite_uri: result.invite_uri,
        },
        Err(error) => BridgeResult {
            ok: false,
            kind: "error".into(),
            error: Some(error.to_string()),
            error_code: Some(error.descriptor().code().to_string()),
            resource_id: None,
            invite_uri: None,
        },
    }
}

pub fn bridge_message_from_domain(message: Message) -> BridgeMessage {
    BridgeMessage {
        id: message.id().to_string(),
        conversation_id: message.conversation_id().to_string(),
        body: message.body().as_str().to_owned(),
        direction: message_direction_name(message.direction()).into(),
        status: message_status_name(message.status()).into(),
        reply_to_message_id: message.reply_to().map(|reply| reply.message_id.to_string()),
        created_at_ms: message.created_at().to_unix_millis(),
        updated_at_ms: message.updated_at().to_unix_millis(),
        sent_at_ms: message.sent_at().map(Timestamp::to_unix_millis),
        delivered_at_ms: message.delivered_at().map(Timestamp::to_unix_millis),
        read_at_ms: message.read_at().map(Timestamp::to_unix_millis),
        attempt_count: u32::try_from(message.attempts().len()).unwrap_or(u32::MAX),
    }
}

#[must_use]
pub const fn message_direction_name(value: MessageDirection) -> &'static str {
    match value {
        MessageDirection::Outbound => "outbound",
        MessageDirection::Inbound => "inbound",
    }
}

#[must_use]
pub const fn message_status_name(value: MessageStatus) -> &'static str {
    match value {
        MessageStatus::Queued => "queued",
        MessageStatus::Sending => "sending",
        MessageStatus::Sent => "sent",
        MessageStatus::Delivered => "delivered",
        MessageStatus::Read => "read",
        MessageStatus::Failed => "failed",
        MessageStatus::Cancelled => "cancelled",
    }
}

#[must_use]
pub const fn tor_state_name(value: TorState) -> &'static str {
    match value {
        TorState::Stopped => "stopped",
        TorState::Starting => "starting",
        TorState::Ready => "ready",
        TorState::Degraded => "degraded",
        TorState::Failed => "failed",
    }
}

const fn probe_status_name(value: ProbeStatus) -> &'static str {
    match value {
        ProbeStatus::Unknown => "unknown",
        ProbeStatus::Checking => "checking",
        ProbeStatus::Healthy => "healthy",
        ProbeStatus::Degraded => "degraded",
        ProbeStatus::Unreachable => "unreachable",
        ProbeStatus::Failed => "failed",
        ProbeStatus::Disabled => "disabled",
    }
}

const fn bootstrap_step_state_name(value: BootstrapStepState) -> &'static str {
    match value {
        BootstrapStepState::Pending => "pending",
        BootstrapStepState::Running => "running",
        BootstrapStepState::Verifying => "verifying",
        BootstrapStepState::Ready => "ready",
        BootstrapStepState::Degraded => "degraded",
        BootstrapStepState::Failed => "failed",
        BootstrapStepState::Blocked => "blocked",
    }
}

const fn pairing_role_name(value: PairingRole) -> &'static str {
    match value {
        PairingRole::Creator => "creator",
        PairingRole::Joiner => "joiner",
    }
}

const fn pairing_state_name(value: PairingState) -> &'static str {
    match value {
        PairingState::Open => "open",
        PairingState::PeerJoined => "peerjoined",
        PairingState::AwaitingApproval => "awaitingapproval",
        PairingState::Approved => "approved",
        PairingState::Rejected => "rejected",
        PairingState::Cancelled => "cancelled",
        PairingState::Expired => "expired",
        PairingState::Completed => "completed",
    }
}

const fn peer_connection_status_name(value: PeerConnectionStatus) -> &'static str {
    match value {
        PeerConnectionStatus::Disconnected => "disconnected",
        PeerConnectionStatus::Connecting => "connecting",
        PeerConnectionStatus::Handshaking => "handshaking",
        PeerConnectionStatus::Ready => "ready",
        PeerConnectionStatus::Reconnecting => "reconnecting",
        PeerConnectionStatus::Failed => "failed",
    }
}

const fn peer_health_quality_name(value: PeerHealthQuality) -> &'static str {
    match value {
        PeerHealthQuality::Unknown => "unknown",
        PeerHealthQuality::Excellent => "excellent",
        PeerHealthQuality::Good => "good",
        PeerHealthQuality::Fair => "fair",
        PeerHealthQuality::Poor => "poor",
    }
}

const fn contact_status_name(value: ContactStatus) -> &'static str {
    match value {
        ContactStatus::Active => "active",
        ContactStatus::Blocked => "blocked",
        ContactStatus::Removed => "removed",
    }
}

const fn conversation_status_name(value: ConversationStatus) -> &'static str {
    match value {
        ConversationStatus::Active => "active",
        ConversationStatus::Archived => "archived",
    }
}

fn parse_id(value: &str) -> Result<OpaqueId, String> {
    value.parse::<OpaqueId>().map_err(string_error)
}

fn parse_ticket(value: &str) -> Result<[u8; 16], String> {
    if value.len() != 32 {
        return Err("invalid pairing ticket".into());
    }
    let mut out = [0_u8; 16];
    for (i, chunk) in value.as_bytes().chunks_exact(2).enumerate() {
        let s = core::str::from_utf8(chunk).map_err(|_| "invalid pairing ticket")?;
        out[i] = u8::from_str_radix(s, 16).map_err(|_| "invalid pairing ticket")?;
    }
    Ok(out)
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

/// Pure wire projection: application owns the snapshot context and this
/// function only converts it to contract DTOs. No use-case or readiness policy
/// belongs here.
pub fn bridge_snapshot_from_application(context: ApplicationSnapshotContext) -> BridgeSnapshot {
    let ApplicationSnapshotContext {
        application: snapshot,
        network,
        attachments,
        bootstrap,
        identity_fingerprint,
        identity_fingerprints,
        safety_numbers,
        pending_operations,
    } = context;
    let identity_name = snapshot.identity.as_ref().and_then(|identity| {
        identity.profile().map(|profile| profile.display_name().as_str().to_owned())
    });
    // Root snapshots deliberately omit message history. Conversation page and
    // search queries are the only history transport exposed to presentation.
    let messages = Vec::new();
    let tor_state = tor_state_name(network.tor).to_owned();
    let relay_probe = network.probes.iter().find(|probe| probe.target == ProbeTarget::Relay);
    let relay_state = relay_probe
        .map(|probe| probe_status_name(probe.status).to_owned())
        .unwrap_or_else(|| "unknown".into());
    let relay_code = relay_probe
        .map(|probe| probe.diagnostic_code.clone())
        .unwrap_or_else(|| "RELAY_UNAVAILABLE".into());
    let relay_latency_ms = relay_probe.and_then(|probe| probe.latency_ms);
    let bootstrap_snapshot = bootstrap.clone();
    let bootstrap_phase = bootstrap_phase_name(bootstrap_snapshot.phase);
    BridgeSnapshot {
        contract_version: CONTRACT_VERSION,
        identity_name,
        identity_fingerprint,
        tor_state: tor_state.clone(),
        transport: BridgeTransportStatus {
            tor: BridgeTransportIndicator {
                state: tor_state.clone(),
                code: if tor_state == "ready" {
                    "TOR_READY".into()
                } else {
                    "TOR_NOT_READY".into()
                },
                latency_ms: None,
                last_activity_at_ms: network
                    .connectivity
                    .tor
                    .last_tx_at
                    .into_iter()
                    .chain(network.connectivity.tor.last_rx_at)
                    .max()
                    .map(Timestamp::to_unix_millis),
                activity_sequence: network
                    .connectivity
                    .tor
                    .tx_sequence
                    .saturating_add(network.connectivity.tor.rx_sequence),
                tx_sequence: network.connectivity.tor.tx_sequence,
                rx_sequence: network.connectivity.tor.rx_sequence,
                in_flight: network.connectivity.tor.in_flight,
                queued: network.connectivity.tor.queued,
            },
            relay: BridgeTransportIndicator {
                state: relay_state,
                code: relay_code,
                latency_ms: relay_latency_ms,
                last_activity_at_ms: network
                    .connectivity
                    .relay
                    .last_tx_at
                    .into_iter()
                    .chain(network.connectivity.relay.last_rx_at)
                    .max()
                    .map(Timestamp::to_unix_millis),
                activity_sequence: network
                    .connectivity
                    .relay
                    .tx_sequence
                    .saturating_add(network.connectivity.relay.rx_sequence),
                tx_sequence: network.connectivity.relay.tx_sequence,
                rx_sequence: network.connectivity.relay.rx_sequence,
                in_flight: network.connectivity.relay.in_flight,
                queued: network.connectivity.relay.queued,
            },
            peer: BridgeTransportIndicator {
                state: if network.connectivity.peers_total == 0 {
                    "inactive"
                } else if network.connectivity.peers_ready > 0 {
                    "ready"
                } else {
                    "disconnected"
                }
                .into(),
                code: if network.connectivity.peers_total == 0 {
                    "PEER_INACTIVE"
                } else if network.connectivity.peers_ready > 0 {
                    "PEER_READY"
                } else {
                    "PEER_DISCONNECTED"
                }
                .into(),
                latency_ms: network.connectivity.peer.latency_ms,
                last_activity_at_ms: network
                    .connectivity
                    .peer
                    .last_tx_at
                    .into_iter()
                    .chain(network.connectivity.peer.last_rx_at)
                    .max()
                    .map(Timestamp::to_unix_millis),
                activity_sequence: network
                    .connectivity
                    .peer
                    .tx_sequence
                    .saturating_add(network.connectivity.peer.rx_sequence),
                tx_sequence: network.connectivity.peer.tx_sequence,
                rx_sequence: network.connectivity.peer.rx_sequence,
                in_flight: network.connectivity.peer.in_flight,
                queued: network.connectivity.peer.queued,
            },
            peers_ready: network.connectivity.peers_ready,
            peers_total: network.connectivity.peers_total,
            relay_info: network.relay_info.map(|info| BridgeRelayInfo {
                product_version: info.product_version,
                build_id: info.build_id,
                source_commit: info.source_commit,
                protocol_version: info.protocol_version,
            }),
        },
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
                state: bootstrap_step_state_name(step.state).into(),
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
            // Pairing history is intentionally not a root-snapshot feature.
            // Terminal sessions have already been removed from the relay and
            // are projected to UI through the operation result/toast instead.
            .filter(|pairing| {
                !matches!(
                    pairing.state(),
                    PairingState::Rejected
                        | PairingState::Cancelled
                        | PairingState::Expired
                        | PairingState::Completed
                )
            })
            .map(|pairing| {
                let remote_identity = pairing.remote_proposal().map(|proposal| {
                    let identity = &proposal.public_identity;
                    (
                        identity.identity_id().to_string(),
                        identity_fingerprints
                            .get(&identity.identity_id().to_opaque())
                            .cloned()
                            .unwrap_or_default(),
                    )
                });
                BridgePairing {
                    id: pairing.id().to_string(),
                    code: pairing.code().as_str().to_owned(),
                    invite_uri: encode_invite_uri(pairing.code().as_str(), None)
                        .unwrap_or_default(),
                    role: pairing_role_name(pairing.role()).into(),
                    state: pairing_state_name(pairing.state()).into(),
                    expires_at_ms: pairing.expires_at().to_unix_millis(),
                    local_approved: pairing.local_approved(),
                    remote_approved: pairing.remote_approved(),
                    remote_identity_id: remote_identity.as_ref().map(|value| value.0.clone()),
                    remote_display_name: pairing
                        .remote_proposal()
                        .map(|proposal| proposal.display_name.clone()),
                    remote_fingerprint: remote_identity.map(|value| value.1),
                }
            })
            .collect(),
        contacts: snapshot
            .contacts
            .into_iter()
            .map(|contact| {
                let connection_state = network.peers.get(&contact.id()).map_or_else(
                    || "disconnected".to_owned(),
                    |state| peer_connection_status_name(*state).to_owned(),
                );
                let peer_health = network.peer_health.get(&contact.id()).map_or_else(
                    || BridgePeerHealth {
                        state: connection_state.clone(),
                        quality: "unknown".into(),
                        rtt_ms: None,
                        last_success_at_ms: None,
                        consecutive_failures: 0,
                        reconnect_attempt: 0,
                        last_activity_at_ms: None,
                        activity_sequence: 0,
                    },
                    |health| BridgePeerHealth {
                        state: peer_connection_status_name(health.state).into(),
                        quality: peer_health_quality_name(health.quality).into(),
                        rtt_ms: health.rtt_ms,
                        last_success_at_ms: health.last_success_at.map(Timestamp::to_unix_millis),
                        consecutive_failures: health.consecutive_failures,
                        reconnect_attempt: health.reconnect_attempt,
                        last_activity_at_ms: network
                            .peer_activity
                            .get(&contact.id())
                            .and_then(|activity| activity.last_activity_at)
                            .map(Timestamp::to_unix_millis),
                        activity_sequence: network
                            .peer_activity
                            .get(&contact.id())
                            .map_or(0, |activity| activity.sequence),
                    },
                );
                let safety_number = safety_numbers.get(&contact.id()).cloned().unwrap_or_default();
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
                    status: contact_status_name(contact.status()).into(),
                    connection_state: peer_health.state.clone(),
                    presence_state: if peer_health.state == "ready" {
                        "online".into()
                    } else if peer_health.last_success_at_ms.is_some() {
                        "offline".into()
                    } else {
                        "unknown".into()
                    },
                    last_seen_at_ms: peer_health.last_success_at_ms,
                    safety_number,
                    peer_health,
                    verification_status: if verification.verified {
                        "verified".into()
                    } else {
                        "unverified".into()
                    },
                    verified_at_ms: verification.verified_at.map(Timestamp::to_unix_millis),
                    created_at_ms: contact.created_at().to_unix_millis(),
                }
            })
            .collect(),
        conversations: snapshot
            .conversations
            .into_iter()
            .map(|conversation| BridgeConversation {
                id: conversation.id().to_string(),
                contact_id: conversation.contact_id().to_string(),
                status: conversation_status_name(conversation.status()).into(),
                unread_count: 0,
                last_activity_at_ms: 0,
                last_message_body: None,
                last_message_direction: None,
                last_message_status: None,
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
                attempt_count: attachment.attempt_count,
                updated_at_ms: attachment.updated_at_ms,
                direction: attachment.direction,
                last_error_code: attachment.last_error_code,
            })
            .collect(),
        pending_operations: pending_operations
            .into_iter()
            .map(|operation| {
                let (kind, dependency) = match operation.kind {
                    // Pairing envelopes contain the local onion endpoint and
                    // must also reach the rendezvous relay. A single "relay"
                    // label hid the Android wait for its own onion service.
                    PendingOperationKind::CreatePairing => {
                        ("pairing.create", "tor_onion_and_relay")
                    }
                    PendingOperationKind::JoinPairing { .. } => {
                        ("pairing.join", "tor_onion_and_relay")
                    }
                    PendingOperationKind::ApprovePairing => ("pairing.approve", "relay"),
                    PendingOperationKind::RejectPairing => ("pairing.reject", "relay"),
                    PendingOperationKind::CancelPairing => ("pairing.cancel", "relay"),
                    PendingOperationKind::RenameContact { .. } => ("contact.rename", "runtime"),
                    PendingOperationKind::VerifyContact => ("contact.verify", "runtime"),
                    PendingOperationKind::ResetContactVerification => {
                        ("contact.verification.reset", "runtime")
                    }
                    PendingOperationKind::BlockContact => ("contact.block", "runtime"),
                    PendingOperationKind::UnblockContact => ("contact.unblock", "runtime"),
                    PendingOperationKind::RemoveContact => ("contact.remove", "runtime"),
                    PendingOperationKind::ClearConversationHistory => {
                        ("conversation.history.clear", "runtime")
                    }
                    PendingOperationKind::MarkConversationRead => {
                        ("conversation.mark_read", "runtime")
                    }
                };
                BridgePendingOperation {
                    id: operation.id.to_string(),
                    resource_id: operation.resource_id.to_string(),
                    kind: kind.into(),
                    state: if operation.attempts == 0 { "queued" } else { "retrying" }.into(),
                    dependency: dependency.into(),
                    attempts: operation.attempts,
                    next_attempt_at_ms: operation.next_attempt_at_ms,
                    created_at_ms: operation.created_at_ms,
                    last_error: operation.last_error,
                }
            })
            .collect(),
        unread_messages_count: 0,
        new_contacts_count: 0,
        pairing_attention_count: 0,
    }
}
fn fallback_contact_name(id: ContactId) -> String {
    let value = id.to_string();
    let short = value.get(..8).unwrap_or(&value);
    format!("Contact {short}")
}
pub fn dart_contract_source() -> &'static str {
    include_str!("../schema/torca_contract.dart")
}

#[cfg(test)]
mod tests {
    use super::generated;

    #[test]
    fn generated_operation_allowlist_matches_runtime_surface() {
        assert!(generated::contains("command", "profile.set"));
        assert!(generated::contains("query", "snapshot.get"));
        assert!(generated::contains("query", "runtime.poll"));
        assert!(generated::contains("lifecycle", "foregrounded"));
        assert!(!generated::contains("command", "operation.unknown"));
        assert!(!generated::contains("query", "message.history.full"));
    }
}
