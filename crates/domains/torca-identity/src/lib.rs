//! Local installation identity domain for Torca.

mod error;
mod memory;
mod model;
mod ports;
mod service;

pub use error::{IdentityError, IdentityKeyProviderError, IdentityRepositoryError, ProfileError};
pub use memory::{DeterministicKeyProvider, InMemoryIdentityRepository};
pub use model::{
    AvatarReference, CreateIdentity, GeneratedSigningKey, Identity, IdentityCreated, IdentityId,
    IdentityKey, IdentityKeyRotated, KeyAlgorithm, KeyId, Profile, ProfileName, ProfileUpdated,
    PublicIdentity, RotateIdentity, UpdateProfile,
};
pub use ports::{IdentityKeyProvider, IdentityRepository};
pub use service::IdentityService;

/// Computes the stable redacted fingerprint for a public key.
pub fn fingerprint_for(public_key: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    hash.update(b"TORCA-FINGERPRINT-V1");
    hash.update(public_key);
    grouped_hex(&hash.finalize())
}

/// Computes the stable pairwise Safety Number representation.
pub fn safety_number(local: &PublicIdentity, remote: &PublicIdentity) -> String {
    use sha2::{Digest, Sha256};
    let (first, second) = if local.identity_id().to_opaque() <= remote.identity_id().to_opaque() {
        (local, remote)
    } else {
        (remote, local)
    };
    let mut hash = Sha256::new();
    hash.update(b"TORCA-SAFETY-NUMBER-V1");
    update_identity_hash(&mut hash, first);
    update_identity_hash(&mut hash, second);
    grouped_hex(&hash.finalize())
}

fn update_identity_hash(hash: &mut impl sha2::Digest, identity: &PublicIdentity) {
    hash.update(identity.identity_id().to_opaque().as_bytes());
    let key = identity.key().public_key();
    hash.update(u32::try_from(key.len()).unwrap_or(u32::MAX).to_be_bytes());
    hash.update(key);
}

fn grouped_hex(bytes: &[u8]) -> String {
    bytes
        .chunks(4)
        .map(|chunk| {
            let mut value = String::with_capacity(chunk.len() * 2);
            for byte in chunk {
                let _ = core::fmt::Write::write_fmt(&mut value, format_args!("{byte:02X}"));
            }
            value
        })
        .collect::<Vec<_>>()
        .join(" ")
}
