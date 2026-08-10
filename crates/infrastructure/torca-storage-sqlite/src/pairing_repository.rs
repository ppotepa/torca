//! SQLCipher-backed pairing session repository.

use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};
use torca_contacts::ContactRoute;
use torca_foundation::{OpaqueId, Timestamp};
use torca_identity::{IdentityId, IdentityKey, KeyAlgorithm, KeyId, PublicIdentity};
use torca_pairing::{
    PairingCode, PairingError, PairingRepository, PairingRole, PairingSession, PairingSessionId,
    PairingState, PeerProposal,
};

use crate::{DatabaseKey, SqlCipherBackend, StorageKernel};

/// Durable pairing session store. It owns its SQLCipher connection and is used only by the
/// single client-engine actor, so all repository mutations remain single-writer operations.
pub struct SqlCipherPairingRepository {
    backend: SqlCipherBackend,
}

impl SqlCipherPairingRepository {
    /// Opens the encrypted database and ensures the baseline schema exists.
    pub fn open(path: impl AsRef<Path>, key: &DatabaseKey) -> Result<Self, PairingError> {
        let backend = SqlCipherBackend::open(&path, key).map_err(|_| PairingError::Storage)?;
        let _schema_version =
            StorageKernel::new(backend).bootstrap().map_err(|_| PairingError::Storage)?;
        // `bootstrap` consumes only the migration result, so reopen the connection after the
        // schema has been applied. This keeps the repository's connection independent from the
        // composition-time storage handles.
        let backend = SqlCipherBackend::open(&path, key).map_err(|_| PairingError::Storage)?;
        Ok(Self { backend })
    }

    fn connection(&self) -> &Connection {
        self.backend.connection()
    }
}

impl PairingRepository for SqlCipherPairingRepository {
    fn insert(&mut self, session: PairingSession) -> Result<(), PairingError> {
        let values = encode(&session)?;
        self.connection()
            .execute(
                "INSERT INTO pairing_sessions(session_id, code, role, state, expires_at_ms, local_approved, remote_approved,
                 remote_identity_id, remote_key_id, remote_key_algorithm, remote_public_key, remote_key_generation,
                 remote_onion_address, remote_capability_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                params![
                    values.id,
                    values.code,
                    values.role,
                    values.state,
                    values.expires_at,
                    values.local_approved,
                    values.remote_approved,
                    values.remote_identity_id,
                    values.remote_key_id,
                    values.remote_key_algorithm,
                    values.remote_public_key,
                    values.remote_key_generation,
                    values.remote_onion_address,
                    values.remote_capability_id,
                ],
            )
            .map_err(|error| if matches!(error, rusqlite::Error::SqliteFailure(_, _)) { PairingError::AlreadyExists } else { PairingError::Storage })?;
        Ok(())
    }

    fn get(&self, id: PairingSessionId) -> Result<Option<PairingSession>, PairingError> {
        self.connection()
            .query_row(
                "SELECT session_id, code, role, state, expires_at_ms, local_approved, remote_approved,
                 remote_identity_id, remote_key_id, remote_key_algorithm, remote_public_key, remote_key_generation,
                 remote_onion_address, remote_capability_id FROM pairing_sessions WHERE session_id = ?1",
                params![id.to_opaque().as_bytes().as_slice()],
                decode_row,
            )
            .optional()
            .map_err(|_| PairingError::Storage)
    }

    fn update(&mut self, session: PairingSession) -> Result<(), PairingError> {
        let values = encode(&session)?;
        let changed = self
            .connection()
            .execute(
                "UPDATE pairing_sessions SET code=?2, role=?3, state=?4, expires_at_ms=?5, local_approved=?6,
                 remote_approved=?7, remote_identity_id=?8, remote_key_id=?9, remote_key_algorithm=?10,
                 remote_public_key=?11, remote_key_generation=?12, remote_onion_address=?13, remote_capability_id=?14
                 WHERE session_id=?1",
                params![
                    values.id,
                    values.code,
                    values.role,
                    values.state,
                    values.expires_at,
                    values.local_approved,
                    values.remote_approved,
                    values.remote_identity_id,
                    values.remote_key_id,
                    values.remote_key_algorithm,
                    values.remote_public_key,
                    values.remote_key_generation,
                    values.remote_onion_address,
                    values.remote_capability_id,
                ],
            )
            .map_err(|_| PairingError::Storage)?;
        if changed == 0 {
            return Err(PairingError::NotFound);
        }
        Ok(())
    }

    fn list(&self) -> Result<Vec<PairingSession>, PairingError> {
        let mut statement = self
            .connection()
            .prepare("SELECT session_id, code, role, state, expires_at_ms, local_approved, remote_approved,
                      remote_identity_id, remote_key_id, remote_key_algorithm, remote_public_key, remote_key_generation,
                      remote_onion_address, remote_capability_id FROM pairing_sessions ORDER BY session_id")
            .map_err(|_| PairingError::Storage)?;
        let rows = statement.query_map([], decode_row).map_err(|_| PairingError::Storage)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|_| PairingError::Storage)
    }

    fn delete(&mut self, id: PairingSessionId) -> Result<(), PairingError> {
        let changed = self
            .connection()
            .execute(
                "DELETE FROM pairing_sessions WHERE session_id = ?1",
                params![id.to_opaque().as_bytes().as_slice()],
            )
            .map_err(|_| PairingError::Storage)?;
        if changed == 0 { Err(PairingError::NotFound) } else { Ok(()) }
    }
}

struct Encoded {
    id: Vec<u8>,
    code: String,
    role: i64,
    state: i64,
    expires_at: i64,
    local_approved: i64,
    remote_approved: i64,
    remote_identity_id: Option<Vec<u8>>,
    remote_key_id: Option<Vec<u8>>,
    remote_key_algorithm: Option<i64>,
    remote_public_key: Option<Vec<u8>>,
    remote_key_generation: Option<i64>,
    remote_onion_address: Option<String>,
    remote_capability_id: Option<Vec<u8>>,
}

fn encode(session: &PairingSession) -> Result<Encoded, PairingError> {
    let proposal = session.remote_proposal();
    let (
        remote_identity_id,
        remote_key_id,
        remote_key_algorithm,
        remote_public_key,
        remote_key_generation,
        remote_onion_address,
        remote_capability_id,
    ) = if let Some(proposal) = proposal {
        let key_algorithm = match proposal.public_identity.key().algorithm() {
            KeyAlgorithm::Ed25519 => 0,
        };
        (
            Some(proposal.public_identity.identity_id().to_opaque().into_bytes().to_vec()),
            Some(proposal.public_identity.key().key_id().to_opaque().into_bytes().to_vec()),
            Some(key_algorithm),
            Some(proposal.public_identity.key().public_key().to_vec()),
            Some(i64::from(proposal.public_identity.generation())),
            Some(proposal.route.onion_address().to_owned()),
            Some(proposal.route.capability_id().into_bytes().to_vec()),
        )
    } else {
        (None, None, None, None, None, None, None)
    };
    Ok(Encoded {
        id: session.id().to_opaque().into_bytes().to_vec(),
        code: session.code().as_str().to_owned(),
        role: role_code(session.role()),
        state: state_code(session.state()),
        expires_at: session.expires_at().to_unix_millis(),
        local_approved: i64::from(session.local_approved()),
        remote_approved: i64::from(session.remote_approved()),
        remote_identity_id,
        remote_key_id,
        remote_key_algorithm,
        remote_public_key,
        remote_key_generation,
        remote_onion_address,
        remote_capability_id,
    })
}

fn decode_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<PairingSession> {
    let id = PairingSessionId::from_opaque(OpaqueId::from_bytes(blob16(row.get(0)?)?));
    let code =
        PairingCode::new(row.get::<_, String>(1)?).map_err(|_| rusqlite::Error::InvalidQuery)?;
    let role = decode_role(row.get(2)?)?;
    let state = decode_state(row.get(3)?)?;
    let expires_at =
        Timestamp::from_unix_millis(row.get(4)?).map_err(|_| rusqlite::Error::InvalidQuery)?;
    let local_approved = row.get::<_, i64>(5)? != 0;
    let remote_approved = row.get::<_, i64>(6)? != 0;
    let remote_proposal = match (
        row.get::<_, Option<Vec<u8>>>(7)?,
        row.get::<_, Option<Vec<u8>>>(8)?,
        row.get::<_, Option<i64>>(9)?,
        row.get::<_, Option<Vec<u8>>>(10)?,
        row.get::<_, Option<i64>>(11)?,
        row.get::<_, Option<String>>(12)?,
        row.get::<_, Option<Vec<u8>>>(13)?,
    ) {
        (
            Some(identity_id),
            Some(key_id),
            Some(algorithm),
            Some(public_key),
            Some(generation),
            Some(onion),
            Some(capability),
        ) => {
            let algorithm = match algorithm {
                0 => KeyAlgorithm::Ed25519,
                _ => return Err(rusqlite::Error::InvalidQuery),
            };
            let identity = PublicIdentity::new(
                IdentityId::from_opaque(OpaqueId::from_bytes(blob16(identity_id)?)),
                IdentityKey::new(
                    KeyId::from_opaque(OpaqueId::from_bytes(blob16(key_id)?)),
                    algorithm,
                    public_key,
                )
                .map_err(|_| rusqlite::Error::InvalidQuery)?,
                u32::try_from(generation).map_err(|_| rusqlite::Error::InvalidQuery)?,
            );
            let route = ContactRoute::new(onion, OpaqueId::from_bytes(blob16(capability)?))
                .map_err(|_| rusqlite::Error::InvalidQuery)?;
            Some(PeerProposal { public_identity: identity, route })
        }
        (None, None, None, None, None, None, None) => None,
        _ => return Err(rusqlite::Error::InvalidQuery),
    };
    PairingSession::restore(
        id,
        code,
        role,
        state,
        expires_at,
        local_approved,
        remote_approved,
        remote_proposal,
    )
    .map_err(|_| rusqlite::Error::InvalidQuery)
}

fn blob16(value: Vec<u8>) -> rusqlite::Result<[u8; 16]> {
    value.try_into().map_err(|_| rusqlite::Error::InvalidQuery)
}
fn role_code(role: PairingRole) -> i64 {
    match role {
        PairingRole::Creator => 0,
        PairingRole::Joiner => 1,
    }
}
fn decode_role(value: i64) -> rusqlite::Result<PairingRole> {
    match value {
        0 => Ok(PairingRole::Creator),
        1 => Ok(PairingRole::Joiner),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}
fn state_code(state: PairingState) -> i64 {
    match state {
        PairingState::Open => 0,
        PairingState::PeerJoined => 1,
        PairingState::AwaitingApproval => 2,
        PairingState::Approved => 3,
        PairingState::Rejected => 4,
        PairingState::Cancelled => 5,
        PairingState::Expired => 6,
        PairingState::Completed => 7,
    }
}
fn decode_state(value: i64) -> rusqlite::Result<PairingState> {
    match value {
        0 => Ok(PairingState::Open),
        1 => Ok(PairingState::PeerJoined),
        2 => Ok(PairingState::AwaitingApproval),
        3 => Ok(PairingState::Approved),
        4 => Ok(PairingState::Rejected),
        5 => Ok(PairingState::Cancelled),
        6 => Ok(PairingState::Expired),
        7 => Ok(PairingState::Completed),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use torca_foundation::OpaqueId;
    use torca_identity::{IdentityKey, KeyAlgorithm};

    #[test]
    fn pairing_sessions_survive_repository_reopen() {
        let path = std::env::temp_dir().join(format!("torca-pairing-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let key = DatabaseKey::new([7_u8; 32]);
        let id = PairingSessionId::from_opaque(OpaqueId::from_u128(9));
        let code = PairingCode::new("ABC123").expect("valid code");
        let session = PairingSession::creator(
            id,
            code,
            Timestamp::from_unix_millis(10_000).expect("valid timestamp"),
        );
        {
            let mut repository = SqlCipherPairingRepository::open(&path, &key).expect("open");
            repository.insert(session.clone()).expect("insert");
        }
        let repository = SqlCipherPairingRepository::open(&path, &key).expect("reopen");
        assert_eq!(repository.get(id).expect("load"), Some(session));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn pairing_proposals_round_trip_without_private_material() {
        let path =
            std::env::temp_dir().join(format!("torca-pairing-proposal-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let key = DatabaseKey::new([8_u8; 32]);
        let proposal = PeerProposal {
            public_identity: PublicIdentity::new(
                IdentityId::from_u128(11),
                IdentityKey::new(KeyId::from_u128(12), KeyAlgorithm::Ed25519, vec![3; 32])
                    .expect("public key"),
                2,
            ),
            route: ContactRoute::new("a".repeat(56) + ".onion", OpaqueId::from_u128(13))
                .expect("route"),
        };
        let session = PairingSession::joiner(
            PairingSessionId::from_u128(14),
            PairingCode::new("JOIN42").expect("code"),
            Timestamp::from_unix_millis(20_000).expect("timestamp"),
            proposal,
        );
        {
            let mut repository = SqlCipherPairingRepository::open(&path, &key).expect("open");
            repository.insert(session.clone()).expect("insert");
        }
        let repository = SqlCipherPairingRepository::open(&path, &key).expect("reopen");
        assert_eq!(repository.get(session.id()).expect("load"), Some(session));
        let _ = std::fs::remove_file(path);
    }
}
