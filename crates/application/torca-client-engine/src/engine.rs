//! Single-writer client engine coordinating identity, pairing and messaging.

use core::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender, TrySendError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use torca_contacts::{
    Contact, ContactError, ContactId, ContactRepository, InMemoryContactRepository,
    InMemoryPeerCredentialRepository, PeerCredential, PeerCredentialRepository,
};
use torca_conversations::{
    ConversationError, ConversationId, ConversationRepository, DirectConversation,
    InMemoryConversationRepository,
};
use torca_foundation::{
    ClassifiedError, ErrorCategory, ErrorCode, ErrorDescriptor, RetryAdvice, Timestamp,
};
use torca_identity::{
    CreateIdentity, DeterministicKeyProvider, Identity, IdentityId, IdentityKeyProvider,
    IdentityRepository, IdentityService, InMemoryIdentityRepository, Profile, ProfileName,
    UpdateProfile,
};
use torca_messaging::{
    InMemoryMessageRepository, Message, MessageBody, MessageId, MessageReaction, MessageRepository,
    MessageStatus, ReplyReference,
};
use torca_pairing::{
    InMemoryPairingRepository, PairingCode, PairingRepository, PairingSession, PairingSessionId,
    PeerProposal,
};
use torca_receipts::{InMemoryReceiptRepository, Receipt, ReceiptRepository};

include!("engine/model.rs");
include!("engine/legacy_error.rs");
include!("engine/relationship_repository.rs");
include!("engine/core.rs");
include!("engine/dispatch.rs");
include!("engine/runtime_port.rs");
include!("engine/actor.rs");
