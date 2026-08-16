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

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EngineCommand {
    CreateIdentity {
        identity_id: IdentityId,
        profile: Option<Profile>,
        at: Timestamp,
    },
    UpdateProfile {
        display_name: ProfileName,
        at: Timestamp,
    },
    /// Stores the immutable, content-addressed avatar genome for this device.
    SetAvatarGenome {
        record: AvatarGenomeRecord,
        at: Timestamp,
    },
    StartPairing {
        session_id: PairingSessionId,
        code: PairingCode,
        expires_at: Timestamp,
    },
    JoinPairing {
        session_id: PairingSessionId,
        code: PairingCode,
        expires_at: Timestamp,
    },
    PeerJoined {
        session_id: PairingSessionId,
        proposal: PeerProposal,
        at: Timestamp,
    },
    ApprovePairing {
        session_id: PairingSessionId,
        at: Timestamp,
    },
    RejectPairing {
        session_id: PairingSessionId,
    },
    CancelPairing {
        session_id: PairingSessionId,
    },
    ExpirePairing {
        session_id: PairingSessionId,
        at: Timestamp,
    },
    RemoteApproved {
        session_id: PairingSessionId,
        at: Timestamp,
    },
    CompletePairing {
        session_id: PairingSessionId,
        contact_id: ContactId,
        conversation_id: ConversationId,
        display_name: String,
        credential: PeerCredential,
        at: Timestamp,
    },
    EnsureConversation {
        contact_id: ContactId,
        conversation_id: ConversationId,
        at: Timestamp,
    },
    ArchiveConversation {
        conversation_id: ConversationId,
        at: Timestamp,
    },
    RestoreConversation {
        conversation_id: ConversationId,
        at: Timestamp,
    },
    RemoveContact {
        contact_id: ContactId,
    },
    RemovePairing {
        session_id: PairingSessionId,
    },
    QueueMessage {
        message_id: MessageId,
        conversation_id: ConversationId,
        body: MessageBody,
        reply_to: Option<ReplyReference>,
        at: Timestamp,
    },
    /// Cancels a locally queued message before delivery can claim it.
    CancelMessage {
        message_id: MessageId,
        at: Timestamp,
    },
    EditMessage {
        message_id: MessageId,
        body: MessageBody,
        at: Timestamp,
    },
    SetMessageReaction {
        reaction: MessageReaction,
    },
    BeginMessageSend {
        message_id: MessageId,
        at: Timestamp,
    },
    MarkMessageSent {
        message_id: MessageId,
        at: Timestamp,
    },
    MarkMessageFailed {
        message_id: MessageId,
        at: Timestamp,
        error_code: ErrorCode,
    },
    RetryMessage {
        message_id: MessageId,
        at: Timestamp,
    },
    ApplyReceipt(Receipt),
}

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EngineResult {
    IdentityCreated,
    ProfileUpdated,
    PairingStarted,
    PairingJoined,
    PairingUpdated,
    PairingRejected,
    PairingCancelled,
    PairingCompleted { contact_id: ContactId, conversation_id: ConversationId },
    ConversationStarted { conversation_id: ConversationId },
    ConversationUpdated { conversation_id: ConversationId },
    ContactRemoved { contact_id: ContactId },
    PairingRemoved,
    MessageQueued { message_id: MessageId },
    MessageUpdated { message_id: MessageId },
    ReactionUpdated { message_id: MessageId },
    ReceiptApplied { message_id: MessageId, changed: bool },
}

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientSnapshot {
    pub identity: Option<Identity>,
    pub pairings: Vec<PairingSession>,
    pub contacts: Vec<Contact>,
    pub conversations: Vec<DirectConversation>,
    pub messages: Vec<Message>,
    pub reactions: Vec<MessageReaction>,
    /// Content-addressed local avatar genome; never contains rendered pixels.
    pub avatar_genome: Option<AvatarGenomeRecord>,
}

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AvatarGenomeRecord {
    pub genome_hash: [u8; 32],
    pub schema_version: u8,
    pub generator_version: String,
    pub catalog_version: String,
    pub compressed_genome: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EngineError(pub String);
impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for EngineError {}

impl ClassifiedError for EngineError {
    fn descriptor(&self) -> ErrorDescriptor {
        ErrorDescriptor::new(
            ErrorCode::new("application.engine_failed"),
            ErrorCategory::Internal,
            RetryAdvice::Never,
        )
    }
}

pub trait RelationshipRepository:
    ContactRepository + ConversationRepository + PeerCredentialRepository
{
    fn upsert_avatar_genome(
        &mut self,
        record: AvatarGenomeRecord,
        at: Timestamp,
    ) -> Result<(), EngineError>;
    fn avatar_genome(&self, hash: [u8; 32]) -> Result<Option<AvatarGenomeRecord>, EngineError>;
    fn avatar_genome_for_identity(
        &self,
        _identity_id: IdentityId,
    ) -> Result<Option<AvatarGenomeRecord>, EngineError> {
        Ok(None)
    }
    fn local_avatar_genome(&self) -> Result<Option<AvatarGenomeRecord>, EngineError> {
        Ok(None)
    }
    fn insert_pairing_result(
        &mut self,
        contact: Contact,
        conversation: DirectConversation,
        display_name: &str,
        credential: PeerCredential,
        avatar: Option<AvatarGenomeRecord>,
        at: Timestamp,
    ) -> Result<(), EngineError>;
    /// Clears one complete local relationship from this repository.
    fn remove_relationship(&mut self, contact_id: ContactId) -> Result<(), EngineError>;
}

#[derive(Clone, Debug, Default)]
pub struct InMemoryRelationshipRepository {
    contacts: InMemoryContactRepository,
    conversations: InMemoryConversationRepository,
    credentials: InMemoryPeerCredentialRepository,
    avatar_genomes: Vec<AvatarGenomeRecord>,
    local_avatar_hash: Option<[u8; 32]>,
    identity_avatars: Vec<(IdentityId, [u8; 32])>,
}

impl ContactRepository for InMemoryRelationshipRepository {
    fn insert(&mut self, contact: Contact) -> Result<(), ContactError> {
        self.contacts.insert(contact)
    }
    fn get(&self, id: ContactId) -> Result<Option<Contact>, ContactError> {
        self.contacts.get(id)
    }
    fn update(&mut self, contact: Contact) -> Result<(), ContactError> {
        self.contacts.update(contact)
    }
    fn list(&self) -> Result<Vec<Contact>, ContactError> {
        self.contacts.list()
    }
}
impl ConversationRepository for InMemoryRelationshipRepository {
    fn insert(&mut self, conversation: DirectConversation) -> Result<(), ConversationError> {
        self.conversations.insert(conversation)
    }
    fn get(&self, id: ConversationId) -> Result<Option<DirectConversation>, ConversationError> {
        self.conversations.get(id)
    }
    fn for_contact(
        &self,
        contact_id: ContactId,
    ) -> Result<Option<DirectConversation>, ConversationError> {
        self.conversations.for_contact(contact_id)
    }
    fn list(&self) -> Result<Vec<DirectConversation>, ConversationError> {
        self.conversations.list()
    }
    fn update(&mut self, conversation: DirectConversation) -> Result<(), ConversationError> {
        self.conversations.update(conversation)
    }
}
impl PeerCredentialRepository for InMemoryRelationshipRepository {
    fn insert_credential(&mut self, credential: PeerCredential) -> Result<(), ContactError> {
        self.credentials.insert_credential(credential)
    }
    fn credential_for_contact(
        &self,
        contact_id: ContactId,
    ) -> Result<Option<PeerCredential>, ContactError> {
        self.credentials.credential_for_contact(contact_id)
    }
}
impl RelationshipRepository for InMemoryRelationshipRepository {
    fn upsert_avatar_genome(
        &mut self,
        record: AvatarGenomeRecord,
        _at: Timestamp,
    ) -> Result<(), EngineError> {
        self.local_avatar_hash = Some(record.genome_hash);
        if let Some(existing) = self
            .avatar_genomes
            .iter_mut()
            .find(|existing| existing.genome_hash == record.genome_hash)
        {
            *existing = record;
        } else {
            self.avatar_genomes.push(record);
        }
        Ok(())
    }

    fn avatar_genome(&self, hash: [u8; 32]) -> Result<Option<AvatarGenomeRecord>, EngineError> {
        Ok(self.avatar_genomes.iter().find(|record| record.genome_hash == hash).cloned())
    }

    fn avatar_genome_for_identity(
        &self,
        identity_id: IdentityId,
    ) -> Result<Option<AvatarGenomeRecord>, EngineError> {
        let Some((_, hash)) =
            self.identity_avatars.iter().find(|(candidate, _)| *candidate == identity_id)
        else {
            return Ok(None);
        };
        self.avatar_genome(*hash)
    }

    fn local_avatar_genome(&self) -> Result<Option<AvatarGenomeRecord>, EngineError> {
        self.local_avatar_hash.map_or(Ok(None), |hash| self.avatar_genome(hash))
    }

    fn insert_pairing_result(
        &mut self,
        contact: Contact,
        conversation: DirectConversation,
        _display_name: &str,
        credential: PeerCredential,
        avatar: Option<AvatarGenomeRecord>,
        _at: Timestamp,
    ) -> Result<(), EngineError> {
        if contact.id() != conversation.contact_id() || contact.id() != credential.contact_id() {
            return Err(EngineError("pairing relationship identifiers do not match".into()));
        }
        if ContactRepository::get(self, contact.id()).map_err(map_error)?.is_some()
            || ConversationRepository::get(self, conversation.id()).map_err(map_error)?.is_some()
            || ConversationRepository::for_contact(self, contact.id()).map_err(map_error)?.is_some()
            || PeerCredentialRepository::credential_for_contact(self, contact.id())
                .map_err(map_error)?
                .is_some()
        {
            return Err(EngineError("contact, conversation or credential already exists".into()));
        }
        let mut contacts = self.contacts.clone();
        let mut conversations = self.conversations.clone();
        let mut credentials = self.credentials.clone();
        let remote_identity_id = contact.remote_identity().identity_id();
        contacts.insert(contact).map_err(map_error)?;
        conversations.insert(conversation).map_err(map_error)?;
        credentials.insert_credential(credential).map_err(map_error)?;
        self.contacts = contacts;
        self.conversations = conversations;
        self.credentials = credentials;
        if let Some(avatar) = avatar {
            let hash = avatar.genome_hash;
            if let Some(existing) =
                self.avatar_genomes.iter_mut().find(|existing| existing.genome_hash == hash)
            {
                *existing = avatar;
            } else {
                self.avatar_genomes.push(avatar);
            }
            self.identity_avatars.retain(|(identity, _)| *identity != remote_identity_id);
            self.identity_avatars.push((remote_identity_id, hash));
        }
        Ok(())
    }

    fn remove_relationship(&mut self, contact_id: ContactId) -> Result<(), EngineError> {
        if ContactRepository::get(self, contact_id).map_err(map_error)?.is_none() {
            return Err(EngineError("contact not found".into()));
        }
        let mut contacts = self.contacts.clone();
        let mut conversations = self.conversations.clone();
        let mut credentials = self.credentials.clone();
        contacts.remove(contact_id);
        conversations.remove_for_contact(contact_id);
        credentials.remove_credential(contact_id);
        self.contacts = contacts;
        self.conversations = conversations;
        self.credentials = credentials;
        Ok(())
    }
}

pub struct ClientEngine<
    I = InMemoryIdentityRepository,
    K = DeterministicKeyProvider,
    P = InMemoryPairingRepository,
    L = InMemoryRelationshipRepository,
    M = InMemoryMessageRepository,
    R = InMemoryReceiptRepository,
> {
    identity: IdentityService<I, K>,
    pairings: P,
    relationships: L,
    messages: M,
    receipts: R,
}

impl Default
    for ClientEngine<
        InMemoryIdentityRepository,
        DeterministicKeyProvider,
        InMemoryPairingRepository,
        InMemoryRelationshipRepository,
        InMemoryMessageRepository,
        InMemoryReceiptRepository,
    >
{
    fn default() -> Self {
        Self::new(
            InMemoryIdentityRepository::default(),
            DeterministicKeyProvider::default(),
            InMemoryPairingRepository::default(),
            InMemoryRelationshipRepository::default(),
            InMemoryMessageRepository::default(),
            InMemoryReceiptRepository::default(),
        )
    }
}

impl<I, K, P, L, M, R> ClientEngine<I, K, P, L, M, R>
where
    I: IdentityRepository,
    K: IdentityKeyProvider,
    P: PairingRepository,
    L: RelationshipRepository,
    M: MessageRepository,
    R: ReceiptRepository,
{
    pub const fn new(
        identity_repository: I,
        key_provider: K,
        pairings: P,
        relationships: L,
        messages: M,
        receipts: R,
    ) -> Self {
        Self {
            identity: IdentityService::new(identity_repository, key_provider),
            pairings,
            relationships,
            messages,
            receipts,
        }
    }

    #[allow(clippy::too_many_lines)]
    pub fn dispatch(&mut self, command: EngineCommand) -> Result<EngineResult, EngineError> {
        match command {
            EngineCommand::CreateIdentity { identity_id, profile, at } => {
                let (_identity, _event) = self
                    .identity
                    .create(CreateIdentity { identity_id, profile, at })
                    .map_err(map_error)?;
                Ok(EngineResult::IdentityCreated)
            }
            EngineCommand::UpdateProfile { display_name, at } => {
                let profile = Profile::new(display_name, None);
                let (_identity, _event) = self
                    .identity
                    .update_profile(UpdateProfile { profile, at })
                    .map_err(map_error)?;
                Ok(EngineResult::ProfileUpdated)
            }
            EngineCommand::SetAvatarGenome { record, at } => {
                self.relationships.upsert_avatar_genome(record, at)?;
                Ok(EngineResult::ProfileUpdated)
            }
            EngineCommand::StartPairing { session_id, code, expires_at } => {
                self.pairings
                    .insert(PairingSession::creator(session_id, code, expires_at))
                    .map_err(map_error)?;
                Ok(EngineResult::PairingStarted)
            }
            EngineCommand::JoinPairing { session_id, code, expires_at } => {
                self.pairings
                    .insert(PairingSession::joining(session_id, code, expires_at))
                    .map_err(map_error)?;
                Ok(EngineResult::PairingJoined)
            }
            EngineCommand::PeerJoined { session_id, proposal, at } => {
                let mut session = self.load_pairing(session_id)?;
                session.peer_joined(proposal, at).map_err(map_error)?;
                self.pairings.update(session).map_err(map_error)?;
                Ok(EngineResult::PairingUpdated)
            }
            EngineCommand::ApprovePairing { session_id, at } => {
                let mut session = self.load_pairing(session_id)?;
                session.approve_local(at).map_err(map_error)?;
                self.pairings.update(session).map_err(map_error)?;
                Ok(EngineResult::PairingUpdated)
            }
            EngineCommand::RejectPairing { session_id } => {
                let mut session = self.load_pairing(session_id)?;
                session.reject().map_err(map_error)?;
                self.pairings.update(session).map_err(map_error)?;
                Ok(EngineResult::PairingRejected)
            }
            EngineCommand::CancelPairing { session_id } => {
                let mut session = self.load_pairing(session_id)?;
                session.cancel().map_err(map_error)?;
                self.pairings.update(session).map_err(map_error)?;
                Ok(EngineResult::PairingCancelled)
            }
            EngineCommand::ExpirePairing { session_id, at } => {
                let mut session = self.load_pairing(session_id)?;
                if !session.expire(at) {
                    return Err(EngineError("pairing session is not due to expire".into()));
                }
                self.pairings.update(session).map_err(map_error)?;
                Ok(EngineResult::PairingUpdated)
            }
            EngineCommand::RemoteApproved { session_id, at } => {
                let mut session = self.load_pairing(session_id)?;
                session.approve_remote(at).map_err(map_error)?;
                self.pairings.update(session).map_err(map_error)?;
                Ok(EngineResult::PairingUpdated)
            }
            EngineCommand::CompletePairing {
                session_id,
                contact_id,
                conversation_id,
                display_name,
                credential,
                at,
            } => {
                if credential.contact_id() != contact_id {
                    return Err(EngineError(
                        "peer credential contact does not match pairing".into(),
                    ));
                }
                let existing_contact =
                    ContactRepository::get(&self.relationships, contact_id).map_err(map_error)?;
                let existing_conversation =
                    ConversationRepository::get(&self.relationships, conversation_id)
                        .map_err(map_error)?;
                let existing_for_contact =
                    ConversationRepository::for_contact(&self.relationships, contact_id)
                        .map_err(map_error)?;
                let existing_credential = PeerCredentialRepository::credential_for_contact(
                    &self.relationships,
                    contact_id,
                )
                .map_err(map_error)?;
                if existing_contact.is_some()
                    && existing_conversation
                        .as_ref()
                        .is_some_and(|conversation| conversation.contact_id() == contact_id)
                    && existing_for_contact.is_some()
                    && existing_credential.is_some()
                {
                    return Ok(EngineResult::PairingCompleted { contact_id, conversation_id });
                }
                if existing_contact.is_some()
                    || existing_conversation.is_some()
                    || existing_for_contact.is_some()
                    || existing_credential.is_some()
                {
                    return Err(EngineError(
                        "contact, conversation or peer credential already exists".into(),
                    ));
                }
                let mut session = self.load_pairing(session_id)?;
                let proposal = session.complete(at).map_err(map_error)?;
                let avatar = proposal.avatar.as_ref().map(|avatar| AvatarGenomeRecord {
                    genome_hash: avatar.genome_hash,
                    schema_version: avatar.schema_version,
                    generator_version: avatar.generator_version.clone(),
                    catalog_version: avatar.catalog_version.clone(),
                    compressed_genome: avatar.compressed_genome.clone(),
                });
                let contact =
                    Contact::new(contact_id, proposal.public_identity, proposal.route, at);
                let conversation = DirectConversation::new(conversation_id, contact_id, at);
                self.relationships.insert_pairing_result(
                    contact,
                    conversation,
                    &display_name,
                    credential,
                    avatar,
                    at,
                )?;
                let _ = self.pairings.update(session);
                Ok(EngineResult::PairingCompleted { contact_id, conversation_id })
            }
            EngineCommand::RemovePairing { session_id } => {
                self.pairings.delete(session_id).map_err(map_error)?;
                Ok(EngineResult::PairingRemoved)
            }
            EngineCommand::EnsureConversation { contact_id, conversation_id, at } => {
                if ContactRepository::get(&self.relationships, contact_id)
                    .map_err(map_error)?
                    .is_none()
                {
                    return Err(EngineError("contact not found".into()));
                }
                if let Some(existing) =
                    ConversationRepository::for_contact(&self.relationships, contact_id)
                        .map_err(map_error)?
                {
                    return Ok(EngineResult::ConversationStarted {
                        conversation_id: existing.id(),
                    });
                }
                if ConversationRepository::get(&self.relationships, conversation_id)
                    .map_err(map_error)?
                    .is_some()
                {
                    return Err(EngineError("conversation id already exists".into()));
                }
                ConversationRepository::insert(
                    &mut self.relationships,
                    DirectConversation::new(conversation_id, contact_id, at),
                )
                .map_err(map_error)?;
                Ok(EngineResult::ConversationStarted { conversation_id })
            }
            EngineCommand::RemoveContact { contact_id } => {
                self.relationships.remove_relationship(contact_id)?;
                Ok(EngineResult::ContactRemoved { contact_id })
            }
            EngineCommand::ArchiveConversation { conversation_id, at } => {
                let mut conversation =
                    ConversationRepository::get(&self.relationships, conversation_id)
                        .map_err(map_error)?
                        .ok_or_else(|| EngineError("conversation not found".into()))?;
                conversation.archive(at).map_err(map_error)?;
                ConversationRepository::update(&mut self.relationships, conversation)
                    .map_err(map_error)?;
                Ok(EngineResult::ConversationUpdated { conversation_id })
            }
            EngineCommand::RestoreConversation { conversation_id, at } => {
                let mut conversation =
                    ConversationRepository::get(&self.relationships, conversation_id)
                        .map_err(map_error)?
                        .ok_or_else(|| EngineError("conversation not found".into()))?;
                conversation.restore(at).map_err(map_error)?;
                ConversationRepository::update(&mut self.relationships, conversation)
                    .map_err(map_error)?;
                Ok(EngineResult::ConversationUpdated { conversation_id })
            }
            EngineCommand::QueueMessage { message_id, conversation_id, body, reply_to, at } => {
                if ConversationRepository::get(&self.relationships, conversation_id)
                    .map_err(map_error)?
                    .is_none()
                {
                    return Err(EngineError("conversation not found".into()));
                }
                self.messages
                    .insert(Message::outbound(message_id, conversation_id, body, reply_to, at))
                    .map_err(map_error)?;
                Ok(EngineResult::MessageQueued { message_id })
            }
            EngineCommand::CancelMessage { message_id, at } => {
                let mut message = self
                    .messages
                    .get(message_id)
                    .map_err(map_error)?
                    .ok_or_else(|| EngineError("message not found".into()))?;
                message.cancel(at).map_err(map_error)?;
                self.messages.update(message).map_err(map_error)?;
                Ok(EngineResult::MessageUpdated { message_id })
            }
            EngineCommand::EditMessage { message_id, body, at } => {
                let mut message = self
                    .messages
                    .get(message_id)
                    .map_err(map_error)?
                    .ok_or_else(|| EngineError("message not found".into()))?;
                message.edit(body, at).map_err(map_error)?;
                self.messages.update(message).map_err(map_error)?;
                Ok(EngineResult::MessageUpdated { message_id })
            }
            EngineCommand::SetMessageReaction { reaction } => {
                let message_id = reaction.message_id();
                if self.messages.get(message_id).map_err(map_error)?.is_none() {
                    return Err(EngineError("message not found".into()));
                }
                self.messages.upsert_reaction(reaction).map_err(map_error)?;
                Ok(EngineResult::ReactionUpdated { message_id })
            }
            EngineCommand::BeginMessageSend { message_id, at } => {
                let mut message = self.load_message(message_id)?;
                let _ = message.begin_send(at).map_err(map_error)?;
                self.messages.update(message).map_err(map_error)?;
                Ok(EngineResult::MessageUpdated { message_id })
            }
            EngineCommand::MarkMessageSent { message_id, at } => {
                let mut message = self.load_message(message_id)?;
                message.mark_sent(at).map_err(map_error)?;
                self.messages.update(message).map_err(map_error)?;
                Ok(EngineResult::MessageUpdated { message_id })
            }
            EngineCommand::MarkMessageFailed { message_id, at, error_code } => {
                let mut message = self.load_message(message_id)?;
                message.mark_failed(at, error_code).map_err(map_error)?;
                self.messages.update(message).map_err(map_error)?;
                Ok(EngineResult::MessageUpdated { message_id })
            }
            EngineCommand::RetryMessage { message_id, at } => {
                let mut message = self.load_message(message_id)?;
                message.retry(at).map_err(map_error)?;
                self.messages.update(message).map_err(map_error)?;
                Ok(EngineResult::MessageUpdated { message_id })
            }
            EngineCommand::ApplyReceipt(receipt) => {
                let mut message = self.load_message(receipt.message_id)?;
                let changed = receipt.apply(&mut message).map_err(map_error)?;
                let _ = self.receipts.record(receipt).map_err(map_error)?;
                self.messages.update(message).map_err(map_error)?;
                Ok(EngineResult::ReceiptApplied { message_id: receipt.message_id, changed })
            }
        }
    }

    pub fn snapshot(&self) -> Result<ClientSnapshot, EngineError> {
        let mut snapshot = self.overview_snapshot()?;
        snapshot.messages = self.messages.list().map_err(map_error)?;
        let mut conversation_ids = Vec::new();
        for message in &snapshot.messages {
            let id = message.conversation_id();
            if !conversation_ids.contains(&id) {
                conversation_ids.push(id);
            }
        }
        for conversation_id in conversation_ids {
            snapshot.reactions.extend(
                self.messages.reactions_for_conversation(conversation_id).map_err(map_error)?,
            );
        }
        Ok(snapshot)
    }

    pub fn overview_snapshot(&self) -> Result<ClientSnapshot, EngineError> {
        let conversations = ConversationRepository::list(&self.relationships).map_err(map_error)?;
        Ok(ClientSnapshot {
            identity: self.identity.load().map_err(map_error)?,
            pairings: self.pairings.list().map_err(map_error)?,
            contacts: ContactRepository::list(&self.relationships).map_err(map_error)?,
            conversations,
            messages: Vec::new(),
            // Reactions belong to conversation history and are intentionally
            // omitted from the root overview projection.
            reactions: Vec::new(),
            avatar_genome: self.relationships.local_avatar_genome().map_err(map_error)?,
        })
    }

    pub fn message_status(
        &self,
        message_id: MessageId,
    ) -> Result<Option<MessageStatus>, EngineError> {
        self.messages
            .get(message_id)
            .map(|message| message.map(|value| value.status()))
            .map_err(map_error)
    }

    fn load_pairing(&self, id: PairingSessionId) -> Result<PairingSession, EngineError> {
        self.pairings
            .get(id)
            .map_err(map_error)?
            .ok_or_else(|| EngineError("pairing session not found".into()))
    }

    fn load_message(&self, id: MessageId) -> Result<Message, EngineError> {
        self.messages
            .get(id)
            .map_err(map_error)?
            .ok_or_else(|| EngineError("message not found".into()))
    }
}

pub trait EngineRuntime: Send + 'static {
    fn dispatch(&mut self, command: EngineCommand) -> Result<EngineResult, EngineError>;
    fn snapshot(&self) -> Result<ClientSnapshot, EngineError>;
    fn overview_snapshot(&self) -> Result<ClientSnapshot, EngineError> {
        self.snapshot()
    }
    fn avatar_genome_for_identity(
        &self,
        identity_id: IdentityId,
    ) -> Result<Option<AvatarGenomeRecord>, EngineError>;
    fn message_status(&self, message_id: MessageId) -> Result<Option<MessageStatus>, EngineError>;
}

impl<I, K, P, L, M, R> EngineRuntime for ClientEngine<I, K, P, L, M, R>
where
    I: IdentityRepository + Send + 'static,
    K: IdentityKeyProvider + Send + 'static,
    P: PairingRepository + Send + 'static,
    L: RelationshipRepository + Send + 'static,
    M: MessageRepository + Send + 'static,
    R: ReceiptRepository + Send + 'static,
{
    fn dispatch(&mut self, command: EngineCommand) -> Result<EngineResult, EngineError> {
        ClientEngine::dispatch(self, command)
    }
    fn snapshot(&self) -> Result<ClientSnapshot, EngineError> {
        ClientEngine::snapshot(self)
    }
    fn overview_snapshot(&self) -> Result<ClientSnapshot, EngineError> {
        ClientEngine::overview_snapshot(self)
    }
    fn avatar_genome_for_identity(
        &self,
        identity_id: IdentityId,
    ) -> Result<Option<AvatarGenomeRecord>, EngineError> {
        self.relationships.avatar_genome_for_identity(identity_id)
    }
    fn message_status(&self, message_id: MessageId) -> Result<Option<MessageStatus>, EngineError> {
        ClientEngine::message_status(self, message_id)
    }
}

fn map_error(error: impl fmt::Display) -> EngineError {
    EngineError(error.to_string())
}

enum ActorRequest {
    Dispatch(Box<EngineCommand>, Sender<Result<EngineResult, EngineError>>),
    Snapshot(Sender<Result<ClientSnapshot, EngineError>>),
    OverviewSnapshot(Sender<Result<ClientSnapshot, EngineError>>),
    AvatarGenomeForIdentity(IdentityId, Sender<Result<Option<AvatarGenomeRecord>, EngineError>>),
    MessageStatus(MessageId, Sender<Result<Option<MessageStatus>, EngineError>>),
    Shutdown,
}

#[derive(Clone)]
pub struct EngineHandle {
    sender: SyncSender<ActorRequest>,
    projection_events: Arc<AtomicU64>,
}
impl EngineHandle {
    pub fn dispatch(&self, command: EngineCommand) -> Result<EngineResult, EngineError> {
        let (sender, receiver) = mpsc::channel();
        send_with_timeout(&self.sender, ActorRequest::Dispatch(Box::new(command), sender))?;
        receiver
            .recv_timeout(Duration::from_secs(10))
            .map_err(|_| EngineError("engine response timed out".into()))?
    }
    pub fn snapshot(&self) -> Result<ClientSnapshot, EngineError> {
        let (sender, receiver) = mpsc::channel();
        send_with_timeout(&self.sender, ActorRequest::Snapshot(sender))?;
        receiver
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| EngineError("engine snapshot timed out".into()))?
    }
    pub fn overview_snapshot(&self) -> Result<ClientSnapshot, EngineError> {
        let (sender, receiver) = mpsc::channel();
        send_with_timeout(&self.sender, ActorRequest::OverviewSnapshot(sender))?;
        receiver
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| EngineError("engine overview timed out".into()))?
    }
    pub fn avatar_genome_for_identity(
        &self,
        identity_id: IdentityId,
    ) -> Result<Option<AvatarGenomeRecord>, EngineError> {
        let (sender, receiver) = mpsc::channel();
        send_with_timeout(
            &self.sender,
            ActorRequest::AvatarGenomeForIdentity(identity_id, sender),
        )?;
        receiver
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| EngineError("avatar genome query timed out".into()))?
    }

    pub fn message_status(
        &self,
        message_id: MessageId,
    ) -> Result<Option<MessageStatus>, EngineError> {
        let (sender, receiver) = mpsc::channel();
        send_with_timeout(&self.sender, ActorRequest::MessageStatus(message_id, sender))?;
        receiver
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| EngineError("engine message status timed out".into()))?
    }

    /// Cumulative successful receipt/reaction projection commands. This is a
    /// logical operation count, not an estimate of SQL statements or energy.
    pub fn projection_event_count(&self) -> u64 {
        self.projection_events.load(Ordering::Acquire)
    }
}

pub struct ClientEngineActor {
    sender: SyncSender<ActorRequest>,
    join: Option<JoinHandle<()>>,
}
impl ClientEngineActor {
    pub fn spawn<E: EngineRuntime>(mut engine: E) -> (EngineHandle, Self) {
        let (sender, receiver): (SyncSender<ActorRequest>, Receiver<ActorRequest>) =
            mpsc::sync_channel(256);
        let projection_events = Arc::new(AtomicU64::new(0));
        let handle = EngineHandle {
            sender: sender.clone(),
            projection_events: Arc::clone(&projection_events),
        };
        let join = thread::spawn(move || {
            loop {
                // The engine has no periodic maintenance responsibility. It
                // owns durable command/snapshot serialization only, so a
                // blocking receive removes one application wakeup per second
                // while idle. Deadline work belongs to torca-runtime.
                let request = match receiver.recv() {
                    Ok(request) => request,
                    Err(_) => break,
                };
                match request {
                    ActorRequest::Dispatch(command, response) => {
                        let counts_projection = matches!(
                            &*command,
                            EngineCommand::ApplyReceipt(_)
                                | EngineCommand::SetMessageReaction { .. }
                        );
                        let result = engine.dispatch(*command);
                        if counts_projection && result.is_ok() {
                            projection_events.fetch_add(1, Ordering::Release);
                        }
                        let _ = response.send(result);
                    }
                    ActorRequest::Snapshot(response) => {
                        let _ = response.send(engine.snapshot());
                    }
                    ActorRequest::OverviewSnapshot(response) => {
                        let _ = response.send(engine.overview_snapshot());
                    }
                    ActorRequest::AvatarGenomeForIdentity(identity_id, response) => {
                        let _ = response.send(engine.avatar_genome_for_identity(identity_id));
                    }
                    ActorRequest::MessageStatus(message_id, response) => {
                        let _ = response.send(engine.message_status(message_id));
                    }
                    ActorRequest::Shutdown => break,
                }
            }
        });
        (handle, Self { sender, join: Some(join) })
    }

    pub fn shutdown(mut self) -> Result<(), EngineError> {
        send_with_timeout(&self.sender, ActorRequest::Shutdown)?;
        if let Some(join) = self.join.take() {
            join.join().map_err(|_| EngineError("engine actor panicked".into()))?;
        }
        Ok(())
    }
}

fn send_with_timeout(
    sender: &SyncSender<ActorRequest>,
    mut request: ActorRequest,
) -> Result<(), EngineError> {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match sender.try_send(request) {
            Ok(()) => return Ok(()),
            Err(TrySendError::Disconnected(_)) => {
                return Err(EngineError("engine actor stopped".into()));
            }
            Err(TrySendError::Full(returned)) => {
                if Instant::now() >= deadline {
                    return Err(EngineError("engine actor mailbox timed out".into()));
                }
                request = returned;
                thread::yield_now();
            }
        }
    }
}
