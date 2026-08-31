// Responsibility: public command/result model and classified engine errors.

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EngineCommand {
    CreateIdentity { identity_id: IdentityId, profile: Option<Profile>, at: Timestamp },
    UpdateProfile { display_name: ProfileName, country_code: Option<String>, at: Timestamp },
    SetAvatarGenome { record: AvatarGenomeRecord, at: Timestamp },
    StartPairing { session_id: PairingSessionId, code: PairingCode, expires_at: Timestamp },
    JoinPairing { session_id: PairingSessionId, code: PairingCode, expires_at: Timestamp },
    PeerJoined { session_id: PairingSessionId, proposal: PeerProposal, at: Timestamp },
    ApprovePairing { session_id: PairingSessionId, at: Timestamp },
    RejectPairing { session_id: PairingSessionId },
    CancelPairing { session_id: PairingSessionId },
    ExpirePairing { session_id: PairingSessionId, at: Timestamp },
    RemoteApproved { session_id: PairingSessionId, at: Timestamp },
    CompletePairing {
        session_id: PairingSessionId,
        contact_id: ContactId,
        conversation_id: ConversationId,
        display_name: String,
        country_code: Option<String>,
        credential: PeerCredential,
        at: Timestamp,
    },
    EnsureConversation { contact_id: ContactId, conversation_id: ConversationId, at: Timestamp },
    ArchiveConversation { conversation_id: ConversationId, at: Timestamp },
    RestoreConversation { conversation_id: ConversationId, at: Timestamp },
    RemoveContact { contact_id: ContactId },
    RemovePairing { session_id: PairingSessionId },
    QueueMessage {
        message_id: MessageId,
        conversation_id: ConversationId,
        body: MessageBody,
        reply_to: Option<ReplyReference>,
        at: Timestamp,
    },
    CancelMessage { message_id: MessageId, at: Timestamp },
    DeleteMessage { message_id: MessageId, at: Timestamp },
    ApplyMessageDeletion { message_id: MessageId, at: Timestamp },
    EditMessage { message_id: MessageId, body: MessageBody, at: Timestamp },
    SetMessageReaction { reaction: MessageReaction },
    BeginMessageSend { message_id: MessageId, at: Timestamp },
    MarkMessageSent { message_id: MessageId, at: Timestamp },
    MarkMessageFailed { message_id: MessageId, at: Timestamp, error_code: ErrorCode },
    RetryMessage { message_id: MessageId, at: Timestamp },
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

/// Stable, redacted application-engine failure taxonomy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EngineError {
    NotFound,
    Conflict,
    InvalidState,
    Repository,
    Identity,
    Pairing,
    Messaging,
    Unavailable,
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NotFound => "engine resource not found",
            Self::Conflict => "engine state conflict",
            Self::InvalidState => "engine operation is invalid for the current state",
            Self::Repository => "engine repository operation failed",
            Self::Identity => "identity operation failed",
            Self::Pairing => "pairing operation failed",
            Self::Messaging => "messaging operation failed",
            Self::Unavailable => "engine is temporarily unavailable",
        })
    }
}
impl std::error::Error for EngineError {}

impl ClassifiedError for EngineError {
    fn descriptor(&self) -> ErrorDescriptor {
        let (code, category, retry) = match self {
            Self::NotFound => (
                ErrorCode::new("application.engine.not_found"),
                ErrorCategory::NotFound,
                RetryAdvice::Never,
            ),
            Self::Conflict => (
                ErrorCode::new("application.engine.conflict"),
                ErrorCategory::Conflict,
                RetryAdvice::Never,
            ),
            Self::InvalidState => (
                ErrorCode::new("application.engine.invalid_state"),
                ErrorCategory::Conflict,
                RetryAdvice::Never,
            ),
            Self::Repository => (
                ErrorCode::new("application.engine.repository_unavailable"),
                ErrorCategory::Unavailable,
                RetryAdvice::Backoff,
            ),
            Self::Identity => (
                ErrorCode::new("application.engine.identity_failed"),
                ErrorCategory::Internal,
                RetryAdvice::Never,
            ),
            Self::Pairing => (
                ErrorCode::new("application.engine.pairing_failed"),
                ErrorCategory::Conflict,
                RetryAdvice::Never,
            ),
            Self::Messaging => (
                ErrorCode::new("application.engine.messaging_failed"),
                ErrorCategory::Conflict,
                RetryAdvice::Never,
            ),
            Self::Unavailable => (
                ErrorCode::new("application.engine.unavailable"),
                ErrorCategory::Unavailable,
                RetryAdvice::Backoff,
            ),
        };
        ErrorDescriptor::new(code, category, retry)
    }
}
