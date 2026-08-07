//! Application orchestration for ephemeral pairing transport state.
//!
//! Domain approval state remains in `torca-pairing`. This crate owns only rendezvous transport,
//! transcript binding and short-lived crypto handles required while a pairing session is active.

mod approval;
mod core;
mod credential;
mod final_runtime;
mod invitation;
mod invite_uri;
mod policy;

pub use approval::{PairingApprovalError, PairingApprovalPort};
pub use core::{
    EncryptedPairingPayload, PairingCoordinator, PairingCoordinatorError, PairingCryptoHandle,
    PairingCryptoPort, PairingDerivedSecret, PairingEphemeralKey, PairingRendezvousPort,
    PairingSideToken, PairingSlotCapability, PairingSlotId,
};
pub use credential::{PairingCredentialError, PairingPeerSecretStore};
pub use final_runtime::{
    LocalPairingContext, PairingCompletedContact, PairingInvitation, PairingPollReport,
    PairingRuntime, PairingRuntimeError,
};
pub use invite_uri::{decode_invite_uri, encode_invite_uri};
pub use policy::{PAIRING_INVITATION_TTL, invitation_expires_at};
