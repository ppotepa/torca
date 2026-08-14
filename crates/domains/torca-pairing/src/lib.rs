//! Explicit invitation and pairing approval state machine.

use core::fmt;
use std::collections::BTreeMap;

use torca_contacts::ContactRoute;
use torca_foundation::{OpaqueId, Timestamp};
use torca_identity::PublicIdentity;

const CROCKFORD_BASE32: &str = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// Pairing session ID.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PairingSessionId(OpaqueId);
impl PairingSessionId {
    /// Creates an ID.
    pub const fn from_opaque(value: OpaqueId) -> Self {
        Self(value)
    }
    /// Creates an ID from an integer.
    pub const fn from_u128(value: u128) -> Self {
        Self(OpaqueId::from_u128(value))
    }
    /// Returns the opaque representation.
    pub const fn to_opaque(self) -> OpaqueId {
        self.0
    }
}
impl fmt::Display for PairingSessionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, formatter)
    }
}

/// Short-lived pairing code.
#[must_use]
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PairingCode(String);
impl PairingCode {
    /// Creates the canonical six-character Crockford Base32 invitation code.
    ///
    /// Presentation may group a code as `ABC-12`; spaces and separators are
    /// deliberately ignored here so every entry point (manual input, QR and
    /// deep links) has one canonical representation.  Ambiguous `O`, `I` and
    /// `L` are accepted as their human-friendly `0`/`1` equivalents, while the
    /// generator never emits them.
    pub fn new(value: impl Into<String>) -> Result<Self, PairingError> {
        let mut value = value
            .into()
            .chars()
            .filter(|character| !matches!(character, '-' | ' '))
            .collect::<String>()
            .to_ascii_uppercase();
        value = value
            .chars()
            .map(|character| match character {
                'O' => '0',
                'I' | 'L' => '1',
                other => other,
            })
            .collect();
        if value.len() != 6 || !value.chars().all(|character| CROCKFORD_BASE32.contains(character))
        {
            return Err(PairingError::InvalidCode);
        }
        Ok(Self(value))
    }
    /// Returns the code.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod pairing_code_tests {
    use super::{PairingCode, PairingError};

    #[test]
    fn normalizes_human_friendly_crockford_input() {
        let code = PairingCode::new("ab c-iO2").expect("normalizable code");
        assert_eq!(code.as_str(), "ABC102");
    }

    #[test]
    fn rejects_legacy_and_ambiguous_alphabet_values() {
        assert_eq!(PairingCode::new("ABCDE"), Err(PairingError::InvalidCode));
        assert_eq!(PairingCode::new("ABCDEFG"), Err(PairingError::InvalidCode));
        assert_eq!(PairingCode::new("ABCU1!"), Err(PairingError::InvalidCode));
    }
}

/// Local role in a pairing session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PairingRole {
    Creator,
    Joiner,
}
/// State of explicit pairing approval.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PairingState {
    Open,
    PeerJoined,
    AwaitingApproval,
    Approved,
    Rejected,
    Cancelled,
    Expired,
    Completed,
}

/// Public peer proposal transported opaquely by the relay.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PeerProposal {
    pub public_identity: PublicIdentity,
    pub display_name: String,
    pub route: ContactRoute,
    /// Opaque, content-addressed avatar genome carried by the signed offer.
    pub avatar: Option<AvatarGenomeReference>,
}

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AvatarGenomeReference {
    pub schema_version: u8,
    pub generator_version: String,
    pub catalog_version: String,
    pub genome_hash: [u8; 32],
    pub compressed_genome: Vec<u8>,
}

/// Pairing session aggregate.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairingSession {
    id: PairingSessionId,
    code: PairingCode,
    role: PairingRole,
    state: PairingState,
    expires_at: Timestamp,
    local_approved: bool,
    remote_approved: bool,
    remote_proposal: Option<PeerProposal>,
}
impl PairingSession {
    /// Creates a creator session waiting for a remote proposal.
    pub const fn creator(id: PairingSessionId, code: PairingCode, expires_at: Timestamp) -> Self {
        Self {
            id,
            code,
            role: PairingRole::Creator,
            state: PairingState::Open,
            expires_at,
            local_approved: false,
            remote_approved: false,
            remote_proposal: None,
        }
    }
    /// Creates a joiner session while rendezvous resolves the creator proposal.
    ///
    /// The proposal is deliberately absent here so presentation code only needs the invitation
    /// code. The application coordinator later supplies the verified proposal through
    /// [`Self::peer_joined`].
    pub const fn joining(id: PairingSessionId, code: PairingCode, expires_at: Timestamp) -> Self {
        Self {
            id,
            code,
            role: PairingRole::Joiner,
            state: PairingState::Open,
            expires_at,
            local_approved: false,
            remote_approved: false,
            remote_proposal: None,
        }
    }
    /// Creates a joiner session with the creator proposal already known.
    pub const fn joiner(
        id: PairingSessionId,
        code: PairingCode,
        expires_at: Timestamp,
        remote_proposal: PeerProposal,
    ) -> Self {
        Self {
            id,
            code,
            role: PairingRole::Joiner,
            state: PairingState::AwaitingApproval,
            expires_at,
            local_approved: false,
            remote_approved: false,
            remote_proposal: Some(remote_proposal),
        }
    }
    /// Returns the ID.
    pub const fn id(&self) -> PairingSessionId {
        self.id
    }
    /// Returns the code.
    pub const fn code(&self) -> &PairingCode {
        &self.code
    }
    /// Returns the state.
    pub const fn state(&self) -> PairingState {
        self.state
    }
    /// Returns the role.
    pub const fn role(&self) -> PairingRole {
        self.role
    }
    /// Returns the pairing expiry deadline.
    pub const fn expires_at(&self) -> Timestamp {
        self.expires_at
    }
    /// Returns whether this side explicitly approved the peer.
    pub const fn local_approved(&self) -> bool {
        self.local_approved
    }
    /// Returns whether remote approval has been observed.
    pub const fn remote_approved(&self) -> bool {
        self.remote_approved
    }
    /// Returns the verified remote proposal, when one has been received.
    pub const fn remote_proposal(&self) -> Option<&PeerProposal> {
        self.remote_proposal.as_ref()
    }
    /// Restores a session from durable storage.
    pub fn restore(
        id: PairingSessionId,
        code: PairingCode,
        role: PairingRole,
        state: PairingState,
        expires_at: Timestamp,
        local_approved: bool,
        remote_approved: bool,
        remote_proposal: Option<PeerProposal>,
    ) -> Result<Self, PairingError> {
        if matches!(
            state,
            PairingState::AwaitingApproval | PairingState::Approved | PairingState::Completed
        ) && remote_proposal.is_none()
        {
            return Err(PairingError::InvalidTransition);
        }
        Ok(Self {
            id,
            code,
            role,
            state,
            expires_at,
            local_approved,
            remote_approved,
            remote_proposal,
        })
    }
    /// Records the verified remote proposal for either local role.
    pub fn peer_joined(
        &mut self,
        proposal: PeerProposal,
        now: Timestamp,
    ) -> Result<(), PairingError> {
        self.ensure_live(now)?;
        if self.state != PairingState::Open {
            return Err(PairingError::InvalidTransition);
        }
        self.remote_proposal = Some(proposal);
        self.state = PairingState::AwaitingApproval;
        Ok(())
    }
    /// Approves locally.
    pub fn approve_local(&mut self, now: Timestamp) -> Result<(), PairingError> {
        self.ensure_live(now)?;
        if self.state != PairingState::AwaitingApproval {
            return Err(PairingError::InvalidTransition);
        }
        self.local_approved = true;
        self.state = PairingState::Approved;
        Ok(())
    }
    /// Records remote approval.
    pub fn approve_remote(&mut self, now: Timestamp) -> Result<(), PairingError> {
        self.ensure_live(now)?;
        if !matches!(self.state, PairingState::AwaitingApproval | PairingState::Approved) {
            return Err(PairingError::InvalidTransition);
        }
        self.remote_approved = true;
        Ok(())
    }
    /// Rejects the session.
    pub fn reject(&mut self) -> Result<(), PairingError> {
        if self.is_terminal() {
            return Err(PairingError::InvalidTransition);
        }
        self.state = PairingState::Rejected;
        Ok(())
    }
    /// Cancels the session.
    pub fn cancel(&mut self) -> Result<(), PairingError> {
        if self.is_terminal() {
            return Err(PairingError::InvalidTransition);
        }
        self.state = PairingState::Cancelled;
        Ok(())
    }
    /// Expires when the deadline is reached.
    pub fn expire(&mut self, now: Timestamp) -> bool {
        if !self.is_terminal() && now >= self.expires_at {
            self.state = PairingState::Expired;
            true
        } else {
            false
        }
    }
    /// Returns whether both sides approved and a proposal exists.
    pub fn can_complete(&self, now: Timestamp) -> bool {
        now < self.expires_at
            && self.local_approved
            && self.remote_approved
            && self.remote_proposal.is_some()
            && !self.is_terminal()
    }
    /// Completes and returns the verified remote proposal.
    pub fn complete(&mut self, now: Timestamp) -> Result<PeerProposal, PairingError> {
        if !self.can_complete(now) {
            return Err(PairingError::NotReady);
        }
        self.state = PairingState::Completed;
        self.remote_proposal.clone().ok_or(PairingError::MissingProposal)
    }
    fn ensure_live(&mut self, now: Timestamp) -> Result<(), PairingError> {
        if self.expire(now) {
            return Err(PairingError::Expired);
        }
        if self.is_terminal() {
            return Err(PairingError::InvalidTransition);
        }
        Ok(())
    }
    const fn is_terminal(&self) -> bool {
        matches!(
            self.state,
            PairingState::Rejected
                | PairingState::Cancelled
                | PairingState::Expired
                | PairingState::Completed
        )
    }
}

/// Pairing domain error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PairingError {
    InvalidCode,
    InvalidTransition,
    Expired,
    NotReady,
    MissingProposal,
    AlreadyExists,
    NotFound,
    Storage,
}
impl fmt::Display for PairingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for PairingError {}

/// Pairing repository port.
pub trait PairingRepository {
    /// Inserts a session.
    fn insert(&mut self, session: PairingSession) -> Result<(), PairingError>;
    /// Loads a session.
    fn get(&self, id: PairingSessionId) -> Result<Option<PairingSession>, PairingError>;
    /// Replaces a session.
    fn update(&mut self, session: PairingSession) -> Result<(), PairingError>;
    /// Lists sessions.
    fn list(&self) -> Result<Vec<PairingSession>, PairingError>;
    /// Removes a terminal session. Pairing history is intentionally not retained.
    fn delete(&mut self, id: PairingSessionId) -> Result<(), PairingError>;
}

/// In-memory pairing repository.
#[derive(Clone, Debug, Default)]
pub struct InMemoryPairingRepository {
    sessions: BTreeMap<PairingSessionId, PairingSession>,
}
impl PairingRepository for InMemoryPairingRepository {
    fn insert(&mut self, session: PairingSession) -> Result<(), PairingError> {
        if self.sessions.contains_key(&session.id()) {
            return Err(PairingError::AlreadyExists);
        }
        self.sessions.insert(session.id(), session);
        Ok(())
    }
    fn get(&self, id: PairingSessionId) -> Result<Option<PairingSession>, PairingError> {
        Ok(self.sessions.get(&id).cloned())
    }
    fn update(&mut self, session: PairingSession) -> Result<(), PairingError> {
        if !self.sessions.contains_key(&session.id()) {
            return Err(PairingError::NotFound);
        }
        self.sessions.insert(session.id(), session);
        Ok(())
    }
    fn list(&self) -> Result<Vec<PairingSession>, PairingError> {
        Ok(self.sessions.values().cloned().collect())
    }
    fn delete(&mut self, id: PairingSessionId) -> Result<(), PairingError> {
        self.sessions.remove(&id).map(|_| ()).ok_or(PairingError::NotFound)
    }
}
