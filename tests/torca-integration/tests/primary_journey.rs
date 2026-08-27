use std::time::Duration;

use torca_attachments::{
    Attachment, AttachmentId, AttachmentName, AttachmentRepository, InMemoryAttachmentRepository,
    MediaType,
};
use torca_client_engine::{ClientEngine, EngineCommand, EngineResult};
use torca_contacts::ContactRoute;
use torca_contacts::{ContactId, PeerCredential};
use torca_conversations::ConversationId;
use torca_crypto::{CryptoProvider, DeterministicTestCrypto, Nonce, SealingKey, SigningSecretKey};
use torca_file_storage::{EncryptedAttachmentStore, MemoryBlobStore};
use torca_foundation::{CommandId, ErrorCode, OpaqueId, Timestamp};
use torca_identity::{IdentityId, Profile, ProfileName};
use torca_identity::{IdentityKey, KeyAlgorithm, KeyId, PublicIdentity};
use torca_messaging::{MessageBody, MessageId, RetryPolicy};
use torca_pairing::{PairingCode, PairingSessionId, PeerProposal};
use torca_receipts::{Receipt, ReceiptId, ReceiptKind};
use torca_storage_sqlite::{DurableDeliveryStore, InMemoryDurableDeliveryStore, StorageKernel};
use torca_storage_sqlite::{MemoryStorageBackend, migrations};

fn ts(ms: i64) -> Timestamp {
    Timestamp::from_unix_millis(ms).expect("test timestamp is in range")
}

fn peer() -> PeerProposal {
    let key = IdentityKey::new(KeyId::from_u128(40), KeyAlgorithm::Ed25519, vec![7_u8; 32])
        .expect("peer key is valid");
    let public_identity = PublicIdentity::new(IdentityId::from_u128(41), key, 0);
    let route = ContactRoute::for_provider_endpoint(
        OpaqueId::from_u128(42),
        "tor",
        b"peerexample.onion".to_vec(),
    )
    .expect("peer route is valid");
    PeerProposal { public_identity, display_name: "Remote device".to_owned(), route, avatar: None }
}

fn credential(contact_id: ContactId) -> PeerCredential {
    PeerCredential::new(contact_id, OpaqueId::from_u128(70), OpaqueId::from_u128(71))
        .expect("credential is valid")
}

#[test]
fn primary_journey_is_deterministic_across_bounded_components() {
    let mut engine = ClientEngine::default();
    let identity_id = IdentityId::from_u128(1);
    let profile = Profile::new(ProfileName::new("Orca").expect("profile name is valid"), None);
    assert_eq!(
        engine
            .dispatch(EngineCommand::CreateIdentity {
                identity_id,
                profile: Some(profile),
                at: ts(1)
            })
            .expect("identity command succeeds"),
        EngineResult::IdentityCreated
    );

    let pairing_id = PairingSessionId::from_u128(2);
    let _ = engine
        .dispatch(EngineCommand::StartPairing {
            session_id: pairing_id,
            code: PairingCode::new("RCA422").expect("pairing code is valid"),
            expires_at: ts(10_000),
        })
        .expect("pairing starts");
    let _ = engine
        .dispatch(EngineCommand::PeerJoined { session_id: pairing_id, proposal: peer(), at: ts(2) })
        .expect("peer joins");
    let _ = engine
        .dispatch(EngineCommand::ApprovePairing { session_id: pairing_id, at: ts(3) })
        .expect("local approval succeeds");
    let _ = engine
        .dispatch(EngineCommand::RemoteApproved { session_id: pairing_id, at: ts(4) })
        .expect("remote approval succeeds");

    let contact_id = ContactId::from_u128(3);
    let conversation_id = ConversationId::from_u128(4);
    let pairing_credential = credential(contact_id);
    let _ = engine
        .dispatch(EngineCommand::CompletePairing {
            session_id: pairing_id,
            contact_id,
            conversation_id,
            display_name: "Peer".into(),
            credential: pairing_credential,
            at: ts(5),
        })
        .expect("pairing completes");
    assert_eq!(
        engine
            .dispatch(EngineCommand::CompletePairing {
                session_id: pairing_id,
                contact_id,
                conversation_id,
                display_name: "Peer".into(),
                credential: pairing_credential,
                at: ts(5),
            })
            .expect("retrying a committed pairing is idempotent"),
        EngineResult::PairingCompleted { contact_id, conversation_id }
    );

    let message_id = MessageId::from_u128(5);
    let _ = engine
        .dispatch(EngineCommand::QueueMessage {
            message_id,
            conversation_id,
            body: MessageBody::new("hello through Torca").expect("message body is valid"),
            reply_to: None,
            at: ts(6),
        })
        .expect("message is queued");
    let _ = engine
        .dispatch(EngineCommand::BeginMessageSend { message_id, at: ts(7) })
        .expect("send begins");
    let _ = engine
        .dispatch(EngineCommand::MarkMessageSent { message_id, at: ts(8) })
        .expect("message is sent");
    let _ = engine
        .dispatch(EngineCommand::ApplyReceipt(Receipt {
            id: ReceiptId::from_u128(6),
            message_id,
            kind: ReceiptKind::Delivered,
            at: ts(9),
        }))
        .expect("delivered receipt applies");
    let _ = engine
        .dispatch(EngineCommand::ApplyReceipt(Receipt {
            id: ReceiptId::from_u128(7),
            message_id,
            kind: ReceiptKind::Read,
            at: ts(10),
        }))
        .expect("read receipt applies");

    let snapshot = engine.snapshot().expect("snapshot succeeds");
    assert_eq!(snapshot.contacts.len(), 1);
    assert_eq!(snapshot.conversations.len(), 1);
    assert_eq!(snapshot.messages.len(), 1);
    assert_eq!(snapshot.messages[0].sent_at(), Some(ts(8)));
    assert_eq!(snapshot.messages[0].delivered_at(), Some(ts(9)));
    assert_eq!(snapshot.messages[0].read_at(), Some(ts(10)));

    let mut durable = InMemoryDurableDeliveryStore::default();
    let retry_message = torca_messaging::Message::outbound(
        MessageId::from_u128(10),
        conversation_id,
        MessageBody::new("retry me").expect("message body is valid"),
        None,
        ts(20),
    );
    durable
        .queue_outbound(retry_message, CommandId::from_u128(11), ts(20))
        .expect("outbox insert succeeds");
    let claimed = durable.claim_due(ts(20), 8).expect("claim succeeds");
    assert_eq!(claimed.len(), 1);
    let policy = RetryPolicy {
        max_attempts: 4,
        base_delay: Duration::from_secs(1),
        max_delay: Duration::from_secs(8),
    };
    let next = ts(20)
        .checked_add(policy.delay_after(1).expect("first retry is allowed"))
        .expect("retry timestamp remains in range");
    durable.reschedule(MessageId::from_u128(10), 1, next).expect("reschedule succeeds");
    assert_eq!(durable.recover_stale_claims(next).expect("recovery succeeds"), 0);
    assert!(durable.record_inbound(OpaqueId::from_u128(12)).expect("dedup insert succeeds"));
    assert!(!durable.record_inbound(OpaqueId::from_u128(12)).expect("dedup replay succeeds"));

    let mut attachments = InMemoryAttachmentRepository::default();
    let attachment_id = AttachmentId::from_u128(20);
    let mut attachment = Attachment::prepare(
        attachment_id,
        message_id,
        AttachmentName::new("hello.txt").expect("attachment name is valid"),
        MediaType::new("text/plain").expect("media type is valid"),
        5,
        ts(30),
    )
    .expect("attachment prepares");
    attachment.begin_encryption(ts(31)).expect("encryption starts");
    attachment.mark_queued(ts(32)).expect("attachment queues");
    let _ = attachment.begin_transfer(ts(33)).expect("transfer starts");
    attachment.mark_failed(ts(34), ErrorCode::new("network")).expect("failure records");
    let _ = attachment.begin_transfer(ts(35)).expect("retry starts");
    attachment.mark_available(ts(36)).expect("attachment completes");
    attachments.insert(attachment.clone()).expect("attachment persists");
    assert_eq!(attachments.get(attachment_id).expect("attachment load succeeds"), Some(attachment));

    let mut encrypted = EncryptedAttachmentStore::new(
        DeterministicTestCrypto::default(),
        MemoryBlobStore::default(),
    );
    let key = SealingKey::new([9; 32]);
    let _nonce = encrypted
        .store(attachment_id, &key, b"attachment-v1", b"hello")
        .expect("encrypted attachment stores");
    assert_eq!(
        encrypted.load(attachment_id, &key, b"attachment-v1").expect("encrypted attachment loads"),
        b"hello"
    );

    let mut crypto = DeterministicTestCrypto::default();
    let (secret, public) = crypto.generate_signing_key().expect("signing key generation succeeds");
    let signature = crypto.sign(&secret, b"handshake").expect("signing succeeds");
    crypto.verify(&public, b"handshake", &signature).expect("verification succeeds");
    let _ = SigningSecretKey::new([1; 32]);
    let ciphertext =
        crypto.seal(&key, Nonce([1; 24]), b"aad", b"payload").expect("sealing succeeds");
    assert_eq!(
        crypto.open(&key, Nonce([1; 24]), b"aad", &ciphertext).expect("opening succeeds"),
        b"payload"
    );

    let backend = MemoryStorageBackend::default();
    let mut kernel = StorageKernel::new(backend);
    assert_eq!(kernel.bootstrap().expect("storage bootstrap succeeds"), migrations().len() as u32);
}
