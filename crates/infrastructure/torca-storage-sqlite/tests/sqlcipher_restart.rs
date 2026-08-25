use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use torca_client_application::{PendingOperation, PendingOperationKind, PendingOperationStore};
use torca_conversations::ConversationId;
use torca_foundation::{CommandId, Timestamp};
use torca_identity::{
    Identity, IdentityId, IdentityKey, IdentityRepository, KeyAlgorithm, KeyId, Profile,
    ProfileName, PublicIdentity,
};
use torca_messaging::{Message, MessageBody, MessageId};
use torca_storage_sqlite::{
    DatabaseKey, DurableDeliveryStore, SqlCipherDurableStore, SqlCipherPendingOperationStore,
    SqlCipherStore,
};

struct TemporaryDatabase(PathBuf);

impl TemporaryDatabase {
    fn new(label: &str) -> Self {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock").as_nanos();
        Self(std::env::temp_dir().join(format!("torca-{label}-{}-{nanos}.db", std::process::id())))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TemporaryDatabase {
    fn drop(&mut self) {
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{}", self.0.display(), suffix));
        }
    }
}

fn identity() -> Identity {
    let profile = Profile::new(ProfileName::new("Restart Orca").expect("name"), None);
    let key =
        IdentityKey::new(KeyId::from_u128(2), KeyAlgorithm::Ed25519, vec![3; 32]).expect("key");
    Identity::new(
        PublicIdentity::new(IdentityId::from_u128(1), key, 0),
        Some(profile),
        Timestamp::UNIX_EPOCH,
    )
}

#[test]
fn identity_survives_file_backed_restart() {
    let database = TemporaryDatabase::new("identity-restart");
    let key = DatabaseKey::new([0x71; 32]);
    let expected = identity();

    {
        let mut store = SqlCipherStore::open(database.path(), &key).expect("open first");
        store.insert(&expected).expect("insert");
    }

    let reopened = SqlCipherStore::open(database.path(), &key).expect("reopen");
    assert_eq!(reopened.load().expect("load"), Some(expected));
}

#[test]
fn wrong_database_key_is_rejected() {
    let database = TemporaryDatabase::new("wrong-key");
    let correct = DatabaseKey::new([0x72; 32]);
    let wrong = DatabaseKey::new([0x73; 32]);

    {
        let mut store = SqlCipherStore::open(database.path(), &correct).expect("open first");
        store.insert(&identity()).expect("insert");
    }

    assert!(SqlCipherStore::open(database.path(), &wrong).is_err());
}

#[test]
fn claimed_outbox_is_recovered_after_restart() {
    let database = TemporaryDatabase::new("outbox-restart");
    let key = DatabaseKey::new([0x74; 32]);
    let message = Message::outbound(
        MessageId::from_u128(11),
        ConversationId::from_u128(12),
        MessageBody::new("restart-safe").expect("body"),
        None,
        Timestamp::UNIX_EPOCH,
    );

    {
        let mut store = SqlCipherDurableStore::open(database.path(), &key).expect("open first");
        store
            .queue_outbound(message, CommandId::from_u128(13), Timestamp::UNIX_EPOCH)
            .expect("queue");
        assert_eq!(store.claim_due(Timestamp::UNIX_EPOCH, 1).expect("claim").len(), 1);
    }

    let mut reopened = SqlCipherDurableStore::open(database.path(), &key).expect("reopen");
    assert_eq!(reopened.recover_stale_claims(Timestamp::UNIX_EPOCH).expect("recover"), 1);
    assert_eq!(reopened.claim_due(Timestamp::UNIX_EPOCH, 1).expect("reclaim").len(), 1);
}

#[test]
fn pending_pairing_operation_survives_file_backed_restart() {
    let database = TemporaryDatabase::new("pending-pairing-restart");
    let key = DatabaseKey::new([0x75; 32]);
    let id = "00000000000000000000000000000021".parse().expect("operation id");

    {
        let mut store =
            SqlCipherPendingOperationStore::open(database.path(), &key).expect("open first");
        store
            .enqueue(PendingOperation {
                id,
                resource_id: id,
                kind: PendingOperationKind::JoinPairing {
                    code: "ABC123".into(),
                    ticket: Some([9; 16]),
                    bootstrap: None,
                },
                attempts: 0,
                next_attempt_at_ms: 10,
                created_at_ms: 10,
                last_error: None,
            })
            .expect("enqueue pending pairing");
    }

    let reopened =
        SqlCipherPendingOperationStore::open(database.path(), &key).expect("reopen pending store");
    let operations = reopened.due(10, 8).expect("load pending operations");
    assert_eq!(operations.len(), 1);
    assert_eq!(operations[0].resource_id, id);
    let PendingOperationKind::JoinPairing { ticket, .. } = &operations[0].kind else {
        panic!("expected pending join");
    };
    assert_eq!(*ticket, Some([9; 16]));
}
