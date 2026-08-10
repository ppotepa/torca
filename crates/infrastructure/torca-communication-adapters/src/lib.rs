use torca_communication_driver::InboundEnvelope;
use torca_communication_driver::PeerConnectionStatus;
use torca_peer_link::InboundPeerEnvelope;
use torca_peer_link::PeerConnectionState;

pub(crate) fn application_envelope(envelope: InboundPeerEnvelope) -> InboundEnvelope {
    InboundEnvelope {
        contact_id: envelope.contact_id,
        envelope_id: envelope.envelope_id,
        message_kind: envelope.message_kind,
        ciphertext: envelope.ciphertext,
    }
}

pub(crate) fn peer_envelope(envelope: &InboundEnvelope) -> InboundPeerEnvelope {
    InboundPeerEnvelope {
        contact_id: envelope.contact_id,
        envelope_id: envelope.envelope_id,
        message_kind: envelope.message_kind,
        ciphertext: envelope.ciphertext.clone(),
    }
}

pub(crate) const fn application_peer_state(state: PeerConnectionState) -> PeerConnectionStatus {
    match state {
        PeerConnectionState::Disconnected => PeerConnectionStatus::Disconnected,
        PeerConnectionState::Connecting => PeerConnectionStatus::Connecting,
        PeerConnectionState::Handshaking => PeerConnectionStatus::Handshaking,
        PeerConnectionState::Ready => PeerConnectionStatus::Ready,
        PeerConnectionState::Reconnecting => PeerConnectionStatus::Reconnecting,
        PeerConnectionState::Failed => PeerConnectionStatus::Failed,
    }
}

#[path = "base.rs"]
mod adapters;
pub use adapters::*;

mod active_relationships;
pub use active_relationships::ActiveRelationshipStore;

mod attachment_controls;
pub use attachment_controls::AttachmentControlAdapter;

mod attachment_export;
pub use attachment_export::AttachmentExportAdapter;

mod peer_health;
pub use peer_health::HealthPeerLinkAdapter;

mod privacy_read_state;
pub use privacy_read_state::PrivacyReadStateAdapter;

mod relationship_admin;
pub use relationship_admin::RelationshipAdminAdapter;

mod production;
pub use production::{
    CommunicationBuildError, ProductionCommunicationInputs, build_production_communication,
};
