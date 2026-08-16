//! TorcaRuntime adapter for the completed pairing coordinator/runtime.

mod worker;
pub use worker::PairingWorkerDriver;

use std::collections::BTreeMap;
use std::time::Duration;

use torca_client_engine::EngineHandle;
use torca_crypto::{CryptoProvider, RustCryptoProvider};
use torca_foundation::{OpaqueId, Timestamp};
use torca_pairing::{PairingCode, PairingSessionId, PairingState};
use torca_pairing_coordinator::{
    LocalPairingContext, PairingApprovalPort, PairingCryptoPort, PairingPeerSecretStore,
    PairingPollReport, PairingRendezvousPort, PairingRuntime, PairingRuntimeError,
};
use torca_pairing_protocol::AvatarEnvelope;
use torca_runtime::{PairingDriver, PairingInvitationView, RuntimeDriverError};
use torca_tor::SharedTorEndpoint;

pub struct RuntimePairingDriver<R, C, A, S> {
    runtime: PairingRuntime<R, C, A, S>,
    engine: EngineHandle,
    tor_endpoint: SharedTorEndpoint,
    random: RustCryptoProvider,
    poll_schedule: BTreeMap<PairingSessionId, PairingPollSchedule>,
}

#[derive(Clone, Copy, Debug)]
struct PairingPollSchedule {
    next_at: Timestamp,
    consecutive_failures: u8,
}

const ACTIVE_POLL_INTERVAL: Duration = Duration::from_secs(1);
const IDLE_POLL_INTERVAL: Duration = Duration::from_secs(2);
const MAX_POLL_BACKOFF: Duration = Duration::from_secs(30);

impl<R, C, A, S> RuntimePairingDriver<R, C, A, S>
where
    R: PairingRendezvousPort,
    C: PairingCryptoPort,
    A: PairingApprovalPort,
    S: PairingPeerSecretStore,
{
    pub const fn new(
        runtime: PairingRuntime<R, C, A, S>,
        engine: EngineHandle,
        tor_endpoint: SharedTorEndpoint,
    ) -> Self {
        Self {
            runtime,
            engine,
            tor_endpoint,
            random: RustCryptoProvider,
            poll_schedule: BTreeMap::new(),
        }
    }

    fn context(&mut self) -> Result<LocalPairingContext, RuntimeDriverError> {
        let snapshot = self
            .engine
            .overview_snapshot()
            .map_err(|_| RuntimeDriverError::Engine)?;
        let identity = snapshot.identity.ok_or(RuntimeDriverError::Pairing)?;
        // The local onion endpoint is a readiness dependency, not a protocol
        // rejection. Keep it retryable so a cold Android Tor bootstrap does
        // not create a permanent pairing failure.
        let onion_address = self.tor_endpoint.get().ok_or(RuntimeDriverError::Tor)?;
        Ok(LocalPairingContext {
            display_name: identity.profile().map_or_else(
                || "Torca".to_owned(),
                |profile| profile.display_name().as_str().to_owned(),
            ),
            public_identity: identity.public().clone(),
            onion_address,
            capability_id: self.random_id()?,
            avatar: snapshot.avatar_genome.map(|record| AvatarEnvelope {
                schema: record.schema_version,
                generator_version: record.generator_version,
                catalog_version: record.catalog_version,
                genome_hash: record.genome_hash,
                compressed_genome: record.compressed_genome,
            }),
        })
    }

    fn publish_local_offer_if_ready(
        &mut self,
        session_id: PairingSessionId,
    ) -> Result<bool, RuntimeDriverError> {
        let context = match self.context() {
            Ok(context) => context,
            Err(RuntimeDriverError::Tor) => return Ok(false),
            Err(error) => return Err(error),
        };
        self.runtime
            .publish_local_offer(session_id, context)
            .map_err(map_pairing_error)?;
        Ok(true)
    }

    fn random_id(&mut self) -> Result<OpaqueId, RuntimeDriverError> {
        for _ in 0..8 {
            let mut bytes = [0_u8; 16];
            self.random
                .fill_random(&mut bytes)
                .map_err(|_| RuntimeDriverError::Pairing)?;
            let id = OpaqueId::from_bytes(bytes);
            if !id.is_nil() {
                return Ok(id);
            }
        }
        Err(RuntimeDriverError::Pairing)
    }

    fn active_sessions(&self) -> Result<Vec<PairingSessionId>, RuntimeDriverError> {
        self.engine
            .overview_snapshot()
            .map_err(|_| RuntimeDriverError::Pairing)
            .map(|snapshot| {
                snapshot
                    .pairings
                    .into_iter()
                    .filter(|s| {
                        !matches!(
                            s.state(),
                            PairingState::Rejected
                                | PairingState::Cancelled
                                | PairingState::Expired
                        )
                    })
                    .map(|s| s.id())
                    .collect()
            })
    }
}

impl<R, C, A, S> PairingDriver for RuntimePairingDriver<R, C, A, S>
where
    R: PairingRendezvousPort + Send + 'static,
    C: PairingCryptoPort + Send + 'static,
    A: PairingApprovalPort + Send + 'static,
    S: PairingPeerSecretStore + Send + 'static,
{
    fn create(
        &mut self,
        session_id: PairingSessionId,
        now: Timestamp,
    ) -> Result<PairingInvitationView, RuntimeDriverError> {
        let invitation = self
            .runtime
            .create_invitation_pending_route(session_id, now)
            .map_err(map_pairing_error)?;
        let _ = self.publish_local_offer_if_ready(session_id)?;
        self.schedule_now(session_id, now);
        Ok(PairingInvitationView {
            session_id: invitation.session_id,
            code: invitation.code,
            uri: invitation.uri,
            expires_at: invitation.expires_at,
        })
    }

    fn join(
        &mut self,
        session_id: PairingSessionId,
        code: PairingCode,
        ticket: Option<[u8; 16]>,
        now: Timestamp,
    ) -> Result<(), RuntimeDriverError> {
        self.runtime
            .join_invitation_pending_route(session_id, code, ticket)
            .map_err(map_pairing_error)?;
        let _ = self.publish_local_offer_if_ready(session_id)?;
        self.schedule_now(session_id, now);
        Ok(())
    }

    fn approve(
        &mut self,
        session_id: PairingSessionId,
        now: Timestamp,
    ) -> Result<(), RuntimeDriverError> {
        self.runtime
            .approve(session_id, now)
            .map_err(map_pairing_error)?;
        self.schedule_now(session_id, now);
        Ok(())
    }

    fn reject(&mut self, session_id: PairingSessionId) -> Result<(), RuntimeDriverError> {
        self.runtime.reject(session_id).map_err(map_pairing_error)
    }

    fn cancel(&mut self, session_id: PairingSessionId) -> Result<(), RuntimeDriverError> {
        self.runtime.cancel(session_id).map_err(map_pairing_error)
    }

    fn maintenance(&mut self, now: Timestamp) -> Result<(), RuntimeDriverError> {
        self.runtime
            .maintenance(now)
            .map_err(|_| RuntimeDriverError::Pairing)?;
        let active_sessions = self.active_sessions()?;
        self.poll_schedule
            .retain(|id, _| active_sessions.contains(id));
        for session_id in active_sessions {
            if self
                .poll_schedule
                .get(&session_id)
                .is_some_and(|state| now < state.next_at)
            {
                continue;
            }
            if !self.publish_local_offer_if_ready(session_id)? {
                self.schedule_failure(session_id, now);
                continue;
            }
            match self.runtime.poll(session_id, now) {
                Ok(report) => {
                    let had_activity = report != PairingPollReport::default();
                    self.schedule_success(session_id, now, had_activity);
                }
                Err(torca_pairing_coordinator::PairingRuntimeError::SessionNotFound) => {
                    self.poll_schedule.remove(&session_id);
                }
                Err(_) => self.schedule_failure(session_id, now),
            }
        }
        Ok(())
    }

    fn next_maintenance_delay(&self, now: Timestamp) -> Option<Duration> {
        self.poll_schedule
            .values()
            .map(|schedule| schedule.next_at.duration_since(now).unwrap_or_default())
            .min()
    }

    fn network_changed(&mut self, now: Timestamp) {
        self.runtime.network_changed();
        for session_id in self.poll_schedule.keys().copied().collect::<Vec<_>>() {
            self.schedule_now(session_id, now);
        }
    }

    fn shutdown(&mut self) {
        if let Ok(sessions) = self.active_sessions() {
            for id in sessions {
                let _ = self.runtime.close_transport(id);
            }
        }
    }
}

impl<R, C, A, S> RuntimePairingDriver<R, C, A, S> {
    fn schedule_now(&mut self, session_id: PairingSessionId, now: Timestamp) {
        self.poll_schedule.insert(
            session_id,
            PairingPollSchedule {
                next_at: now,
                consecutive_failures: 0,
            },
        );
    }

    fn schedule_success(&mut self, session_id: PairingSessionId, now: Timestamp, active: bool) {
        let delay = if active {
            ACTIVE_POLL_INTERVAL
        } else {
            IDLE_POLL_INTERVAL
        };
        let next_at = now.checked_add(delay).unwrap_or(now);
        self.poll_schedule.insert(
            session_id,
            PairingPollSchedule {
                next_at,
                consecutive_failures: 0,
            },
        );
    }

    fn schedule_failure(&mut self, session_id: PairingSessionId, now: Timestamp) {
        let failures = self
            .poll_schedule
            .get(&session_id)
            .map_or(1, |state| state.consecutive_failures.saturating_add(1));
        let exponent = u32::from(failures.saturating_sub(1).min(5));
        let delay = Duration::from_secs(1_u64 << exponent).min(MAX_POLL_BACKOFF);
        let next_at = now.checked_add(delay).unwrap_or(now);
        self.poll_schedule.insert(
            session_id,
            PairingPollSchedule {
                next_at,
                consecutive_failures: failures,
            },
        );
    }
}

fn map_pairing_error(error: PairingRuntimeError) -> RuntimeDriverError {
    match error {
        // A rendezvous transport failure is transient and must participate in
        // the supervisor backoff loop. Protocol/session errors are terminal
        // for the current invitation and must not be retried forever.
        PairingRuntimeError::Coordinator(
            torca_pairing_coordinator::PairingCoordinatorError::Rendezvous,
        ) => RuntimeDriverError::Communication,
        PairingRuntimeError::SessionNotFound => RuntimeDriverError::Pairing,
        _ => RuntimeDriverError::Pairing,
    }
}
