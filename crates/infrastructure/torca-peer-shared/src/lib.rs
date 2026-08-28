//! Shared ownership handle for the single process-owned PeerLink.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use torca_contacts::{ContactId, ContactRepository, PeerCredentialRepository};
use torca_foundation::{OpaqueId, ProviderId, Timestamp};
use torca_peer_link::{
    InboundPeerEnvelope, LinkAck, PeerActivitySnapshot, PeerConnectionState, PeerLink,
    PeerLinkError, PeerLinkReport,
};
use torca_peer_protocol::{AckStatus, HandshakeSigner};
use torca_transport_api::TransportCapabilities;

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
    pub fn set_waker(&self, waker: Arc<dyn Fn() + Send + Sync>) -> Result<(), PeerLinkError> {
        self.inner.lock().map_err(|_| PeerLinkError::Protocol)?.set_waker(waker)
    }

    pub fn provider_id(&self) -> Result<ProviderId, PeerLinkError> {
        self.inner.lock().map(|link| link.provider_id()).map_err(|_| PeerLinkError::Protocol)
    }

    pub fn transport_capabilities(&self) -> Result<TransportCapabilities, PeerLinkError> {
        self.inner
            .lock()
            .map(|link| link.transport_capabilities())
            .map_err(|_| PeerLinkError::Protocol)
    }

    pub fn maintenance(
        &self,
        contacts: &[ContactId],
        now: Timestamp,
    ) -> Result<PeerLinkReport, PeerLinkError> {
        self.inner.lock().map_err(|_| PeerLinkError::Protocol)?.maintenance(contacts, now)
    }

    pub fn close_idle_sessions(
        &self,
        retained: &[ContactId],
        now: Timestamp,
    ) -> Result<usize, PeerLinkError> {
        self.inner.lock().map_err(|_| PeerLinkError::Protocol)?.close_idle_sessions(retained, now)
    }

    pub fn next_maintenance_delay(&self, now: Timestamp) -> Option<Duration> {
        self.inner.lock().ok().and_then(|link| link.next_maintenance_delay(now))
    }

    pub fn network_changed(&self, now: Timestamp) -> Result<(), PeerLinkError> {
        self.inner.lock().map_err(|_| PeerLinkError::Protocol)?.network_changed(now);
        Ok(())
    }

    pub fn prime_connections(&self) -> Result<usize, PeerLinkError> {
        self.inner.lock().map_err(|_| PeerLinkError::Protocol)?.prime_connections()
    }

    pub fn prime_contact(&self, contact_id: ContactId) -> Result<bool, PeerLinkError> {
        self.inner.lock().map_err(|_| PeerLinkError::Protocol)?.prime_contact(contact_id)
    }

    pub fn connection_state(&self, contact_id: ContactId) -> PeerConnectionState {
        self.inner
            .lock()
            .map_or(PeerConnectionState::Failed, |link| link.connection_state(contact_id))
    }

    pub fn activity(&self) -> BTreeMap<ContactId, PeerActivitySnapshot> {
        self.inner.lock().map_or_else(|_| BTreeMap::new(), |link| link.activity())
    }

    pub fn send_and_wait_ack(
        &self,
        contact_id: ContactId,
        envelope_id: OpaqueId,
        message_kind: u16,
        ciphertext: Vec<u8>,
        timeout: Duration,
    ) -> Result<LinkAck, PeerLinkError> {
        self.send_and_wait_ack_with_limit(
            contact_id,
            envelope_id,
            message_kind,
            ciphertext,
            timeout,
            Duration::from_secs(5),
        )
    }

    pub fn send_envelope(
        &self,
        contact_id: ContactId,
        envelope_id: OpaqueId,
        message_kind: u16,
        ciphertext: Vec<u8>,
    ) -> Result<(), PeerLinkError> {
        self.inner.lock().map_err(|_| PeerLinkError::Protocol)?.send_envelope(
            contact_id,
            envelope_id,
            message_kind,
            ciphertext,
        )
    }

    pub fn send_envelopes_batch(
        &self,
        contact_id: ContactId,
        envelopes: Vec<(OpaqueId, u16, Vec<u8>)>,
    ) -> Result<(), PeerLinkError> {
        self.inner
            .lock()
            .map_err(|_| PeerLinkError::Protocol)?
            .send_envelopes_batch(contact_id, envelopes)
    }

    pub fn poll_envelope_ack(
        &self,
        contact_id: ContactId,
        envelope_id: OpaqueId,
    ) -> Result<Option<LinkAck>, PeerLinkError> {
        self.inner
            .lock()
            .map_err(|_| PeerLinkError::Protocol)?
            .poll_envelope_ack(contact_id, envelope_id)
    }

    pub fn send_and_wait_ack_with_limit(
        &self,
        contact_id: ContactId,
        envelope_id: OpaqueId,
        message_kind: u16,
        ciphertext: Vec<u8>,
        timeout: Duration,
        wait_limit: Duration,
    ) -> Result<LinkAck, PeerLinkError> {
        // Never hold the process-wide PeerLink mutex while waiting for I/O.
        // Send once, then perform short non-blocking polls with the mutex
        // released between each poll. This keeps lifecycle, Radio, delivery
        // and attachment control responsive when an ACK is missing.
        self.send_envelope(contact_id, envelope_id, message_kind, ciphertext)?;
        let deadline =
            Instant::now().checked_add(timeout.min(wait_limit)).ok_or(PeerLinkError::AckTimeout)?;
        loop {
            if let Some(ack) = self.poll_envelope_ack(contact_id, envelope_id)? {
                return Ok(ack);
            }
            let now = Instant::now();
            if now >= deadline {
                let millis = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_err(|_| PeerLinkError::AckTimeout)?
                    .as_millis();
                let timestamp = Timestamp::from_unix_millis(
                    i64::try_from(millis).map_err(|_| PeerLinkError::AckTimeout)?,
                )
                .map_err(|_| PeerLinkError::AckTimeout)?;
                let _ = self
                    .inner
                    .lock()
                    .map_err(|_| PeerLinkError::Protocol)?
                    .mark_ack_timeout(contact_id, timestamp);
                return Err(PeerLinkError::AckTimeout);
            }
            thread::sleep((deadline - now).min(Duration::from_millis(10)));
        }
    }

    pub fn send_keepalive_and_wait_ack(
        &self,
        contact_id: ContactId,
        envelope_id: OpaqueId,
        message_kind: u16,
        ciphertext: Vec<u8>,
        timeout: Duration,
    ) -> Result<LinkAck, PeerLinkError> {
        // Keepalive probes use the same non-blocking shared-lock path as
        // normal delivery.  The probe worker must never monopolise PeerLink
        // while waiting for a remote ACK.
        self.send_and_wait_ack_with_limit(
            contact_id,
            envelope_id,
            message_kind,
            ciphertext,
            timeout,
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
