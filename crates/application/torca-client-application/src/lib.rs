//! Public application boundary for commands, queries, projections and policy.

use std::collections::BTreeMap;

mod pending;
mod query;
mod runtime;
pub use pending::{
    InMemoryPendingOperationStore, PendingOperation, PendingOperationKind, PendingOperationStore,
    PendingOperationStoreError, pending_operation_id,
};
pub use query::{
    ApplicationQueryError, ApplicationReadModels, ContactSecuritySnapshot, ContactSecurityState,
    ConversationHistoryPort, ConversationMessagePage, ConversationMessageSummary,
    RuntimeSettingsPort, SecurityProjectionPort,
};
pub use runtime::{
    ApplicationCommand, ApplicationCommandResult, ApplicationError, ClientApplicationRuntime,
};

pub use torca_bootstrap::{BootstrapPhase, BootstrapSnapshot, BootstrapStepId, BootstrapStepState};
use torca_client_engine::EngineHandle;
pub use torca_client_engine::{ClientSnapshot, EngineCommand, EngineError, EngineResult};
use torca_contacts::ContactId;
use torca_foundation::OpaqueId;
use torca_identity::{fingerprint_for, safety_number};
pub use torca_probing::{ProbeStatus, ProbeTarget};
pub use torca_runtime::{
    AttachmentSendRequest, AttachmentView, NetworkSnapshot, RelayServiceInfo, RuntimeDriverError,
    RuntimeHandle, TorState, TransportActivitySnapshot,
};

/// Process-safe handle to the application consistency boundary.
#[derive(Clone)]
pub struct ClientApplicationHandle {
    engine: EngineHandle,
}

/// Application-owned input for the external projection. Contract and native
/// layers may serialize this context, but may not assemble domain state from
/// separate repositories.
#[derive(Clone, Debug)]
pub struct ApplicationSnapshotContext {
    pub application: ClientSnapshot,
    pub network: NetworkSnapshot,
    pub attachments: Vec<AttachmentView>,
    pub bootstrap: BootstrapSnapshot,
    pub identity_fingerprint: Option<String>,
    pub identity_fingerprints: BTreeMap<OpaqueId, String>,
    pub safety_numbers: BTreeMap<ContactId, String>,
    pub pending_operations: Vec<PendingOperation>,
}

impl ApplicationSnapshotContext {
    fn security_projection(
        snapshot: &ClientSnapshot,
    ) -> (Option<String>, BTreeMap<OpaqueId, String>, BTreeMap<ContactId, String>) {
        let local = snapshot.identity.as_ref().map(|identity| identity.public());
        let identity_fingerprint =
            local.map(|identity| fingerprint_for(identity.key().public_key()));
        let identity_fingerprints = snapshot
            .pairings
            .iter()
            .filter_map(|pairing| pairing.remote_proposal())
            .map(|proposal| &proposal.public_identity)
            .map(|identity| {
                (identity.identity_id().to_opaque(), fingerprint_for(identity.key().public_key()))
            })
            .collect();
        let safety_numbers = local.map_or_else(BTreeMap::new, |local| {
            snapshot
                .contacts
                .iter()
                .map(|contact| (contact.id(), safety_number(local, contact.remote_identity())))
                .collect()
        });
        (identity_fingerprint, identity_fingerprints, safety_numbers)
    }
}

impl ClientApplicationHandle {
    /// Wraps the single-writer engine behind the application boundary.
    pub const fn new(engine: EngineHandle) -> Self {
        Self { engine }
    }

    /// Dispatches one application command.
    pub fn dispatch(&self, command: EngineCommand) -> Result<EngineResult, EngineError> {
        self.engine.dispatch(command)
    }

    /// Returns the bounded application overview projection.
    pub fn overview(&self) -> Result<ClientSnapshot, EngineError> {
        self.engine.overview_snapshot()
    }

    /// Returns the full engine snapshot for targeted application queries.
    pub fn snapshot(&self) -> Result<ClientSnapshot, EngineError> {
        self.engine.snapshot()
    }

    /// Returns the actor handle for composition of infrastructure workers.
    pub fn engine_handle(&self) -> EngineHandle {
        self.engine.clone()
    }

    /// Local precondition for creating an invitation. The relay probe is not
    /// authoritative here: it is a separate stream and may be stale while the
    /// real pairing transport can reconnect or queue the operation. The
    /// operation itself remains responsible for returning a retryable result.
    pub fn pairing_creation_allowed(network: &NetworkSnapshot) -> Result<(), &'static str> {
        if network.tor == TorState::Ready { Ok(()) } else { Err("TOR_NOT_READY") }
    }

    /// Joining is an explicit connectivity attempt. An unknown or degraded
    /// relay probe must not prevent the user from submitting a valid code; the
    /// command itself returns the authoritative transport error. Tor readiness
    /// is the only safe local precondition.
    pub fn pairing_join_allowed(network: &NetworkSnapshot) -> Result<(), &'static str> {
        if network.tor == TorState::Ready { Ok(()) } else { Err("TOR_NOT_READY") }
    }

    pub fn profile_setup_allowed(network: &NetworkSnapshot) -> Result<(), &'static str> {
        if network.tor != TorState::Ready || network.onion_address.is_none() {
            return Err("PROFILE_NOT_READY");
        }
        match network
            .probes
            .iter()
            .find(|probe| probe.target == ProbeTarget::Relay)
            .map(|probe| probe.status)
        {
            Some(
                ProbeStatus::Healthy
                | ProbeStatus::Degraded
                | ProbeStatus::Failed
                | ProbeStatus::Unreachable,
            ) => Ok(()),
            _ => Err("PROFILE_NOT_READY"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ApplicationCommand, ApplicationError, ClientApplicationHandle, ClientApplicationRuntime,
    };
    use torca_client_engine::{ClientEngine, ClientEngineActor};
    use torca_foundation::ClassifiedError;

    #[test]
    fn facade_exposes_one_application_handle() {
        let (engine, actor) = ClientEngineActor::spawn(ClientEngine::default());
        let application = ClientApplicationHandle::new(engine);
        assert!(application.overview().is_ok());
        actor.shutdown().expect("actor shutdown");
    }

    #[test]
    fn application_errors_keep_machine_readable_descriptors() {
        let error = ApplicationError::from_message("RELAY_DEGRADED".into());
        assert_eq!(error.descriptor().code().as_str(), "relay.degraded");
    }

    #[test]
    fn profile_can_be_saved_before_network_runtime_is_attached() {
        let (engine, actor) = ClientEngineActor::spawn(ClientEngine::default());
        let application = ClientApplicationHandle::new(engine);
        let runtime = ClientApplicationRuntime::new(application.clone());
        runtime
            .bootstrap_identity("00000000000000000000000000000001".parse().expect("identity id"), 0)
            .expect("identity bootstrap");

        let result = runtime.execute(ApplicationCommand::UpdateProfile {
            display_name: "offline-user".into(),
            at_ms: 1,
        });

        assert!(result.is_ok());
        assert_eq!(
            application
                .overview()
                .expect("application overview")
                .identity
                .and_then(|identity| identity
                    .profile()
                    .map(|profile| profile.display_name().as_str().to_owned()))
                .as_deref(),
            Some("offline-user")
        );
        actor.shutdown().expect("actor shutdown");
    }

    #[test]
    fn pairing_command_is_queued_before_network_runtime_is_attached() {
        let (engine, actor) = ClientEngineActor::spawn(ClientEngine::default());
        let application = ClientApplicationHandle::new(engine);
        let runtime = ClientApplicationRuntime::new(application);
        let session_id = "00000000000000000000000000000002".parse().expect("session id");

        let result = runtime
            .execute(ApplicationCommand::CreatePairing { session_id })
            .expect("queue pairing");

        assert_eq!(result.kind, "pairing_queued");
        let snapshot = runtime.snapshot_context().expect("snapshot context");
        assert_eq!(snapshot.pending_operations.len(), 1);
        assert_eq!(snapshot.pending_operations[0].resource_id, session_id);
        actor.shutdown().expect("actor shutdown");
    }
}
