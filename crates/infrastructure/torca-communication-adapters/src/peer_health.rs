use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use crate::{application_envelope, application_peer_state};
use torca_communication_driver::{
    CommunicationError, InboundEnvelope, PROBE_MESSAGE_KIND, PeerHealthQuality, PeerHealthSnapshot,
    PeerLinkRuntime, classify_peer_health,
};
use torca_contacts::{ContactId, ContactRepository, PeerCredentialRepository};
use torca_foundation::{OpaqueId, Timestamp};
use torca_peer_link::{PeerConnectionState, PeerLinkError};
use torca_peer_protocol::{AckStatus, HandshakeSigner};
use torca_peer_shared::SharedPeerLink;
use torca_runtime::PeerConnectionStatus;

const PROBE_INTERVAL: Duration = Duration::from_secs(30);
const PROBE_RETRY: Duration = Duration::from_secs(10);
const PROBE_TIMEOUT: Duration = Duration::from_secs(4);

#[derive(Clone, Copy)]
struct HealthEntry {
    snapshot: PeerHealthSnapshot,
    previous_state: PeerConnectionState,
    next_probe_at: Timestamp,
}

pub struct HealthPeerLinkAdapter<R, S, K> {
    relationships: R,
    link: SharedPeerLink<S, K>,
    local_identity_id: OpaqueId,
    health: BTreeMap<ContactId, HealthEntry>,
    probe_sequence: u128,
}

impl<R, S, K> HealthPeerLinkAdapter<R, S, K> {
    pub fn new(link: SharedPeerLink<S, K>, relationships: R, local_identity_id: OpaqueId) -> Self {
        Self { relationships, link, local_identity_id, health: BTreeMap::new(), probe_sequence: 1 }
    }
}

impl<R, S, K> HealthPeerLinkAdapter<R, S, K>
where
    R: ContactRepository,
    S: ContactRepository + PeerCredentialRepository,
    K: HandshakeSigner,
{
    fn refresh_states(&mut self, contacts: &[ContactId], now: Timestamp) {
        self.health.retain(|contact_id, _| contacts.contains(contact_id));
        for contact_id in contacts {
            let state = self.link.connection_state(*contact_id);
            let entry = self.health.entry(*contact_id).or_insert_with(|| HealthEntry {
                snapshot: PeerHealthSnapshot::from_connection_state(map_peer_state(state)),
                previous_state: state,
                next_probe_at: now,
            });
            if entry.previous_state == PeerConnectionState::Ready
                && state != PeerConnectionState::Ready
            {
                entry.snapshot.reconnect_attempt =
                    entry.snapshot.reconnect_attempt.saturating_add(1);
            }
            entry.snapshot.state = map_peer_state(state);
            if state != PeerConnectionState::Ready {
                entry.snapshot.quality = if entry.snapshot.consecutive_failures > 0 {
                    PeerHealthQuality::Poor
                } else {
                    PeerHealthQuality::Unknown
                };
            } else if let Some(last) = entry.snapshot.last_success_at {
                entry.snapshot.quality = classify_peer_health(
                    entry.snapshot.rtt_ms,
                    entry.snapshot.consecutive_failures,
                    now.duration_since(last),
                );
            }
            if state == PeerConnectionState::Ready
                && entry.previous_state != PeerConnectionState::Ready
            {
                entry.next_probe_at = now;
            }
            entry.previous_state = state;
        }
    }

    fn should_initiate_probe(&self, contact_id: ContactId) -> bool {
        let Ok(Some(contact)) = self.relationships.get(contact_id) else { return false };
        self.local_identity_id.as_bytes()
            < contact.remote_identity().identity_id().to_opaque().as_bytes()
    }

    fn next_probe(&self, now: Timestamp) -> Option<ContactId> {
        self.health.iter().find_map(|(contact_id, entry)| {
            (entry.snapshot.state == PeerConnectionStatus::Ready
                && now >= entry.next_probe_at
                && self.should_initiate_probe(*contact_id))
            .then_some(*contact_id)
        })
    }

    fn record_probe_result(
        &mut self,
        contact_id: ContactId,
        now: Timestamp,
        result: Result<Duration, PeerLinkError>,
    ) {
        let state = self.link.connection_state(contact_id);
        let Some(entry) = self.health.get_mut(&contact_id) else { return };
        entry.snapshot.state = map_peer_state(state);
        match result {
            Ok(elapsed) => {
                let rtt_ms = u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX);
                entry.snapshot.rtt_ms = Some(rtt_ms);
                entry.snapshot.last_success_at = Some(now);
                entry.snapshot.consecutive_failures = 0;
                entry.snapshot.quality =
                    classify_peer_health(Some(rtt_ms), 0, Some(Duration::ZERO));
                entry.next_probe_at = now.checked_add(PROBE_INTERVAL).unwrap_or(now);
            }
            Err(_) => {
                entry.snapshot.consecutive_failures =
                    entry.snapshot.consecutive_failures.saturating_add(1);
                entry.snapshot.quality = classify_peer_health(
                    entry.snapshot.rtt_ms,
                    entry.snapshot.consecutive_failures,
                    entry.snapshot.last_success_at.and_then(|last| now.duration_since(last)),
                );
                entry.next_probe_at = now.checked_add(PROBE_RETRY).unwrap_or(now);
            }
        }
    }
}

impl<R, S, K> PeerLinkRuntime for HealthPeerLinkAdapter<R, S, K>
where
    R: ContactRepository + Send + 'static,
    S: ContactRepository + PeerCredentialRepository + Send + 'static,
    K: HandshakeSigner + Send + 'static,
{
    fn maintenance(
        &mut self,
        contacts: &[ContactId],
        now: Timestamp,
    ) -> Result<(), CommunicationError> {
        let _ = self.link.maintenance(contacts, now).map_err(|_| CommunicationError::Peer)?;
        self.refresh_states(contacts, now);
        Ok(())
    }

    fn connection_state(&self, contact_id: ContactId) -> PeerConnectionStatus {
        application_peer_state(self.link.connection_state(contact_id))
    }

    fn take_inbound(&mut self) -> Result<Option<InboundEnvelope>, CommunicationError> {
        self.link
            .take_inbound()
            .map_err(|_| CommunicationError::Peer)
            .map(|envelope| envelope.map(application_envelope))
    }

    fn reject(&mut self, envelope: &InboundEnvelope) -> Result<(), CommunicationError> {
        self.link
            .send_ack(envelope.contact_id, envelope.envelope_id, AckStatus::Rejected)
            .map_err(|_| CommunicationError::Peer)
    }

    fn peer_health(&self, contact_id: ContactId) -> PeerHealthSnapshot {
        self.health.get(&contact_id).map_or_else(
            || {
                PeerHealthSnapshot::from_connection_state(map_peer_state(
                    self.link.connection_state(contact_id),
                ))
            },
            |entry| entry.snapshot,
        )
    }

    fn probe_due(&mut self, now: Timestamp) -> Result<(), CommunicationError> {
        let Some(contact_id) = self.next_probe(now) else { return Ok(()) };
        let reported_rtt = self
            .health
            .get(&contact_id)
            .and_then(|entry| entry.snapshot.rtt_ms)
            .unwrap_or(u64::MAX);
        let probe_id = OpaqueId::from_u128(self.probe_sequence);
        self.probe_sequence = self.probe_sequence.saturating_add(1).max(1);
        let started = Instant::now();
        let result = self
            .link
            .send_keepalive_and_wait_ack(
                contact_id,
                probe_id,
                PROBE_MESSAGE_KIND,
                reported_rtt.to_be_bytes().to_vec(),
                PROBE_TIMEOUT,
            )
            .map(|_| started.elapsed());
        self.record_probe_result(contact_id, now, result);
        Ok(())
    }

    fn accept_probe(
        &mut self,
        envelope: &InboundEnvelope,
        now: Timestamp,
    ) -> Result<(), CommunicationError> {
        if envelope.ciphertext.len() != 8 {
            return self
                .link
                .send_ack(envelope.contact_id, envelope.envelope_id, AckStatus::Rejected)
                .map_err(|_| CommunicationError::Peer);
        }
        let reported = u64::from_be_bytes(
            envelope.ciphertext.as_slice().try_into().map_err(|_| CommunicationError::Peer)?,
        );
        let state = self.link.connection_state(envelope.contact_id);
        let entry = self.health.entry(envelope.contact_id).or_insert_with(|| HealthEntry {
            snapshot: PeerHealthSnapshot::from_connection_state(map_peer_state(state)),
            previous_state: state,
            next_probe_at: now.checked_add(PROBE_INTERVAL).unwrap_or(now),
        });
        entry.snapshot.state = map_peer_state(state);
        entry.snapshot.last_success_at = Some(now);
        entry.snapshot.consecutive_failures = 0;
        if reported != u64::MAX {
            entry.snapshot.rtt_ms = Some(reported);
            entry.snapshot.quality = classify_peer_health(Some(reported), 0, Some(Duration::ZERO));
        }
        self.link
            .send_ack(envelope.contact_id, envelope.envelope_id, AckStatus::Accepted)
            .map_err(|_| CommunicationError::Peer)
    }

    fn shutdown(&mut self) {
        self.health.clear();
        self.link.shutdown();
    }
}

const fn map_peer_state(state: PeerConnectionState) -> PeerConnectionStatus {
    match state {
        PeerConnectionState::Disconnected => PeerConnectionStatus::Disconnected,
        PeerConnectionState::Connecting => PeerConnectionStatus::Connecting,
        PeerConnectionState::Handshaking => PeerConnectionStatus::Handshaking,
        PeerConnectionState::Ready => PeerConnectionStatus::Ready,
        PeerConnectionState::Reconnecting => PeerConnectionStatus::Reconnecting,
        PeerConnectionState::Failed => PeerConnectionStatus::Failed,
    }
}
