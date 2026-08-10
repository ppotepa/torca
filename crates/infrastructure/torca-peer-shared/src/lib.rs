//! Shared ownership handle for the single process-owned PeerLink.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use torca_contacts::{ContactId, ContactRepository, PeerCredentialRepository};
use torca_foundation::{OpaqueId, Timestamp};
use torca_peer_link::{
    InboundPeerEnvelope, LinkAck, PeerConnectionState, PeerLink, PeerLinkError, PeerLinkReport,
};
use torca_peer_protocol::{AckStatus, HandshakeSigner};

pub struct SharedPeerLink<S, K> {
    inner: Arc<Mutex<PeerLink<S, K>>>,
}
impl<S, K> Clone for SharedPeerLink<S, K> {
    fn clone(&self) -> Self {
        Self { inner: Arc::clone(&self.inner) }
    }
}
impl<S, K> SharedPeerLink<S, K> {
    pub fn new(link: PeerLink<S, K>) -> Self {
        Self { inner: Arc::new(Mutex::new(link)) }
    }
}

impl<S, K> SharedPeerLink<S, K>
where
    S: ContactRepository + PeerCredentialRepository,
    K: HandshakeSigner,
{
    pub fn maintenance(
        &self,
        contacts: &[ContactId],
        now: Timestamp,
    ) -> Result<PeerLinkReport, PeerLinkError> {
        self.inner.lock().map_err(|_| PeerLinkError::Protocol)?.maintenance(contacts, now)
    }

    pub fn connection_state(&self, contact_id: ContactId) -> PeerConnectionState {
        self.inner
            .lock()
            .map_or(PeerConnectionState::Failed, |link| link.connection_state(contact_id))
    }

    pub fn send_and_wait_ack(
        &self,
        contact_id: ContactId,
        envelope_id: OpaqueId,
        message_kind: u16,
        ciphertext: Vec<u8>,
        timeout: Duration,
    ) -> Result<LinkAck, PeerLinkError> {
        self.inner.lock().map_err(|_| PeerLinkError::Protocol)?.send_and_wait_ack(
            contact_id,
            envelope_id,
            message_kind,
            ciphertext,
            timeout,
        )
    }

    pub fn send_keepalive_and_wait_ack(
        &self,
        contact_id: ContactId,
        envelope_id: OpaqueId,
        message_kind: u16,
        ciphertext: Vec<u8>,
        timeout: Duration,
    ) -> Result<LinkAck, PeerLinkError> {
        self.inner.lock().map_err(|_| PeerLinkError::Protocol)?.send_keepalive_and_wait_ack(
            contact_id,
            envelope_id,
            message_kind,
            ciphertext,
            timeout,
        )
    }

    pub fn send_ack(
        &self,
        contact_id: ContactId,
        envelope_id: OpaqueId,
        status: AckStatus,
    ) -> Result<(), PeerLinkError> {
        self.inner.lock().map_err(|_| PeerLinkError::Protocol)?.send_ack(
            contact_id,
            envelope_id,
            status,
        )
    }

    pub fn take_inbound(&self) -> Result<Option<InboundPeerEnvelope>, PeerLinkError> {
        Ok(self.inner.lock().map_err(|_| PeerLinkError::Protocol)?.take_inbound())
    }

    pub fn shutdown(&self) {
        if let Ok(mut link) = self.inner.lock() {
            link.shutdown();
        }
    }
}
