#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalPairingContext {
    pub public_identity: PublicIdentity,
    pub display_name: String,
    pub onion_address: String,
    pub capability_id: OpaqueId,
    pub avatar: Option<AvatarEnvelope>,
}

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairingInvitation {
    pub session_id: PairingSessionId,
    pub code: PairingCode,
    pub uri: String,
    pub expires_at: Timestamp,
    pub ticket: PairingInviteTicket,
}

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairingCompletedContact {
    pub contact_id: ContactId,
    pub conversation_id: ConversationId,
    pub display_name: String,
}

#[must_use]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PairingPollReport {
    pub offers_applied: usize,
    pub approvals_applied: usize,
    pub completions_applied: usize,
    pub completion_acks_applied: usize,
    pub rejections_applied: usize,
    pub cancellations_applied: usize,
    pub completed_contact: Option<PairingCompletedContact>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PairingRuntimeError {
    Coordinator(PairingCoordinatorError),
    Approval(PairingApprovalError),
    Credential(PairingCredentialError),
    Engine,
    IdentityMissing,
    InvalidOffer,
    InvalidCompletion,
    UnsupportedAlgorithm,
    CreatorApprovalRequired,
    SessionNotFound,
}
impl core::fmt::Display for PairingRuntimeError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for PairingRuntimeError {}
impl From<PairingCoordinatorError> for PairingRuntimeError {
    fn from(value: PairingCoordinatorError) -> Self {
        Self::Coordinator(value)
    }
}
impl From<PairingApprovalError> for PairingRuntimeError {
    fn from(value: PairingApprovalError) -> Self {
        Self::Approval(value)
    }
}
impl From<PairingCredentialError> for PairingRuntimeError {
    fn from(value: PairingCredentialError) -> Self {
        Self::Credential(value)
    }
}

pub struct PairingRuntime<R, C, A, S> {
    coordinator: PairingCoordinator<R, C>,
    engine: EngineHandle,
    approval: A,
    peer_secrets: S,
    local_offers: BTreeMap<PairingSessionId, PairingEnvelope>,
    remote_offers: BTreeMap<PairingSessionId, PairingEnvelope>,
    completion_sent: BTreeSet<PairingSessionId>,
    completion_applied: BTreeSet<PairingSessionId>,
    completion_ack_sent: BTreeSet<PairingSessionId>,
}
