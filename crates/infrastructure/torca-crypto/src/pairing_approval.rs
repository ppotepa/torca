use sha2::{Digest, Sha256};
use torca_foundation::OpaqueId;
use torca_identity::{KeyAlgorithm, KeyId, PublicIdentity};
use torca_pairing_coordinator::{PairingApprovalError, PairingApprovalPort};
use torca_pairing_protocol::PairingEnvelope;

use crate::{
    CryptoProvider, ManagedIdentityKeys, ProtectedSecretStore, PublicKey, RustCryptoProvider,
    Signature,
};

const TRANSCRIPT_LABEL: &[u8] = b"TORCA-PAIRING-TRANSCRIPT-V1";
const APPROVAL_LABEL: &[u8] = b"TORCA-PAIRING-APPROVAL-V1";

impl<C, S> PairingApprovalPort for ManagedIdentityKeys<C, S>
where
    C: CryptoProvider,
    S: ProtectedSecretStore,
{
    fn transcript_digest(
        &self,
        creator_offer: &PairingEnvelope,
        joiner_offer: &PairingEnvelope,
    ) -> Result<[u8; 32], PairingApprovalError> {
        let creator = creator_offer
            .transcript_component()
            .map_err(|_| PairingApprovalError::InvalidTranscript)?;
        let joiner = joiner_offer
            .transcript_component()
            .map_err(|_| PairingApprovalError::InvalidTranscript)?;
        let creator_len =
            u32::try_from(creator.len()).map_err(|_| PairingApprovalError::InvalidTranscript)?;
        let joiner_len =
            u32::try_from(joiner.len()).map_err(|_| PairingApprovalError::InvalidTranscript)?;

        let mut digest = Sha256::new();
        digest.update(TRANSCRIPT_LABEL);
        digest.update(creator_len.to_be_bytes());
        digest.update(&creator);
        digest.update(joiner_len.to_be_bytes());
        digest.update(&joiner);
        Ok(digest.finalize().into())
    }

    fn sign_approval(
        &self,
        key_id: KeyId,
        context_id: OpaqueId,
        transcript_digest: [u8; 32],
    ) -> Result<Vec<u8>, PairingApprovalError> {
        let canonical = approval_bytes(context_id, transcript_digest);
        self.sign(key_id, &canonical)
            .map(|signature| signature.0.to_vec())
            .map_err(|_| PairingApprovalError::Crypto)
    }

    fn verify_approval(
        &self,
        remote_identity: &PublicIdentity,
        context_id: OpaqueId,
        transcript_digest: [u8; 32],
        proof: &[u8],
    ) -> Result<(), PairingApprovalError> {
        if remote_identity.key().algorithm() != KeyAlgorithm::Ed25519 {
            return Err(PairingApprovalError::InvalidKey);
        }
        let public: [u8; 32] = remote_identity
            .key()
            .public_key()
            .try_into()
            .map_err(|_| PairingApprovalError::InvalidKey)?;
        let signature: [u8; 64] =
            proof.try_into().map_err(|_| PairingApprovalError::InvalidProof)?;
        RustCryptoProvider
            .verify(
                &PublicKey(public),
                &approval_bytes(context_id, transcript_digest),
                &Signature(signature),
            )
            .map_err(|_| PairingApprovalError::InvalidProof)
    }
}

fn approval_bytes(context_id: OpaqueId, transcript_digest: [u8; 32]) -> Vec<u8> {
    let mut output = Vec::with_capacity(APPROVAL_LABEL.len() + 16 + 32);
    output.extend_from_slice(APPROVAL_LABEL);
    output.extend_from_slice(context_id.as_bytes());
    output.extend_from_slice(&transcript_digest);
    output
}
