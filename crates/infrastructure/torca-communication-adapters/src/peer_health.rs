use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError};
use std::thread::{self, JoinHandle};
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

const PROBE_TIMEOUT: Duration = Duration::from_secs(4);

#[derive(Clone, Copy)]
struct HealthEntry {
    snapshot: PeerHealthSnapshot,
    previous_state: PeerConnectionState,
}

enum PeerProbeCommand {
    Probe {
        contact_id: ContactId,
        probe_id: OpaqueId,
        reported_rtt: u64,
        reply: SyncSender<Result<Duration, PeerLinkError>>,
    },
    Shutdown,
}

/// One durable executor for peer keepalives.  The health coordinator chooses
/// which contact is due; this worker only performs the bounded I/O.
struct PeerProbeWorker {
    commands: SyncSender<PeerProbeCommand>,
    worker: Option<JoinHandle<()>>,
}

impl PeerProbeWorker {
    fn spawn<S, K>(link: SharedPeerLink<S, K>) -> Result<Self, CommunicationError>
    where
        S: ContactRepository + PeerCredentialRepository + Send + 'static,
        K: HandshakeSigner + Send + 'static,
    {
        let (commands, receiver) = mpsc::sync_channel(1);
        let worker = thread::Builder::new()
            .name("torca-peer-probe".to_owned())
            .spawn(move || {
                while let Ok(command) = receiver.recv() {
                    match command {
                        PeerProbeCommand::Shutdown => break,
                        PeerProbeCommand::Probe { contact_id, probe_id, reported_rtt, reply } => {
                            let started = Instant::now();
                            let result = link
                                .send_keepalive_and_wait_ack(
                                    contact_id,
                                    probe_id,
                                    PROBE_MESSAGE_KIND,
                                    reported_rtt.to_be_bytes().to_vec(),
                                    PROBE_TIMEOUT,
                                )
                                .map(|_| started.elapsed());
                            let _ = reply.send(result);
                        }
                    }
                }
            })
            .map_err(|_| CommunicationError::Peer)?;
        Ok(Self { commands, worker: Some(worker) })
    }

    fn submit(&self, command: PeerProbeCommand) -> Result<(), CommunicationError> {
        self.commands.try_send(command).map_err(|_| CommunicationError::Peer)
    }

    fn shutdown(mut self) {
        let _ = self.commands.send(PeerProbeCommand::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

pub struct HealthPeerLinkAdapter<R, S, K> {
    relationships: R,
    link: SharedPeerLink<S, K>,
    local_identity_id: OpaqueId,
    health: BTreeMap<ContactId, HealthEntry>,
    probe_in_flight: Option<(ContactId, Receiver<Result<Duration, PeerLinkError>>)>,
    probe_worker: Option<PeerProbeWorker>,
}

impl<R, S, K> HealthPeerLinkAdapter<R, S, K>
where
    S: ContactRepository + PeerCredentialRepository + Send + 'static,
    K: HandshakeSigner + Send + 'static,
{
    pub fn new(
        link: SharedPeerLink<S, K>,
        relationships: R,
        local_identity_id: OpaqueId,
    ) -> Result<Self, CommunicationError> {
        let probe_worker = PeerProbeWorker::spawn(link.clone())?;
        Ok(Self {
            relationships,
            link,
            local_identity_id,
            health: BTreeMap::new(),
            probe_in_flight: None,
            probe_worker: Some(probe_worker),
        })
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
            entry.previous_state = state;
        }
    }

    fn should_initiate_probe(&self, contact_id: ContactId) -> bool {
        let Ok(Some(contact)) = self.relationships.get(contact_id) else { return false };
        self.local_identity_id.as_bytes()
            < contact.remote_identity().identity_id().to_opaque().as_bytes()
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
            }
            Err(_) => {
                entry.snapshot.consecutive_failures =
                    entry.snapshot.consecutive_failures.saturating_add(1);
                entry.snapshot.quality = classify_peer_health(
                    entry.snapshot.rtt_ms,
                    entry.snapshot.consecutive_failures,
                    entry.snapshot.last_success_at.and_then(|last| now.duration_since(last)),
                );
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

    fn network_changed(&mut self, now: Timestamp) {
        let _ = self.link.network_changed(now);
        for health in self.health.values_mut() {
            health.snapshot.reconnect_attempt = 0;
        }
    }

    fn set_waker(&mut self, waker: Arc<dyn Fn() + Send + Sync>) {
        let _ = self.link.set_waker(waker);
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

    fn peer_probe_eligible(&self, contact_id: ContactId) -> bool {
        self.should_initiate_probe(contact_id)
    }

    fn begin_probe(
        &mut self,
        contact_id: ContactId,
        probe_id: OpaqueId,
        reported_rtt: u64,
    ) -> Result<(), CommunicationError> {
        if self.probe_in_flight.is_some() {
            return Err(CommunicationError::Peer);
        }
        let Some(worker) = self.probe_worker.as_ref() else {
            return Err(CommunicationError::Peer);
        };
        let (sender, receiver) = mpsc::sync_channel(1);
        worker.submit(PeerProbeCommand::Probe {
            contact_id,
            probe_id,
            reported_rtt,
            reply: sender,
        })?;
        self.probe_in_flight = Some((contact_id, receiver));
        Ok(())
    }

    fn take_probe_completion(
        &mut self,
        now: Timestamp,
    ) -> Result<Option<ContactId>, CommunicationError> {
        if let Some((contact_id, receiver)) = self.probe_in_flight.as_ref() {
            match receiver.try_recv() {
                Ok(result) => {
                    let contact_id = *contact_id;
                    self.probe_in_flight = None;
                    self.record_probe_result(contact_id, now, result);
                    return Ok(Some(contact_id));
                }
                Err(TryRecvError::Disconnected) => {
                    let contact_id = *contact_id;
                    self.probe_in_flight = None;
                    self.record_probe_result(contact_id, now, Err(PeerLinkError::NotReady));
                    return Ok(Some(contact_id));
                }
                Err(TryRecvError::Empty) => return Ok(None),
            }
        }
        Ok(None)
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
        self.probe_in_flight = None;
        self.health.clear();
        if let Some(worker) = self.probe_worker.take() {
            worker.shutdown();
        }
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
