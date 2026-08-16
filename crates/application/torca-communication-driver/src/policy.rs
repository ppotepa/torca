//! Communication policy and semantic application message registry.

use std::time::Duration;

use torca_control_delivery::{ControlKind, PendingControlJob, ReadCandidate};
use torca_delivery::{
    ApplicationPayload, ApplicationPayloadCodec, DeliveryReceiptKind, ReceiptPayload,
};
use torca_foundation::Timestamp;
use torca_receipts::{ReceiptId, ReceiptKind};
use torca_runtime::PeerHealthQuality;

use crate::CommunicationError;

/// Application-facing peer payload kinds. Numeric wire compatibility is
/// verified at the infrastructure boundary against `PeerApplicationKind`.
#[repr(u16)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApplicationMessageKind {
    Text = 1,
    Receipt = 2,
    Attachment = 3,
    Probe = 4,
    RadioControl = 5,
    Reaction = 6,
}
impl ApplicationMessageKind {
    pub const fn as_u16(self) -> u16 {
        self as u16
    }
}

pub const TEXT_MESSAGE_KIND: u16 = ApplicationMessageKind::Text.as_u16();
pub const RECEIPT_MESSAGE_KIND: u16 = ApplicationMessageKind::Receipt.as_u16();
pub const ATTACHMENT_MESSAGE_KIND: u16 = ApplicationMessageKind::Attachment.as_u16();
pub const PROBE_MESSAGE_KIND: u16 = ApplicationMessageKind::Probe.as_u16();
pub const RADIO_CONTROL_MESSAGE_KIND: u16 = ApplicationMessageKind::RadioControl.as_u16();
pub const REACTION_MESSAGE_KIND: u16 = ApplicationMessageKind::Reaction.as_u16();

pub fn classify_peer_health(
    rtt_ms: Option<u64>,
    consecutive_failures: u32,
    sample_age: Option<Duration>,
) -> PeerHealthQuality {
    match torca_presence::classify_health(rtt_ms, consecutive_failures, sample_age) {
        torca_presence::PresenceQuality::Unknown => PeerHealthQuality::Unknown,
        torca_presence::PresenceQuality::Excellent => PeerHealthQuality::Excellent,
        torca_presence::PresenceQuality::Good => PeerHealthQuality::Good,
        torca_presence::PresenceQuality::Fair => PeerHealthQuality::Fair,
        torca_presence::PresenceQuality::Poor => PeerHealthQuality::Poor,
    }
}

pub fn plan_read_receipts(
    candidates: &[ReadCandidate],
    at: Timestamp,
) -> Result<Vec<PendingControlJob>, CommunicationError> {
    candidates
        .iter()
        .map(|candidate| {
            let message_id = torca_messaging::MessageId::from_opaque(candidate.message_id);
            let receipt_id =
                ReceiptId::deterministic_for(message_id, ReceiptKind::Read).to_opaque();
            let payload =
                ApplicationPayloadCodec::encode(&ApplicationPayload::Receipt(ReceiptPayload {
                    receipt_id,
                    message_id: candidate.message_id,
                    contact_id: candidate.contact_id,
                    kind: DeliveryReceiptKind::Read,
                    at,
                }))
                .map_err(|_| CommunicationError::ReadState)?;
            Ok(PendingControlJob {
                job_id: receipt_id,
                contact_id: candidate.contact_id,
                message_id: Some(candidate.message_id),
                kind: ControlKind::Receipt,
                payload,
                next_attempt_at: at,
            })
        })
        .collect()
}
