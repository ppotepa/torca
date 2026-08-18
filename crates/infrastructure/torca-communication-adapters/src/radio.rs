use std::collections::VecDeque;
use std::time::{Duration, Instant};

use rand_core::{OsRng, RngCore};
use torca_communication_driver::{
    CommunicationError, InboundEnvelope, RADIO_CONTROL_MESSAGE_KIND, RadioInboundRuntime,
};
use torca_contacts::{ContactId, ContactRepository, ContactStatus, PeerCredentialRepository};
use torca_crypto::{CryptoProvider, ProtectedSecretStore};
use torca_foundation::{OpaqueId, Timestamp};
use torca_peer_protocol::{AckStatus, HandshakeSigner};
use torca_peer_shared::SharedPeerLink;
use torca_radio::RadioSessionId;
use torca_radio_adapters::{RadioMediaCipher, RadioMediaDirectory, RadioMediaRoute};
use torca_radio_coordinator::{
    HostRadioLifecycle, RadioApplicationError, RadioControlPort, RadioEntropy, RadioPeerDirectory,
    SharedRadioCoordinator,
};
use torca_radio_protocol::{RadioControlCodec, RadioControlFrame};

use crate::SharedPeerCrypto;
use crate::adapters::{load_contact, load_credential, open, seal};

const RADIO_CONTROL_QUEUE_LIMIT: usize = 64;

/// Best-effort bounded outbox over the existing authenticated peer lane.
/// StateSync frames are revisioned and session commands are idempotent, so a
/// reconnect can safely retry the head without blocking the application actor.
pub struct PeerRadioControl<R, S, K, C, P> {
    relationships: R,
    link: SharedPeerLink<S, K>,
    crypto: SharedPeerCrypto<C, P>,
    local_identity: OpaqueId,
    queued: VecDeque<(ContactId, RadioControlFrame)>,
    next_attempt_at: Option<Instant>,
    retry_delay: Duration,
}

impl<R, S, K, C, P> PeerRadioControl<R, S, K, C, P> {
    pub fn new(
        relationships: R,
        link: SharedPeerLink<S, K>,
        crypto: SharedPeerCrypto<C, P>,
        local_identity: OpaqueId,
    ) -> Self {
        Self {
            relationships,
            link,
            crypto,
            local_identity,
            queued: VecDeque::new(),
            next_attempt_at: None,
            retry_delay: Duration::from_millis(500),
        }
    }
}

impl<R, S, K, C, P> RadioControlPort for PeerRadioControl<R, S, K, C, P>
where
    R: ContactRepository + PeerCredentialRepository + Send,
    S: ContactRepository + PeerCredentialRepository + Send,
    K: HandshakeSigner + Send,
    C: CryptoProvider + Send,
    P: ProtectedSecretStore + Send,
{
    fn send(
        &mut self,
        contact_id: ContactId,
        frame: RadioControlFrame,
    ) -> Result<(), RadioApplicationError> {
        if matches!(&frame, RadioControlFrame::StateSync { .. }) {
            if let Some(existing) = self.queued.iter_mut().find(|(queued_contact, queued_frame)| {
                *queued_contact == contact_id
                    && matches!(queued_frame, RadioControlFrame::StateSync { .. })
            }) {
                *existing = (contact_id, frame);
                return Ok(());
            }
        }
        if self.queued.len() >= RADIO_CONTROL_QUEUE_LIMIT {
            return Err(RadioApplicationError::ControlTransport);
        }
        eprintln!(
            "torca-radio: control queued contact={} frame={frame:?} queue_len={}",
            contact_id,
            self.queued.len().saturating_add(1),
        );
        self.queued.push_back((contact_id, frame));
        Ok(())
    }

    fn maintain(&mut self, _now: Timestamp) -> Result<(), RadioApplicationError> {
        let now_instant = Instant::now();
        if self.next_attempt_at.is_some_and(|deadline| deadline > now_instant) {
            return Ok(());
        }
        let Some((contact_id, frame)) = self.queued.front().cloned() else {
            return Ok(());
        };
        // Arm the retry deadline before doing repository/crypto/transport
        // work as well. Those early failures must not turn into an immediate
        // maintenance loop while the queue remains non-empty.
        self.next_attempt_at = Some(now_instant + self.retry_delay);
        let contact = load_contact(&self.relationships, contact_id)
            .map_err(|_| RadioApplicationError::ContactUnavailable)?;
        let credential = load_credential(&self.relationships, contact_id)
            .map_err(|_| RadioApplicationError::ContactUnavailable)?;
        let envelope_id = random_id()?;
        let payload = RadioControlCodec::encode(&frame);
        let ciphertext = seal(
            &self.crypto,
            credential.secret_handle(),
            envelope_id,
            RADIO_CONTROL_MESSAGE_KIND,
            self.local_identity,
            contact.remote_identity().identity_id().to_opaque(),
            &payload,
        )
        .map_err(|_| RadioApplicationError::Crypto)?;
        let result = self
            .link
            .send_envelope(contact_id, envelope_id, RADIO_CONTROL_MESSAGE_KIND, ciphertext)
            .is_ok();
        if result {
            eprintln!("torca-radio: control sent contact={contact_id}");
            self.queued.pop_front();
            self.next_attempt_at = None;
            self.retry_delay = Duration::from_millis(500);
        } else {
            self.next_attempt_at = Some(now_instant + self.retry_delay);
            self.retry_delay = (self.retry_delay * 2).min(Duration::from_secs(30));
        }
        Ok(())
    }

    fn next_maintenance_delay(&self) -> Option<Duration> {
        if self.queued.is_empty() {
            return None;
        }
        self.next_attempt_at
            .map(|deadline| deadline.saturating_duration_since(Instant::now()))
            .or(Some(Duration::ZERO))
    }
}

/// Decrypts Radio control envelopes and forwards typed frames to the shared
/// application coordinator.
pub struct RadioInboundAdapter<R, S, K, C, P> {
    relationships: R,
    link: SharedPeerLink<S, K>,
    crypto: SharedPeerCrypto<C, P>,
    local_identity: OpaqueId,
    coordinator: SharedRadioCoordinator,
}

impl<R, S, K, C, P> RadioInboundAdapter<R, S, K, C, P> {
    pub fn new(
        relationships: R,
        link: SharedPeerLink<S, K>,
        crypto: SharedPeerCrypto<C, P>,
        local_identity: OpaqueId,
        coordinator: SharedRadioCoordinator,
    ) -> Self {
        Self { relationships, link, crypto, local_identity, coordinator }
    }
}

impl<R, S, K, C, P> RadioInboundRuntime for RadioInboundAdapter<R, S, K, C, P>
where
    R: ContactRepository + PeerCredentialRepository + Send,
    S: ContactRepository + PeerCredentialRepository + Send,
    K: HandshakeSigner + Send,
    C: CryptoProvider + Send,
    P: ProtectedSecretStore + Send,
{
    fn process_control(
        &mut self,
        envelope: InboundEnvelope,
        now: Timestamp,
    ) -> Result<(), CommunicationError> {
        let contact = load_contact(&self.relationships, envelope.contact_id)?;
        let credential = load_credential(&self.relationships, envelope.contact_id)?;
        let plaintext = open(
            &self.crypto,
            credential.secret_handle(),
            envelope.envelope_id,
            RADIO_CONTROL_MESSAGE_KIND,
            self.local_identity,
            contact.remote_identity().identity_id().to_opaque(),
            &envelope.ciphertext,
        )?;
        let frame =
            RadioControlCodec::decode(&plaintext).map_err(|_| CommunicationError::Inbound)?;
        let result = self
            .coordinator
            .receive_control(envelope.contact_id, frame, now)
            .map_err(|_| CommunicationError::Control);
        let status = if result.is_ok() { AckStatus::Accepted } else { AckStatus::Rejected };
        self.link
            .send_ack(envelope.contact_id, envelope.envelope_id, status)
            .map_err(|_| CommunicationError::Peer)?;
        result
    }

    fn maintenance(&mut self, now: Timestamp) -> Result<(), CommunicationError> {
        self.coordinator.maintain(now).map_err(|_| CommunicationError::Control)
    }

    fn next_maintenance_delay(&self, now: Timestamp) -> Option<Duration> {
        self.coordinator.next_maintenance_delay(now)
    }

    fn shutdown(&mut self) {
        let _ = self.coordinator.lifecycle(HostRadioLifecycle::Terminating);
    }
}

/// Relationship view used by domain consent/coordinator selection.
pub struct RelationshipRadioPeers<R> {
    relationships: R,
    local_identity: OpaqueId,
}

impl<R> RelationshipRadioPeers<R> {
    pub const fn new(relationships: R, local_identity: OpaqueId) -> Self {
        Self { relationships, local_identity }
    }
}

impl<R> RadioPeerDirectory for RelationshipRadioPeers<R>
where
    R: ContactRepository + Send,
{
    fn local_identity(&self) -> OpaqueId {
        self.local_identity
    }

    fn remote_identity(&self, contact_id: ContactId) -> Option<OpaqueId> {
        self.relationships
            .get(contact_id)
            .ok()
            .flatten()
            .filter(|contact| contact.status() == ContactStatus::Active)
            .map(|contact| contact.remote_identity().identity_id().to_opaque())
    }

    fn contact_available(&self, contact_id: ContactId) -> bool {
        self.remote_identity(contact_id).is_some()
    }
}

/// Relationship/secret facade used by the dedicated onion media worker.
pub struct RelationshipRadioMedia<R, C, P> {
    relationships: R,
    crypto: SharedPeerCrypto<C, P>,
    local_identity: OpaqueId,
}

impl<R, C, P> RelationshipRadioMedia<R, C, P> {
    pub fn new(relationships: R, crypto: SharedPeerCrypto<C, P>, local_identity: OpaqueId) -> Self {
        Self { relationships, crypto, local_identity }
    }
}

impl<R, C, P> RadioMediaDirectory for RelationshipRadioMedia<R, C, P>
where
    R: ContactRepository + PeerCredentialRepository + Send,
    C: CryptoProvider + Clone + Send + 'static,
    P: ProtectedSecretStore + Send,
{
    fn route(&self, contact_id: ContactId) -> Option<RadioMediaRoute> {
        self.relationships
            .get(contact_id)
            .ok()
            .flatten()
            .filter(|contact| contact.status() == ContactStatus::Active)
            .map(|contact| RadioMediaRoute {
                onion_address: contact.route().onion_address().to_owned(),
                local_identity: self.local_identity,
                remote_identity: contact.remote_identity().identity_id().to_opaque(),
            })
    }

    fn session_cipher(
        &self,
        contact_id: ContactId,
        session_id: RadioSessionId,
        media_token: &[u8; 32],
    ) -> Result<Box<dyn RadioMediaCipher>, RadioApplicationError> {
        let contact = load_contact(&self.relationships, contact_id)
            .map_err(|_| RadioApplicationError::ContactUnavailable)?;
        let credential = load_credential(&self.relationships, contact_id)
            .map_err(|_| RadioApplicationError::ContactUnavailable)?;
        let cipher = self
            .crypto
            .inner
            .lock()
            .map_err(|_| RadioApplicationError::Crypto)?
            .derive_radio_session_cipher(
                credential.secret_handle(),
                session_id.to_opaque(),
                media_token,
                self.local_identity,
                contact.remote_identity().identity_id().to_opaque(),
            )
            .map_err(|_| RadioApplicationError::Crypto)?;
        Ok(Box::new(cipher))
    }
}

#[derive(Default)]
pub struct OsRadioEntropy;

impl RadioEntropy for OsRadioEntropy {
    fn opaque_id(&mut self) -> Result<OpaqueId, RadioApplicationError> {
        random_id()
    }

    fn bytes_16(&mut self) -> Result<[u8; 16], RadioApplicationError> {
        random_bytes()
    }

    fn bytes_32(&mut self) -> Result<[u8; 32], RadioApplicationError> {
        random_bytes()
    }
}

fn random_id() -> Result<OpaqueId, RadioApplicationError> {
    random_bytes().map(OpaqueId::from_bytes)
}

fn random_bytes<const N: usize>() -> Result<[u8; N], RadioApplicationError> {
    let mut bytes = [0_u8; N];
    OsRng.try_fill_bytes(&mut bytes).map_err(|_| RadioApplicationError::Crypto)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use torca_radio::RadioOperationId;

    #[test]
    fn os_entropy_produces_non_nil_operation_ids() {
        let mut entropy = OsRadioEntropy;
        let id = entropy.opaque_id().expect("entropy");
        assert!(!id.is_nil());
        let operation = RadioOperationId::from_opaque(id);
        assert_eq!(operation.to_opaque(), id);
    }
}
