use std::collections::{BTreeMap, BTreeSet};

use torca_client_engine::{EngineCommand, EngineHandle, EngineResult};
use torca_contacts::{ContactId, ContactRoute, PeerCredential};
use torca_conversations::ConversationId;
use torca_foundation::{OpaqueId, Timestamp};
use torca_identity::{IdentityKey, KeyAlgorithm, PublicIdentity};
use torca_pairing::{
    AvatarGenomeReference, PairingCode, PairingRole, PairingSession, PairingSessionId,
    PairingState, PeerProposal,
};
use torca_pairing_protocol::{
    AvatarEnvelope, PairingApproval, PairingCancellation, PairingCompletion, PairingCompletionAck,
    PairingEnvelope, PairingInviteTicket, PairingOffer, PairingPayload, PairingRejection,
};

use crate::{
    PairingApprovalError, PairingApprovalPort, PairingCoordinator, PairingCoordinatorError,
    PairingCredentialError, PairingCryptoPort, PairingPeerSecretStore, PairingRendezvousPort,
    PairingSideToken, PairingSlotCapability, PairingSlotId, PairingTransportSnapshot,
    encode_invite_uri, invitation_expires_at,
};

const PAIRING_STATE_VERSION: u8 = 2;

include!("runtime/model.rs");

impl<R, C, A, S> PairingRuntime<R, C, A, S>
where
    R: PairingRendezvousPort,
    C: PairingCryptoPort,
    A: PairingApprovalPort,
    S: PairingPeerSecretStore,
{
    include!("runtime/lifecycle_methods.rs");
    include!("runtime/poll_methods.rs");
    include!("runtime/completion_methods.rs");
}

include!("runtime/persistence.rs");
