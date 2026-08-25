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
    PairingPollReport, PairingRuntime, PairingRuntimeError, PairingSessionServicePort,
};
use torca_pairing_protocol::{AvatarEnvelope, PairingBootstrapDescriptor};
use torca_runtime::{PairingDriver, PairingInvitationView, RuntimeDriverError};

/// Provider-owned local endpoint advertised in a pairing offer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairingTransportRoute {
    pub transport_provider: String,
    pub transport_endpoint: Vec<u8>,
}

impl PairingTransportRoute {
    pub fn new(provider: impl Into<String>, endpoint: Vec<u8>) -> Self {
        Self { transport_provider: provider.into(), transport_endpoint: endpoint }
    }
}

/// Reads the endpoint from the currently selected provider. Returning `None`
/// means commissioning has not produced a routable endpoint yet.
pub trait PairingTransportRouteSource: Send + Sync {
    fn local_route(&self) -> Option<PairingTransportRoute>;
}

impl<F> PairingTransportRouteSource for F
where
    F: Fn() -> Option<PairingTransportRoute> + Send + Sync,
{
    fn local_route(&self) -> Option<PairingTransportRoute> {
        self()
    }
}

/// Supplies optional provider-specific data embedded in a newly-created QR
/// invitation. It is only discovery/signaling material; a completed contact
/// always takes its durable route from the authenticated pairing offer.
pub trait PairingBootstrapSource: Send + Sync {
    fn invitation_bootstrap(
        &self,
    ) -> Result<Option<PairingBootstrapDescriptor>, RuntimeDriverError>;
}

impl<F> PairingBootstrapSource for F
where
    F: Fn() -> Result<Option<PairingBootstrapDescriptor>, RuntimeDriverError> + Send + Sync,
{
    fn invitation_bootstrap(
        &self,
    ) -> Result<Option<PairingBootstrapDescriptor>, RuntimeDriverError> {
        self()
    }
}

pub struct RuntimePairingDriver<R, C, A, S> {
    runtime: PairingRuntime<R, C, A, S>,
    engine: EngineHandle,
    route_source: Box<dyn PairingTransportRouteSource>,
    bootstrap_source: Option<Box<dyn PairingBootstrapSource>>,
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
    R: PairingSessionServicePort,
    C: PairingCryptoPort,
    A: PairingApprovalPort,
    S: PairingPeerSecretStore,
{
    pub fn new(
        runtime: PairingRuntime<R, C, A, S>,
        engine: EngineHandle,
        route_source: Box<dyn PairingTransportRouteSource>,
    ) -> Self {
        Self {
            runtime,
            engine,
            route_source,
            bootstrap_source: None,
            random: RustCryptoProvider,
            poll_schedule: BTreeMap::new(),
        }
    }

    pub fn with_bootstrap_source(mut self, source: Box<dyn PairingBootstrapSource>) -> Self {
        self.bootstrap_source = Some(source);
        self
    }

    fn context(&mut self) -> Result<Option<LocalPairingContext>, RuntimeDriverError> {
        let snapshot = self.engine.overview_snapshot().map_err(|_| RuntimeDriverError::Engine)?;
        let identity = snapshot.identity.ok_or(RuntimeDriverError::Pairing)?;
        let Some(route) = self.route_source.local_route() else {
            return Ok(None);
        };
        Ok(Some(LocalPairingContext {
            display_name: identity.profile().map_or_else(
                || "Torca".to_owned(),
                |profile| profile.display_name().as_str().to_owned(),
            ),
            public_identity: identity.public().clone(),
            capability_id: self.random_id()?,
            avatar: snapshot.avatar_genome.map(|record| AvatarEnvelope {
                schema: record.schema_version,
                generator_version: record.generator_version,
                catalog_version: record.catalog_version,
                genome_hash: record.genome_hash,
                compressed_genome: record.compressed_genome,
            }),
            transport_provider: route.transport_provider,
            transport_endpoint: route.transport_endpoint,
        }))
    }

    fn publish_local_offer_if_ready(
        &mut self,
        session_id: PairingSessionId,
    ) -> Result<bool, RuntimeDriverError> {
        let Some(context) = self.context()? else {
            return Ok(false);
        };
        self.runtime.publish_local_offer(session_id, context).map_err(map_pairing_error)?;
        Ok(true)
    }

    fn random_id(&mut self) -> Result<OpaqueId, RuntimeDriverError> {
        for _ in 0..8 {
            let mut bytes = [0_u8; 16];
            self.random.fill_random(&mut bytes).map_err(|_| RuntimeDriverError::Pairing)?;
            let id = OpaqueId::from_bytes(bytes);
            if !id.is_nil() {
                return Ok(id);
            }
        }
        Err(RuntimeDriverError::Pairing)
    }

    fn active_sessions(&self) -> Result<Vec<PairingSessionId>, RuntimeDriverError> {
        self.engine.overview_snapshot().map_err(|_| RuntimeDriverError::Pairing).map(|snapshot| {
            snapshot
                .pairings
                .into_iter()
                .filter(|s| {
                    !matches!(
                        s.state(),
                        PairingState::Rejected | PairingState::Cancelled | PairingState::Expired
                    )
                })
                .map(|s| s.id())
                .collect()
        })
    }
}

impl<R, C, A, S> PairingDriver for RuntimePairingDriver<R, C, A, S>
where
    R: PairingSessionServicePort + Send + 'static,
    C: PairingCryptoPort + Send + 'static,
    A: PairingApprovalPort + Send + 'static,
    S: PairingPeerSecretStore + Send + 'static,
{
    fn create(
        &mut self,
        session_id: PairingSessionId,
        now: Timestamp,
    ) -> Result<PairingInvitationView, RuntimeDriverError> {
        // A creator can open and persist the provider-owned pairing slot
        // before its advertised route is available.  This is important for
        // direct providers such as Iroh: endpoint discovery may complete a
        // moment after the local runtime is ready.  Keep the invitation alive
        // with its code/ticket and let maintenance publish the local offer;
        // the contract projection will add the provider bootstrap as soon as
        // it becomes available.  A real provider error still fails the
        // command, while `Pending` is deliberately non-fatal for creators.
        let bootstrap = match self.bootstrap_source.as_ref() {
            Some(source) => match source.invitation_bootstrap() {
                Ok(value) => value,
                Err(RuntimeDriverError::Pending) => None,
                Err(error) => return Err(error),
            },
            None => None,
        };
        let invitation = self
            .runtime
            .create_invitation_pending_route_with_bootstrap(session_id, now, bootstrap.as_ref())
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
        bootstrap: Option<PairingBootstrapDescriptor>,
        now: Timestamp,
    ) -> Result<(), RuntimeDriverError> {
        self.runtime
            .join_invitation_pending_route_with_bootstrap(
                session_id,
                code,
                ticket,
                bootstrap.as_ref(),
            )
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
        self.runtime.approve(session_id, now).map_err(map_pairing_error)?;
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
        self.runtime.maintenance(now).map_err(|_| RuntimeDriverError::Pairing)?;
        let active_sessions = self.active_sessions()?;
        self.poll_schedule.retain(|id, _| active_sessions.contains(id));
        for session_id in active_sessions {
            if self.poll_schedule.get(&session_id).is_some_and(|state| now < state.next_at) {
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
        self.poll_schedule
            .insert(session_id, PairingPollSchedule { next_at: now, consecutive_failures: 0 });
    }

    fn schedule_success(&mut self, session_id: PairingSessionId, now: Timestamp, active: bool) {
        let delay = if active { ACTIVE_POLL_INTERVAL } else { IDLE_POLL_INTERVAL };
        let next_at = now.checked_add(delay).unwrap_or(now);
        self.poll_schedule
            .insert(session_id, PairingPollSchedule { next_at, consecutive_failures: 0 });
    }

    fn schedule_failure(&mut self, session_id: PairingSessionId, now: Timestamp) {
        let failures = self
            .poll_schedule
            .get(&session_id)
            .map_or(1, |state| state.consecutive_failures.saturating_add(1));
        let exponent = u32::from(failures.saturating_sub(1).min(5));
        let delay = Duration::from_secs(1_u64 << exponent).min(MAX_POLL_BACKOFF);
        let next_at = now.checked_add(delay).unwrap_or(now);
        self.poll_schedule
            .insert(session_id, PairingPollSchedule { next_at, consecutive_failures: failures });
    }
}

fn map_pairing_error(error: PairingRuntimeError) -> RuntimeDriverError {
    match error {
        PairingRuntimeError::Coordinator(
            torca_pairing_coordinator::PairingCoordinatorError::BootstrapMissing,
        ) => RuntimeDriverError::Classified(torca_foundation::ErrorDescriptor::new(
            torca_foundation::ErrorCode::new("pairing.bootstrap_missing"),
            torca_foundation::ErrorCategory::InvalidInput,
            torca_foundation::RetryAdvice::Never,
        )),
        PairingRuntimeError::Coordinator(
            torca_pairing_coordinator::PairingCoordinatorError::BootstrapProviderMismatch,
        ) => RuntimeDriverError::Classified(torca_foundation::ErrorDescriptor::new(
            torca_foundation::ErrorCode::new("pairing.provider_mismatch"),
            torca_foundation::ErrorCategory::Conflict,
            torca_foundation::RetryAdvice::Never,
        )),
        PairingRuntimeError::Coordinator(
            torca_pairing_coordinator::PairingCoordinatorError::BootstrapInvalid,
        ) => RuntimeDriverError::Classified(torca_foundation::ErrorDescriptor::new(
            torca_foundation::ErrorCode::new("pairing.bootstrap_invalid"),
            torca_foundation::ErrorCategory::InvalidInput,
            torca_foundation::RetryAdvice::Never,
        )),
        // A rendezvous transport failure is transient and must participate in
        // the supervisor backoff loop. Protocol/session errors are terminal
        // for the current invitation and must not be retried forever.
        PairingRuntimeError::Coordinator(
            torca_pairing_coordinator::PairingCoordinatorError::SessionService,
        ) => RuntimeDriverError::Communication,
        PairingRuntimeError::SessionNotFound => RuntimeDriverError::Pairing,
        _ => RuntimeDriverError::Pairing,
    }
}
