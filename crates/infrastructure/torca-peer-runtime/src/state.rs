use std::collections::BTreeMap;
use std::time::Duration;

use torca_contacts::{ContactId, ContactRepository, PeerCredentialRepository};
use torca_crypto::{CryptoProvider, RustCryptoProvider};
use torca_foundation::Timestamp;
use torca_peer::{PeerSessionState, PeerTransport};
use torca_peer_protocol::HandshakeSigner;

use crate::core::{PeerRuntime, PeerRuntimeError};

const RECONNECT_BASE_MS: u64 = 1_000;
const RECONNECT_MAX_MS: u64 = 60_000;

/// One presentation-independent connection state per contact, hiding socket direction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PeerConnectionState {
    Disconnected,
    Connecting,
    Handshaking,
    Ready,
    Reconnecting,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ReconnectEntry {
    failures: u32,
    next_attempt_at: Timestamp,
    in_progress: bool,
}

#[must_use]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ReconnectReport {
    pub scheduled: usize,
    pub attempted: usize,
    pub started: usize,
    pub reset: usize,
}

/// Single owner of reconnect timing. It never stores messages or ciphertext; durable delivery stays
/// entirely in the outbox worker.
pub struct ReconnectSupervisor {
    entries: BTreeMap<ContactId, ReconnectEntry>,
    random: RustCryptoProvider,
}

impl Default for ReconnectSupervisor {
    fn default() -> Self {
        Self { entries: BTreeMap::new(), random: RustCryptoProvider }
    }
}

impl ReconnectSupervisor {
    /// Advances reconnect state for the contact IDs supplied by the application snapshot.
    pub fn maintenance<S, K>(
        &mut self,
        runtime: &mut PeerRuntime<S, K>,
        contacts: &[ContactId],
        now: Timestamp,
    ) -> Result<ReconnectReport, PeerRuntimeError>
    where
        S: ContactRepository + PeerCredentialRepository,
        K: HandshakeSigner,
    {
        let mut report = ReconnectReport::default();
        for &contact_id in contacts {
            match runtime.connection_state(contact_id) {
                PeerConnectionState::Ready => {
                    if self.entries.remove(&contact_id).is_some() {
                        report.reset += 1;
                    }
                }
                PeerConnectionState::Connecting | PeerConnectionState::Handshaking => {
                    if let Some(entry) = self.entries.get_mut(&contact_id) {
                        entry.in_progress = true;
                    }
                }
                PeerConnectionState::Disconnected
                | PeerConnectionState::Reconnecting
                | PeerConnectionState::Failed => {
                    let should_schedule = self
                        .entries
                        .get(&contact_id)
                        .is_none_or(|entry| entry.in_progress);
                    if should_schedule {
                        self.schedule(contact_id, now)?;
                        report.scheduled += 1;
                    }
                }
            }
        }

        let due: Vec<_> = self
            .entries
            .iter()
            .filter_map(|(contact_id, entry)| {
                (!entry.in_progress && entry.next_attempt_at <= now).then_some(*contact_id)
            })
            .collect();
        for contact_id in due {
            self.remove_non_ready(runtime, contact_id);
            report.attempted += 1;
            match runtime.ensure_connected(contact_id, now) {
                Ok(true) => {
                    if let Some(entry) = self.entries.get_mut(&contact_id) {
                        entry.in_progress = true;
                    }
                    report.started += 1;
                }
                Ok(false) => {
                    if runtime.connection_state(contact_id) == PeerConnectionState::Ready {
                        self.entries.remove(&contact_id);
                        report.reset += 1;
                    }
                }
                Err(_) => {
                    self.schedule(contact_id, now)?;
                }
            }
        }
        Ok(report)
    }

    pub fn clear(&mut self, contact_id: ContactId) {
        self.entries.remove(&contact_id);
    }

    pub fn clear_all(&mut self) {
        self.entries.clear();
    }

    fn schedule(
        &mut self,
        contact_id: ContactId,
        now: Timestamp,
    ) -> Result<(), PeerRuntimeError> {
        let failures = self
            .entries
            .get(&contact_id)
            .map_or(1, |entry| entry.failures.saturating_add(1));
        let delay = self.delay(failures)?;
        let next_attempt_at = now.checked_add(delay).ok_or(PeerRuntimeError::Protocol)?;
        self.entries.insert(
            contact_id,
            ReconnectEntry { failures, next_attempt_at, in_progress: false },
        );
        Ok(())
    }

    fn delay(&mut self, failures: u32) -> Result<Duration, PeerRuntimeError> {
        let exponent = failures.saturating_sub(1).min(16);
        let base = RECONNECT_BASE_MS
            .saturating_mul(1_u64 << exponent)
            .min(RECONNECT_MAX_MS);
        let jitter_room = (base / 4).min(RECONNECT_MAX_MS.saturating_sub(base));
        let jitter = if jitter_room == 0 {
            0
        } else {
            let mut random = [0_u8; 8];
            self.random
                .fill_random(&mut random)
                .map_err(|_| PeerRuntimeError::Randomness)?;
            u64::from_le_bytes(random) % (jitter_room + 1)
        };
        Ok(Duration::from_millis(base + jitter))
    }

    fn remove_non_ready<S, K>(&self, runtime: &mut PeerRuntime<S, K>, contact_id: ContactId)
    where
        S: ContactRepository + PeerCredentialRepository,
        K: HandshakeSigner,
    {
        if runtime
            .outgoing
            .get(&contact_id)
            .is_some_and(|session| session.state() != PeerSessionState::Ready)
        {
            if let Some(mut session) = runtime.outgoing.remove(&contact_id) {
                let _ = session.close();
            }
        }
        if runtime
            .incoming
            .get(&contact_id)
            .is_some_and(|session| session.state() != PeerSessionState::Ready)
        {
            if let Some(mut session) = runtime.incoming.remove(&contact_id) {
                let _ = session.close();
            }
        }
    }
}

impl<S, K> PeerRuntime<S, K>
where
    S: ContactRepository + PeerCredentialRepository,
    K: HandshakeSigner,
{
    pub fn connection_state(&self, contact_id: ContactId) -> PeerConnectionState {
        if let Some(session) = self.outgoing.get(&contact_id) {
            return map_state(session.state());
        }
        if let Some(session) = self.incoming.get(&contact_id) {
            return map_state(session.state());
        }
        PeerConnectionState::Disconnected
    }

    pub fn is_ready(&self, contact_id: ContactId) -> bool {
        self.connection_state(contact_id) == PeerConnectionState::Ready
    }
}

const fn map_state(state: PeerSessionState) -> PeerConnectionState {
    match state {
        PeerSessionState::Disconnected => PeerConnectionState::Disconnected,
        PeerSessionState::Connecting => PeerConnectionState::Connecting,
        PeerSessionState::Handshaking => PeerConnectionState::Handshaking,
        PeerSessionState::Ready => PeerConnectionState::Ready,
        PeerSessionState::Reconnecting => PeerConnectionState::Reconnecting,
        PeerSessionState::Closed | PeerSessionState::Failed => PeerConnectionState::Failed,
    }
}
