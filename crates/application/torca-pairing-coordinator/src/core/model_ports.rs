#[must_use]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PairingCryptoHandle(pub OpaqueId);

#[must_use]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PairingSlotId(pub OpaqueId);

#[must_use]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PairingContextId(pub OpaqueId);

#[must_use]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PairingSlotCapability(pub OpaqueId);

#[must_use]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PairingSideToken(pub OpaqueId);

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairingSessionDelivery {
    pub sequence: u64,
    pub blob: Vec<u8>,
}

/// Compatibility alias used by the Tor rendezvous adapter. New pairing code
/// must use [`PairingSessionDelivery`].
pub type PairingPairingServiceDelivery = PairingSessionDelivery;

#[must_use]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PairingPollBatch {
    pub envelopes: Vec<PairingEnvelope>,
    pub received_through: Option<u64>,
}

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairingEphemeralKey {
    pub handle: PairingCryptoHandle,
    pub public_key: [u8; 32],
}

#[must_use]
#[derive(Eq, PartialEq)]
pub struct PairingTransportSnapshot {
    pub role: PairingRole,
    pub context: PairingContextId,
    pub private_key: [u8; 32],
    pub slot: PairingSlotId,
    pub token: PairingSideToken,
    pub slot_capability: Option<PairingSlotCapability>,
    pub remote_public_key: Option<[u8; 32]>,
    /// Creator metadata needed to reconstruct provider-owned pairing slots
    /// after a process restart. Joiner snapshots leave these fields empty.
    pub invitation_code: Option<String>,
    pub invitation_expires_at: Option<torca_foundation::Timestamp>,
    pub invitation_ticket: Option<[u8; 16]>,
    pub creator_blob: Option<Vec<u8>>,
}
impl fmt::Debug for PairingTransportSnapshot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PairingTransportSnapshot")
            .field("role", &self.role)
            .field("context", &self.context)
            .field("private_key", &"[REDACTED]")
            .field("slot", &self.slot)
            .field("token", &"[REDACTED]")
            .field("slot_capability", &self.slot_capability.as_ref().map(|_| "[REDACTED]"))
            .field("remote_public_key", &self.remote_public_key)
            .field("invitation_code", &self.invitation_code)
            .field("invitation_expires_at", &self.invitation_expires_at)
            .field("invitation_ticket", &self.invitation_ticket.as_ref().map(|_| "[REDACTED]"))
            .field("creator_blob", &self.creator_blob.as_ref().map(|blob| blob.len()))
            .finish()
    }
}
impl Drop for PairingTransportSnapshot {
    fn drop(&mut self) {
        self.private_key.fill(0);
    }
}

#[must_use]
#[derive(Eq, PartialEq)]
pub struct PairingDerivedSecret(torca_foundation::SecretBytes<32>);
impl PairingDerivedSecret {
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(torca_foundation::SecretBytes::new(bytes))
    }

    pub fn expose_for_protected_storage(&self) -> &[u8; 32] {
        self.0.expose()
    }
}
impl fmt::Debug for PairingDerivedSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PairingDerivedSecret([REDACTED])")
    }
}

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncryptedPairingPayload {
    pub sender_public_key: [u8; 32],
    pub nonce: [u8; 24],
    pub ciphertext: Vec<u8>,
}

pub trait PairingCryptoPort {
    fn generate_ephemeral_key(&mut self) -> Result<PairingEphemeralKey, PairingCoordinatorError>;
    fn release_ephemeral_key(
        &mut self,
        handle: PairingCryptoHandle,
    ) -> Result<(), PairingCoordinatorError>;
    fn export_ephemeral_key(
        &self,
        handle: PairingCryptoHandle,
    ) -> Result<[u8; 32], PairingCoordinatorError>;
    fn import_ephemeral_key(
        &mut self,
        private_key: [u8; 32],
    ) -> Result<PairingEphemeralKey, PairingCoordinatorError>;
    fn fill_random(&mut self, output: &mut [u8]) -> Result<(), PairingCoordinatorError>;
    fn seal_for_peer(
        &self,
        local_key: PairingCryptoHandle,
        remote_public_key: [u8; 32],
        nonce: [u8; 24],
        associated_data: &[u8],
        plaintext: &[u8],
    ) -> Result<Vec<u8>, PairingCoordinatorError>;
    fn open_from_peer(
        &self,
        local_key: PairingCryptoHandle,
        remote_public_key: [u8; 32],
        nonce: [u8; 24],
        associated_data: &[u8],
        ciphertext: &[u8],
    ) -> Result<Vec<u8>, PairingCoordinatorError>;
    fn derive_peer_secret(
        &self,
        local_key: PairingCryptoHandle,
        remote_public_key: [u8; 32],
        transcript_digest: [u8; 32],
    ) -> Result<PairingDerivedSecret, PairingCoordinatorError>;
}

/// Provider-owned short-lived exchange used during pairing.
///
/// The default Tor implementation uses a rendezvous relay, while a direct
/// provider may implement the same slot/push/poll semantics over discovery or
/// signaling.  The coordinator deliberately has no dependency on either.
pub trait PairingSessionServicePort {
    fn network_changed(&mut self) {}
    fn open(
        &mut self,
        code: &PairingCode,
        expires_at: Timestamp,
        creator_blob: Vec<u8>,
        capability: PairingSlotCapability,
        creator_token: PairingSideToken,
        ticket: [u8; 16],
    ) -> Result<(PairingSlotId, Timestamp), PairingCoordinatorError>;
    fn join(
        &mut self,
        code: &PairingCode,
        joiner_blob: Vec<u8>,
        joiner_token: PairingSideToken,
        ticket: Option<[u8; 16]>,
        bootstrap: Option<&torca_pairing_protocol::PairingBootstrapDescriptor>,
    ) -> Result<(PairingSlotId, Timestamp, Vec<u8>), PairingCoordinatorError>;
    fn push(
        &mut self,
        message_id: OpaqueId,
        slot: PairingSlotId,
        token: PairingSideToken,
        blob: Vec<u8>,
    ) -> Result<(), PairingCoordinatorError>;
    fn poll(
        &mut self,
        slot: PairingSlotId,
        token: PairingSideToken,
        after: u64,
    ) -> Result<Vec<PairingSessionDelivery>, PairingCoordinatorError>;
    fn ack(
        &mut self,
        slot: PairingSlotId,
        token: PairingSideToken,
        up_to: u64,
    ) -> Result<(), PairingCoordinatorError>;
    fn close(
        &mut self,
        slot: PairingSlotId,
        capability: PairingSlotCapability,
    ) -> Result<(), PairingCoordinatorError>;

    /// Recreates a provider-owned creator slot after the application process
    /// was restarted. Providers that persist slots outside the client (Tor)
    /// may keep the default no-op; direct providers with process-local slots
    /// (Iroh) must restore their slot from this public, non-secret metadata.
    fn restore_creator(
        &mut self,
        _slot: PairingSlotId,
        _code: &PairingCode,
        _expires_at: Timestamp,
        _creator_blob: Vec<u8>,
        _capability: PairingSlotCapability,
        _creator_token: PairingSideToken,
        _ticket: [u8; 16],
    ) -> Result<(), PairingCoordinatorError> {
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PairingCoordinatorError {
    SessionAlreadyExists,
    SessionNotFound,
    InvalidRole,
    InvalidBlob,
    Protocol,
    Crypto,
    /// Failure of whichever provider-owned service exchanges pairing frames.
    SessionService,
    /// The selected provider requires bootstrap material (for example an
    /// Iroh endpoint descriptor), but the invitation contained only a code.
    /// This is a terminal input error and must never be retried as a network
    /// outage.
    BootstrapMissing,
    /// The invitation was created for a different communication provider.
    BootstrapProviderMismatch,
    /// The provider bootstrap envelope could not be decoded.
    BootstrapInvalid,
    /// Compatibility classification reserved for historic Tor-only callers.
    Rendezvous,
}
impl fmt::Display for PairingCoordinatorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for PairingCoordinatorError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LocalRole {
    Creator,
    Joiner,
}

#[derive(Clone, Debug)]
struct TransportSession {
    role: LocalRole,
    context: PairingContextId,
    key: PairingEphemeralKey,
    slot: PairingSlotId,
    token: PairingSideToken,
    slot_capability: Option<PairingSlotCapability>,
    remote_public_key: Option<[u8; 32]>,
    acknowledged_through: u64,
    invitation_code: Option<PairingCode>,
    invitation_expires_at: Option<Timestamp>,
    invitation_ticket: Option<[u8; 16]>,
}

pub struct PairingCoordinator<R, C> {
    rendezvous: R,
    pub(crate) crypto: C,
    sessions: BTreeMap<PairingSessionId, TransportSession>,
}
