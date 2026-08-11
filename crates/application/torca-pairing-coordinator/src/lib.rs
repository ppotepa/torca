//! Application orchestration for ephemeral pairing transport state.
//!
//! Domain approval state remains in `torca-pairing`. This crate owns only rendezvous transport,
//! transcript binding and short-lived crypto handles required while a pairing session is active.

mod approval;
mod core;
mod credential;
mod invitation;
mod invite_uri;
mod policy;
mod runtime;

pub use approval::{PairingApprovalError, PairingApprovalPort};
pub use core::{
    EncryptedPairingPayload, PairingContextId, PairingCoordinator, PairingCoordinatorError,
    PairingCryptoHandle, PairingCryptoPort, PairingDerivedSecret, PairingEphemeralKey,
    PairingPollBatch, PairingRelayDelivery, PairingRendezvousPort, PairingSideToken,
    PairingSlotCapability, PairingSlotId, PairingTransportSnapshot,
};
pub use credential::{PairingCredentialError, PairingPeerSecretStore};
pub use invite_uri::{decode_invite_uri, encode_invite_uri};
pub use policy::{PAIRING_INVITATION_TTL, invitation_expires_at};
pub use runtime::{
    LocalPairingContext, PairingCompletedContact, PairingInvitation, PairingPollReport,
    PairingRuntime, PairingRuntimeError,
};
