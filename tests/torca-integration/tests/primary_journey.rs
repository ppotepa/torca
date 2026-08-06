//! Primary end-to-end domain journey exercised without platform transports.

use torca_attachments::{Attachment, AttachmentId, AttachmentName, MediaType};
use torca_client_engine::{ClientEngine, EngineCommand, EngineResult};
use torca_contacts::{ContactId, ContactRoute};
use torca_conversations::ConversationId;
use torca_crypto::{CryptoProvider, DeterministicTestCrypto};
use torca_file_storage::{EncryptedAttachmentStore, MemoryBlobStore};
use torca_foundation::{OpaqueId, Timestamp};
use torca_identity::{
    IdentityId, IdentityKey, KeyAlgorithm, KeyId, Profile, ProfileName, PublicIdentity,
};
use torca_messaging::{MessageBody, MessageId, MessageStatus};
use torca_pairing::{PairingCode, PairingSessionId, PeerProposal};
use torca_receipts::{Receipt, ReceiptId, ReceiptKind};

fn at(value: i64) -> Timestamp {
    Timestamp::from_unix_millis(value).expect("valid timestamp")
}

#[test]
fn identity_pairing_message_receipt_and_attachment_flow_is_coherent() {
    let mut engine = ClientEngine::default();
    let profile = Profile::new(ProfileName::new("Alice").expect("valid name"), None);

    assert_eq!(
        engine
            .dispatch(EngineCommand::CreateIdentity {
                identity_id: IdentityId::from_u128(1),
                profile,
                at: at(1),
            })
            .expect("identity"),
        EngineResult::IdentityCreated
    );

    let session_id = PairingSessionId::from_u128(2);
    assert_eq!(
        engine
            .dispatch(EngineCommand::StartPairing {
                session_id,
                code: PairingCode::new("TORCA1").expect("code"),
                expires_at: at(10_000),
            })
            .expect("start pairing"),
        EngineResult::PairingStarted
    );

    let remote_key =
        IdentityKey::new(KeyId::from_u128(3), KeyAlgorithm::Ed25519, vec![7; 32]).expect("key");
    let proposal = PeerProposal {
        public_identity: PublicIdentity::new(IdentityId::from_u128(4), remote_key, 0),
        route: ContactRoute::new("examplecontact.onion", OpaqueId::from_u128(5)).expect("route"),
    };

    assert_eq!(
        engine
            .dispatch(EngineCommand::PeerJoined {
                session_id,
                proposal,
                at: at(2),
            })
            .expect("peer joined"),
        EngineResult::PairingUpdated
    );
    assert_eq!(
        engine
            .dispatch(EngineCommand::ApprovePairing {
                session_id,
                at: at(3),
            })
            .expect("local approval"),
        EngineResult::PairingUpdated
    );
    assert_eq!(
        engine
            .dispatch(EngineCommand::RemoteApproved {
                session_id,
                at: at(4),
            })
            .expect("remote approval"),
        EngineResult::PairingUpdated
    );

    let contact_id = ContactId::from_u128(6);
    let conversation_id = ConversationId::from_u128(7);
    assert_eq!(
        engine
            .dispatch(EngineCommand::CompletePairing {
                session_id,
                contact_id,
                conversation_id,
                at: at(5),
            })
            .expect("complete"),
        EngineResult::PairingCompleted {
            contact_id,
            conversation_id,
        }
    );

    let message_id = MessageId::from_u128(8);
    assert_eq!(
        engine
            .dispatch(EngineCommand::QueueMessage {
                message_id,
                conversation_id,
                body: MessageBody::new("hello").expect("body"),
                reply_to: None,
                at: at(6),
            })
            .expect("queue"),
        EngineResult::MessageQueued { message_id }
    );
    assert_eq!(
        engine
            .dispatch(EngineCommand::BeginMessageSend {
                message_id,
                at: at(7),
            })
            .expect("begin send"),
        EngineResult::MessageUpdated { message_id }
    );
    assert_eq!(
        engine
            .dispatch(EngineCommand::MarkMessageSent {
                message_id,
                at: at(8),
            })
            .expect("sent"),
        EngineResult::MessageUpdated { message_id }
    );
    assert_eq!(
        engine
            .dispatch(EngineCommand::ApplyReceipt(Receipt {
                id: ReceiptId::from_u128(9),
                message_id,
                kind: ReceiptKind::Read,
                at: at(9),
            }))
            .expect("read receipt"),
        EngineResult::ReceiptApplied {
            message_id,
            changed: true,
        }
    );

    let snapshot = engine.snapshot().expect("snapshot");
    assert_eq!(snapshot.contacts.len(), 1);
    assert_eq!(snapshot.conversations.len(), 1);
    assert_eq!(snapshot.messages[0].status(), MessageStatus::Read);

    let mut attachment = Attachment::prepare(
        AttachmentId::from_u128(10),
        message_id,
        AttachmentName::new("photo.png").expect("name"),
        MediaType::new("image/png").expect("media type"),
        5,
        at(10),
    )
    .expect("attachment");
    attachment.begin_encryption(at(11)).expect("encrypting");

    let mut crypto = DeterministicTestCrypto::default();
    let sealing = crypto.generate_sealing_key().expect("test sealing key");
    let mut storage = EncryptedAttachmentStore::new(crypto, MemoryBlobStore::default());
    let _nonce = storage
        .store(attachment.id(), &sealing, b"attachment-v1", b"image")
        .expect("store");
    assert_eq!(
        storage
            .load(attachment.id(), &sealing, b"attachment-v1")
            .expect("load"),
        b"image"
    );

    attachment.mark_queued(at(12)).expect("queued");
    let attempt = attachment.begin_transfer(at(13)).expect("transfer");
    assert_eq!(attempt, 1);
    attachment.mark_available(at(14)).expect("available");
}
