use torca_foundation::OpaqueId;
use torca_identity::{KeyId, PublicIdentity};
use torca_pairing_protocol::PairingEnvelope;

/// Cryptographic operations for explicit human approval after both public offers are known.
pub trait PairingApprovalPort {
    /// Hashes the canonical creator-then-joiner offer transcript.
    fn transcript_digest(
        &self,
        creator_offer: &PairingEnvelope,
        joiner_offer: &PairingEnvelope,
    ) -> Result<[u8; 32], PairingApprovalError>;

    /// Signs one pairing/session-bound approval digest with the local identity key handle.
    fn sign_approval(
        &self,
        key_id: KeyId,
        context_id: OpaqueId,
        transcript_digest: [u8; 32],
    ) -> Result<Vec<u8>, PairingApprovalError>;

    /// Verifies a remote approval against the public identity carried by the validated offer.
    fn verify_approval(
        &self,
        remote_identity: &PublicIdentity,
        context_id: OpaqueId,
        transcript_digest: [u8; 32],
        proof: &[u8],
    ) -> Result<(), PairingApprovalError>;
}

impl PairingApprovalPort for Box<dyn PairingApprovalPort + Send> {
    fn transcript_digest(
        &self,
        creator_offer: &PairingEnvelope,
        joiner_offer: &PairingEnvelope,
    ) -> Result<[u8; 32], PairingApprovalError> {
        (**self).transcript_digest(creator_offer, joiner_offer)
    }

    fn sign_approval(
        &self,
        key_id: KeyId,
        context_id: OpaqueId,
        transcript_digest: [u8; 32],
    ) -> Result<Vec<u8>, PairingApprovalError> {
        (**self).sign_approval(key_id, context_id, transcript_digest)
    }

    fn verify_approval(
        &self,
        remote_identity: &PublicIdentity,
        context_id: OpaqueId,
        transcript_digest: [u8; 32],
        proof: &[u8],
    ) -> Result<(), PairingApprovalError> {
        (**self).verify_approval(remote_identity, context_id, transcript_digest, proof)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PairingApprovalError {
    InvalidTranscript,
    InvalidKey,
    InvalidProof,
    Crypto,
}
impl core::fmt::Display for PairingApprovalError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for PairingApprovalError {}
