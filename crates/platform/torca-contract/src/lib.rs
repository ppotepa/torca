//! Stable typed boundary between Flutter and the process-owned Rust runtime.

use serde::Serialize;
use torca_client_application::{
    ApplicationCommand, ApplicationError, ApplicationSnapshotContext, BootstrapPhase,
    BootstrapStepId, BootstrapStepState, CommunicationState, PeerConnectionStatus,
    PeerHealthQuality, PendingOperationKind, ProbeStatus, ProbeTarget, RadioEventActor, RadioFloor,
    RadioState, RadioTimelineEventKind, RemoteRadioState,
};
use torca_contacts::{ContactId, ContactStatus};
use torca_conversations::ConversationStatus;
use torca_foundation::{ClassifiedError, OpaqueId, Timestamp};
use torca_messaging::{Message, MessageDirection, MessageReaction, MessageStatus};
use torca_pairing::{PairingRole, PairingState};
use torca_pairing_protocol::encode_invite_uri_with_bootstrap;
use torca_runtime_policy::{AttentionContext, AttentionSurface};

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
    SetAttention {
        surface: String,
        focused_resource_id: Option<String>,
        visible_contact_ids: Vec<String>,
        generation: u64,
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
        contact_id_hex: String,
        mode: String,
    },
    AcknowledgeNewContacts,
    StartBatteryObservation,
    StopBatteryObservation,
    ResetBatteryObservation,
    MarkIncident,
    UpdateProfile {
        display_name: String,
        avatar_envelope_json: Option<String>,
        at_ms: i64,
    },
    CreatePairing {
        session_id_hex: String,
    },
    JoinPairing {
        session_id_hex: String,
        code: String,
        ticket: Option<String>,
        bootstrap_json: Option<String>,
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
    ArchiveConversation {
        conversation_id_hex: String,
        at_ms: i64,
    },
    RestoreConversation {
        conversation_id_hex: String,
        at_ms: i64,
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
    CancelMessage {
        message_id_hex: String,
        at_ms: i64,
    },
    EditMessage {
        message_id_hex: String,
        body: String,
        at_ms: i64,
    },
    SetMessageReaction {
        message_id_hex: String,
        conversation_id_hex: String,
        actor_id_hex: String,
        emoji: String,
        active: bool,
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
        preview_source_path: Option<String>,
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
    ExportAttachmentPreview {
        attachment_id_hex: String,
        destination_path: String,
    },
    SetRadioEnabled {
        contact_id_hex: String,
        enabled: bool,
        at_ms: i64,
    },
    ConfigureRadioAudio {
        input_device_id: Option<String>,
        output_device_id: Option<String>,
    },
    BeginRadioTransmission {
        contact_id_hex: String,
    },
    EndRadioTransmission {
        contact_id_hex: String,
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
    pub identity_id: Option<String>,
    #[serde(skip)]
    pub identity_fingerprint: Option<String>,
    /// Selected provider and its generic readiness state. New presentation
    /// code must use these fields rather than the legacy Tor projection.
    pub communication_provider: String,
    pub communication_state: String,
    pub endpoint_summary: Option<String>,
    /// Legacy compatibility state. New presentation code must use
    /// `communication_state`.
    pub tor_state: String,
    pub transport: BridgeTransportStatus,
    pub onion_address: Option<String>,
    pub pairings: Vec<BridgePairing>,
    pub contacts: Vec<BridgeContact>,
    pub conversations: Vec<BridgeConversation>,
    pub messages: Vec<BridgeMessage>,
    pub reactions: Vec<BridgeReaction>,
    pub attachments: Vec<BridgeAttachment>,
    pub pending_operations: Vec<BridgePendingOperation>,
    pub radio: BridgeRadio,
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
pub struct BridgeRadio {
    pub active_contact_id: Option<String>,
    pub contacts: Vec<BridgeRadioContact>,
    pub session: Option<BridgeRadioSession>,
    pub last_transport_failure: Option<String>,
    pub last_transport_failure_contact_id: Option<String>,
    pub timeline: Vec<BridgeRadioTimelineEvent>,
    pub audio: BridgeRadioAudio,
}

#[must_use]
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeRadioAudio {
    pub input_devices: Vec<BridgeAudioDevice>,
    pub output_devices: Vec<BridgeAudioDevice>,
    pub selected_input_id: Option<String>,
    pub selected_output_id: Option<String>,
}

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeAudioDevice {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeRadioTimelineEvent {
    pub event_id: String,
    pub contact_id: String,
    pub kind: String,
    pub actor: String,
    pub occurred_at_ms: i64,
}

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeRadioContact {
    pub contact_id: String,
    pub local_enabled: bool,
    pub remote_state: String,
    pub state: String,
    pub changed_at_ms: i64,
}

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeRadioSession {
    pub contact_id: String,
    pub session_id: String,
    pub state: String,
    pub floor: String,
    pub burst_elapsed_ms: u32,
    pub max_burst_ms: u32,
    pub input_level_milli: u16,
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
    /// Opaque runtime resource targeted by an optional OS notification action.
    pub resource_id: String,
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
    pub remote_avatar_hash: Option<String>,
    pub remote_avatar_generator_version: Option<String>,
    pub remote_avatar_catalog_version: Option<String>,
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
    /// Provider-neutral aggregate. `tor` is retained until older clients are
    /// migrated to this field.
    pub communication: BridgeTransportIndicator,
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
    pub remote_identity_id: String,
    pub display_name: String,
    /// The provider selected when this relationship was paired. Its endpoint
    /// remains opaque and never crosses the UI boundary.
    pub transport_provider: String,
    pub endpoint_available: bool,
    /// Legacy Tor-only field retained for old clients. Direct providers do
    /// not expose their endpoint to the UI, so this is absent for Iroh/WebRTC.
    pub onion_address: Option<String>,
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
pub struct BridgeReaction {
    pub message_id: String,
    pub conversation_id: String,
    pub actor_id: String,
    pub emoji: String,
    pub active: bool,
    pub updated_at_ms: i64,
}

pub fn bridge_reaction_from_domain(reaction: MessageReaction) -> BridgeReaction {
    BridgeReaction {
        message_id: reaction.message_id().to_opaque().to_string(),
        conversation_id: reaction.conversation_id().to_opaque().to_string(),
        actor_id: reaction.actor_id().to_string(),
        emoji: reaction.emoji().to_owned(),
        active: reaction.active(),
        updated_at_ms: reaction.updated_at().to_unix_millis(),
    }
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
        BridgeCommand::SetAttention {
            surface,
            focused_resource_id,
            visible_contact_ids,
            generation,
        } => ApplicationCommand::SetAttention {
            context: AttentionContext {
                surface: decode_attention_surface(&surface)?,
                focused_resource: focused_resource_id.as_deref().map(parse_id).transpose()?,
                visible_contact_ids: visible_contact_ids
                    .iter()
                    .map(|value| parse_id(value))
                    .collect::<Result<Vec<_>, _>>()?,
                generation,
            },
        },
        BridgeCommand::SetNotifications { enabled } => {
            ApplicationCommand::SetNotifications { enabled }
        }
        BridgeCommand::SetReadReceipts { enabled } => {
            ApplicationCommand::SetReadReceipts { enabled }
        }
        BridgeCommand::SetBatteryPreferences {
            mode,
            background_sync,
            allow_delayed_background_delivery,
            metered_transfers,
            visual_activity,
        } => ApplicationCommand::SetBatteryPreferences {
            mode,
            background_sync,
            allow_delayed_background_delivery,
            metered_transfers,
            visual_activity,
        },
        BridgeCommand::SetContactAvailability { contact_id_hex, mode } => {
            ApplicationCommand::SetContactAvailability {
                contact_id: parse_id(&contact_id_hex)?,
                mode,
            }
        }
        BridgeCommand::AcknowledgeNewContacts => ApplicationCommand::AcknowledgeNewContacts,
        BridgeCommand::StartBatteryObservation
        | BridgeCommand::StopBatteryObservation
        | BridgeCommand::ResetBatteryObservation
        | BridgeCommand::MarkIncident => {
            return Err("diagnostics command must be handled by the native runtime".into());
        }
        BridgeCommand::UpdateProfile { display_name, avatar_envelope_json, at_ms } => {
            ApplicationCommand::UpdateProfile { display_name, avatar_envelope_json, at_ms }
        }
        BridgeCommand::CreatePairing { session_id_hex } => {
            ApplicationCommand::CreatePairing { session_id: parse_id(&session_id_hex)? }
        }
        BridgeCommand::JoinPairing { session_id_hex, code, ticket, bootstrap_json } => {
            ApplicationCommand::JoinPairing {
                session_id: parse_id(&session_id_hex)?,
                code,
                ticket: ticket.map(|value| parse_ticket(&value)).transpose()?,
                bootstrap: bootstrap_json.as_deref().map(parse_bootstrap).transpose()?,
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
        BridgeCommand::ArchiveConversation { conversation_id_hex, at_ms } => {
            ApplicationCommand::ArchiveConversation {
                conversation_id: parse_id(&conversation_id_hex)?,
                at_ms,
            }
        }
        BridgeCommand::RestoreConversation { conversation_id_hex, at_ms } => {
            ApplicationCommand::RestoreConversation {
                conversation_id: parse_id(&conversation_id_hex)?,
                at_ms,
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
        BridgeCommand::CancelMessage { message_id_hex, at_ms } => {
            ApplicationCommand::CancelMessage { message_id: parse_id(&message_id_hex)?, at_ms }
        }
        BridgeCommand::EditMessage { message_id_hex, body, at_ms } => {
            ApplicationCommand::EditMessage { message_id: parse_id(&message_id_hex)?, body, at_ms }
        }
        BridgeCommand::SetMessageReaction {
            message_id_hex,
            conversation_id_hex,
            actor_id_hex,
            emoji,
            active,
            at_ms,
        } => ApplicationCommand::SetMessageReaction {
            message_id: parse_id(&message_id_hex)?,
            conversation_id: parse_id(&conversation_id_hex)?,
            actor_id: parse_id(&actor_id_hex)?,
            emoji,
            active,
            at_ms,
        },
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
            preview_source_path,
            name,
            media_type,
            size,
            at_ms,
        } => ApplicationCommand::QueueAttachment {
            attachment_id: parse_id(&attachment_id_hex)?,
            message_id: parse_id(&message_id_hex)?,
            conversation_id: parse_id(&conversation_id_hex)?,
            source_path,
            preview_source_path,
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
        BridgeCommand::ExportAttachmentPreview { attachment_id_hex, destination_path } => {
            ApplicationCommand::ExportAttachmentPreview {
                attachment_id: parse_id(&attachment_id_hex)?,
                destination_path,
            }
        }
        BridgeCommand::SetRadioEnabled { contact_id_hex, enabled, at_ms } => {
            ApplicationCommand::SetRadioEnabled {
                contact_id: parse_id(&contact_id_hex)?,
                enabled,
                at_ms,
            }
        }
        BridgeCommand::ConfigureRadioAudio { input_device_id, output_device_id } => {
            ApplicationCommand::ConfigureRadioAudio { input_device_id, output_device_id }
        }
        BridgeCommand::BeginRadioTransmission { contact_id_hex } => {
            ApplicationCommand::BeginRadioTransmission { contact_id: parse_id(&contact_id_hex)? }
        }
        BridgeCommand::EndRadioTransmission { contact_id_hex } => {
            ApplicationCommand::EndRadioTransmission { contact_id: parse_id(&contact_id_hex)? }
        }
        BridgeCommand::RefreshSnapshot => ApplicationCommand::RefreshSnapshot,
    })
}

fn parse_bootstrap(
    value: &str,
) -> Result<torca_pairing_protocol::PairingBootstrapDescriptor, String> {
    let parsed: serde_json::Value =
        serde_json::from_str(value).map_err(|_| "invalid pairing bootstrap")?;
    let provider = parsed
        .get("provider")
        .and_then(serde_json::Value::as_str)
        .ok_or("pairing bootstrap provider missing")?;
    let hex = parsed
        .get("payloadHex")
        .and_then(serde_json::Value::as_str)
        .ok_or("pairing bootstrap payload missing")?;
    if hex.len() % 2 != 0 || hex.len() > 4096 {
        return Err("pairing bootstrap payload invalid".into());
    }
    let mut payload = Vec::with_capacity(hex.len() / 2);
    for chunk in hex.as_bytes().chunks_exact(2) {
        let high = (chunk[0] as char).to_digit(16).ok_or("pairing bootstrap payload invalid")?;
        let low = (chunk[1] as char).to_digit(16).ok_or("pairing bootstrap payload invalid")?;
        payload.push(((high << 4) | low) as u8);
    }
    torca_pairing_protocol::PairingBootstrapDescriptor::new(provider, payload)
        .map_err(|_| "pairing bootstrap descriptor invalid".into())
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
pub const fn communication_state_name(value: CommunicationState) -> &'static str {
    match value {
        CommunicationState::Stopped => "stopped",
        CommunicationState::Starting => "starting",
        CommunicationState::Ready => "ready",
        CommunicationState::Degraded => "degraded",
        CommunicationState::Failed => "failed",
    }
}

const fn commissioning_state_name(value: torca_transport_api::CommissioningState) -> &'static str {
    match value {
        torca_transport_api::CommissioningState::NotRequired => "not_required",
        torca_transport_api::CommissioningState::Pending => "starting",
        torca_transport_api::CommissioningState::Ready => "ready",
        torca_transport_api::CommissioningState::Degraded => "degraded",
        torca_transport_api::CommissioningState::Failed => "failed",
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
        PairingState::PeerJoined => "peer_joined",
        PairingState::AwaitingApproval => "awaiting_approval",
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

fn decode_attention_surface(value: &str) -> Result<AttentionSurface, String> {
    let resource = |name: &str| {
        value
            .strip_prefix(name)
            .and_then(|suffix| suffix.strip_prefix(':'))
            .ok_or_else(|| format!("invalid attention surface: {value}"))
            .and_then(parse_id)
    };
    match value {
        "background" => Ok(AttentionSurface::Background),
        "home" => Ok(AttentionSurface::Home),
        "chats" => Ok(AttentionSurface::Chats),
        "contacts" => Ok(AttentionSurface::Contacts),
        "diagnostics" => Ok(AttentionSurface::Diagnostics),
        value if value.starts_with("conversation:") => {
            Ok(AttentionSurface::Conversation(resource("conversation")?))
        }
        value if value.starts_with("connection_details:") => {
            Ok(AttentionSurface::ConnectionDetails(resource("connection_details")?))
        }
        value if value.starts_with("pairing:") => {
            Ok(AttentionSurface::Pairing(resource("pairing")?))
        }
        value if value.starts_with("radio:") => Ok(AttentionSurface::Radio(resource("radio")?)),
        _ => Err(format!("invalid attention surface: {value}")),
    }
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
        BootstrapStepId::CommunicationRuntime => "communication_runtime",
        BootstrapStepId::IncomingReachability => "incoming_reachability",
        BootstrapStepId::Rendezvous => "rendezvous",
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
        radio,
    } = context;
    let identity_name = snapshot.identity.as_ref().and_then(|identity| {
        identity.profile().map(|profile| profile.display_name().as_str().to_owned())
    });
    let identity_id =
        snapshot.identity.as_ref().map(|identity| identity.public().identity_id().to_string());
    // Root snapshots deliberately omit message history. Conversation page and
    // search queries are the only history transport exposed to presentation.
    let messages = Vec::new();
    let reactions = snapshot.reactions.into_iter().map(bridge_reaction_from_domain).collect();
    let communication_provider = network.communication.provider.wire_value().to_owned();
    let communication_state = commissioning_state_name(
        network.communication.step(torca_transport_api::CommissioningStage::LocalRuntime),
    )
    .to_owned();
    let endpoint_summary = network.communication.endpoint_summary.clone();
    let is_tor = network.communication.provider == torca_transport_api::TransportKind::Tor;
    // Compatibility projections are intentionally empty/unsupported for a
    // direct provider. Generic consumers must use communication above; this
    // prevents Iroh snapshots from masquerading as a Tor relay being degraded.
    let tor_state = if is_tor {
        communication_state_name(network.tor).to_owned()
    } else {
        "unsupported".to_owned()
    };
    let relay_probe = is_tor
        .then(|| {
            network.probes.iter().find(|probe| {
                matches!(probe.target, ProbeTarget::PairingService | ProbeTarget::Relay)
            })
        })
        .flatten();
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
        identity_id,
        identity_fingerprint,
        communication_provider,
        communication_state: communication_state.clone(),
        endpoint_summary,
        tor_state: tor_state.clone(),
        transport: BridgeTransportStatus {
            communication: BridgeTransportIndicator {
                state: communication_state.clone(),
                code: format!("COMMUNICATION_{}", communication_state.to_ascii_uppercase()),
                latency_ms: None,
                last_activity_at_ms: network
                    .connectivity
                    .communication
                    .last_tx_at
                    .into_iter()
                    .chain(network.connectivity.communication.last_rx_at)
                    .max()
                    .map(Timestamp::to_unix_millis),
                activity_sequence: network
                    .connectivity
                    .communication
                    .tx_sequence
                    .saturating_add(network.connectivity.communication.rx_sequence),
                tx_sequence: network.connectivity.communication.tx_sequence,
                rx_sequence: network.connectivity.communication.rx_sequence,
                in_flight: network.connectivity.communication.in_flight,
                queued: network.connectivity.communication.queued,
            },
            tor: BridgeTransportIndicator {
                state: tor_state.clone(),
                code: if !is_tor {
                    "TOR_UNSUPPORTED".into()
                } else if tor_state == "ready" {
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
                        | BootstrapStepId::CommunicationRuntime
                        | BootstrapStepId::IncomingReachability
                        | BootstrapStepId::Rendezvous
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
                    invite_uri: encode_invite_uri_with_bootstrap(
                        pairing.code().as_str(),
                        None,
                        network.communication.pairing_bootstrap.as_ref(),
                    )
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
                    remote_avatar_hash: pairing
                        .remote_proposal()
                        .and_then(|proposal| proposal.avatar.as_ref())
                        .map(|avatar| hex::encode(avatar.genome_hash)),
                    remote_avatar_generator_version: pairing
                        .remote_proposal()
                        .and_then(|proposal| proposal.avatar.as_ref())
                        .map(|avatar| avatar.generator_version.clone()),
                    remote_avatar_catalog_version: pairing
                        .remote_proposal()
                        .and_then(|proposal| proposal.avatar.as_ref())
                        .map(|avatar| avatar.catalog_version.clone()),
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
                    remote_identity_id: contact.remote_identity().identity_id().to_string(),
                    display_name,
                    transport_provider: network.communication.provider.wire_value().to_owned(),
                    endpoint_available: contact
                        .route()
                        .provider_endpoint(network.communication.provider.wire_value())
                        .is_some(),
                    onion_address: (network.communication.provider
                        == torca_transport_api::TransportKind::Tor)
                        .then(|| contact.route().onion_address().to_owned()),
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
        reactions,
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
                    // A pairing envelope needs the provider's incoming route
                    // and its selected rendezvous/signaling mechanism. The
                    // dependency label is intentionally provider-neutral.
                    PendingOperationKind::CreatePairing => {
                        ("pairing.create", "communication_and_rendezvous")
                    }
                    PendingOperationKind::JoinPairing { .. } => {
                        ("pairing.join", "communication_and_rendezvous")
                    }
                    PendingOperationKind::ApprovePairing => ("pairing.approve", "communication"),
                    PendingOperationKind::RejectPairing => ("pairing.reject", "communication"),
                    PendingOperationKind::CancelPairing => ("pairing.cancel", "communication"),
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
        radio: bridge_radio(radio),
        unread_messages_count: 0,
        new_contacts_count: 0,
        pairing_attention_count: 0,
    }
}

fn bridge_radio(value: Option<torca_client_application::RadioProjection>) -> BridgeRadio {
    let Some(value) = value else {
        return BridgeRadio {
            active_contact_id: None,
            contacts: Vec::new(),
            session: None,
            last_transport_failure: None,
            last_transport_failure_contact_id: None,
            timeline: Vec::new(),
            audio: BridgeRadioAudio::default(),
        };
    };
    BridgeRadio {
        active_contact_id: value.active_contact_id.map(|id| id.to_string()),
        contacts: value
            .contacts
            .into_iter()
            .map(|contact| BridgeRadioContact {
                contact_id: contact.contact_id.to_string(),
                local_enabled: contact.local_enabled,
                remote_state: remote_radio_state_name(contact.remote_state).into(),
                state: radio_state_name(contact.state).into(),
                changed_at_ms: contact.changed_at.to_unix_millis(),
            })
            .collect(),
        session: value.session.map(|session| BridgeRadioSession {
            contact_id: session.contact_id.to_string(),
            session_id: session.session_id.to_string(),
            state: radio_state_name(session.state).into(),
            floor: radio_floor_name(session.floor).into(),
            burst_elapsed_ms: session.burst_elapsed_ms,
            max_burst_ms: session.max_burst_ms,
            input_level_milli: session.input_level_milli,
        }),
        last_transport_failure: value
            .last_transport_failure
            .map(radio_transport_failure_name)
            .map(str::to_owned),
        last_transport_failure_contact_id: value
            .last_transport_failure_contact_id
            .map(|id| id.to_string()),
        timeline: value
            .timeline
            .into_iter()
            .map(|record| BridgeRadioTimelineEvent {
                event_id: record.event_id.to_string(),
                contact_id: record.contact_id.to_string(),
                kind: radio_event_kind_name(record.event.kind).into(),
                actor: radio_event_actor_name(record.event.actor).into(),
                occurred_at_ms: record.event.occurred_at.to_unix_millis(),
            })
            .collect(),
        audio: BridgeRadioAudio {
            input_devices: value
                .audio
                .input_devices
                .into_iter()
                .map(|device| BridgeAudioDevice {
                    id: device.id,
                    name: device.name,
                    is_default: device.is_default,
                })
                .collect(),
            output_devices: value
                .audio
                .output_devices
                .into_iter()
                .map(|device| BridgeAudioDevice {
                    id: device.id,
                    name: device.name,
                    is_default: device.is_default,
                })
                .collect(),
            selected_input_id: value.audio.selected_input_id,
            selected_output_id: value.audio.selected_output_id,
        },
    }
}

const fn remote_radio_state_name(value: RemoteRadioState) -> &'static str {
    match value {
        RemoteRadioState::Unknown => "unknown",
        RemoteRadioState::Disabled => "disabled",
        RemoteRadioState::Enabled => "enabled",
    }
}

const fn radio_state_name(value: RadioState) -> &'static str {
    match value {
        RadioState::Off => "off",
        RadioState::Available => "available",
        RadioState::WaitingForPeer => "waiting_for_peer",
        RadioState::Connecting => "connecting",
        RadioState::Ready => "ready",
        RadioState::RequestingFloor => "requesting_floor",
        RadioState::StartingCapture => "starting_capture",
        RadioState::Transmitting => "transmitting",
        RadioState::Receiving => "receiving",
        RadioState::Reconnecting => "reconnecting",
        RadioState::Unavailable => "unavailable",
    }
}

const fn radio_floor_name(value: RadioFloor) -> &'static str {
    match value {
        RadioFloor::None => "none",
        RadioFloor::Local => "local",
        RadioFloor::Remote => "remote",
    }
}

const fn radio_transport_failure_name(
    value: torca_radio_coordinator::RadioTransportFailure,
) -> &'static str {
    match value {
        torca_radio_coordinator::RadioTransportFailure::EndpointUnavailable => {
            "endpoint_unavailable"
        }
        torca_radio_coordinator::RadioTransportFailure::ConnectTimeout => "connect_timeout",
        torca_radio_coordinator::RadioTransportFailure::StreamReset => "stream_reset",
        torca_radio_coordinator::RadioTransportFailure::IdleTimeout => "idle_timeout",
        torca_radio_coordinator::RadioTransportFailure::NetworkChanged => "network_changed",
        torca_radio_coordinator::RadioTransportFailure::Protocol => "protocol",
        torca_radio_coordinator::RadioTransportFailure::Unknown => "unknown",
    }
}

const fn radio_event_kind_name(value: RadioTimelineEventKind) -> &'static str {
    match value {
        RadioTimelineEventKind::Enabled => "enabled",
        RadioTimelineEventKind::Disabled => "disabled",
        RadioTimelineEventKind::Ready => "ready",
        RadioTimelineEventKind::Interrupted => "interrupted",
        RadioTimelineEventKind::Restored => "restored",
    }
}

const fn radio_event_actor_name(value: RadioEventActor) -> &'static str {
    match value {
        RadioEventActor::Local => "local",
        RadioEventActor::Remote => "remote",
        RadioEventActor::System => "system",
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
    use super::{generated, pairing_state_name};
    use torca_pairing::PairingState;

    #[test]
    fn pairing_states_use_the_generated_wire_contract() {
        assert_eq!(pairing_state_name(PairingState::PeerJoined), "peer_joined");
        assert_eq!(pairing_state_name(PairingState::AwaitingApproval), "awaiting_approval");
    }

    #[test]
    fn generated_operation_allowlist_matches_runtime_surface() {
        assert!(generated::contains("command", "profile.set"));
        assert!(generated::contains("command", "diagnostics.observation.start"));
        assert!(generated::contains("query", "snapshot.get"));
        assert!(generated::contains("query", "runtime.poll"));
        assert!(generated::contains("lifecycle", "foregrounded"));
        assert!(generated::contains("lifecycle", "flutter_gateway_ready"));
        assert!(generated::contains("lifecycle", "network_validated"));
        assert!(!generated::contains("command", "operation.unknown"));
        assert!(!generated::contains("query", "message.history.full"));
    }
}
