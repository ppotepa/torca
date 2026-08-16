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
