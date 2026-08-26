//! Deterministic startup state model shared by Android and Windows.
//!
//! Execution remains in platform/runtime adapters. This crate owns ordering,
//! dependency blocking, retry policy and the presentation-safe snapshot.

use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum BootstrapStepId {
    Preferences,
    NativeBridge,
    Contract,
    SecureStorage,
    Database,
    DeviceIdentity,
    CommunicationRuntime,
    IncomingReachability,
    Rendezvous,
    UserProfile,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BootstrapStepState {
    Pending,
    Running,
    Verifying,
    Ready,
    Degraded,
    Failed,
    Blocked,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BootstrapPhase {
    Idle,
    Starting,
    ReadyForProfile,
    Ready,
    Degraded,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BootstrapCriticality {
    Required,
    DegradedAllowed,
    Optional,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootstrapStep {
    pub id: BootstrapStepId,
    pub state: BootstrapStepState,
    pub criticality: BootstrapCriticality,
    pub diagnostic_code: Option<String>,
    pub attempt: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootstrapSnapshot {
    pub generation: u64,
    pub phase: BootstrapPhase,
    pub steps: Vec<BootstrapStep>,
    pub can_retry: bool,
    pub can_reset: bool,
}

#[derive(Default)]
pub struct BootstrapState {
    generation: u64,
    steps: BTreeMap<BootstrapStepId, BootstrapStep>,
}

impl BootstrapState {
    #[must_use]
    pub fn new() -> Self {
        let mut state = Self::default();
        for (id, criticality) in [
            (BootstrapStepId::Preferences, BootstrapCriticality::Required),
            (BootstrapStepId::NativeBridge, BootstrapCriticality::Required),
            (BootstrapStepId::Contract, BootstrapCriticality::Required),
            (BootstrapStepId::SecureStorage, BootstrapCriticality::Required),
            (BootstrapStepId::Database, BootstrapCriticality::Required),
            (BootstrapStepId::DeviceIdentity, BootstrapCriticality::Required),
            (BootstrapStepId::CommunicationRuntime, BootstrapCriticality::Required),
            (BootstrapStepId::IncomingReachability, BootstrapCriticality::Required),
            (BootstrapStepId::Rendezvous, BootstrapCriticality::DegradedAllowed),
            (BootstrapStepId::UserProfile, BootstrapCriticality::Required),
        ] {
            state.steps.insert(
                id,
                BootstrapStep {
                    id,
                    state: BootstrapStepState::Pending,
                    criticality,
                    diagnostic_code: None,
                    attempt: 0,
                },
            );
        }
        state
    }

    /// Applies provider-owned commissioning requirements to the generic
    /// startup graph.  An onion publisher is one possible implementation of
    /// incoming reachability, not a universal startup dependency.
    pub fn configure_communication_requirements(
        &mut self,
        commissioning: &torca_transport_api::ProviderCommissioning,
    ) {
        if let Some(step) = self.steps.get_mut(&BootstrapStepId::IncomingReachability) {
            step.criticality = if commissioning.requires_for_local_shell(
                torca_transport_api::CommissioningStage::IncomingReachability,
            ) {
                BootstrapCriticality::Required
            } else {
                BootstrapCriticality::DegradedAllowed
            };
        }
        if let Some(step) = self.steps.get_mut(&BootstrapStepId::Rendezvous) {
            step.criticality = if commissioning.requires_for_local_shell(
                torca_transport_api::CommissioningStage::PairingRendezvous,
            ) {
                BootstrapCriticality::Required
            } else {
                BootstrapCriticality::DegradedAllowed
            };
        }
    }

    pub fn begin(&mut self, id: BootstrapStepId) {
        self.generation = self.generation.saturating_add(1);
        if let Some(step) = self.steps.get_mut(&id) {
            step.state = BootstrapStepState::Running;
            step.attempt = step.attempt.saturating_add(1);
            step.diagnostic_code = None;
        }
    }

    pub fn verify(&mut self, id: BootstrapStepId) {
        if let Some(step) = self.steps.get_mut(&id) {
            step.state = BootstrapStepState::Verifying;
        }
    }

    pub fn complete(&mut self, id: BootstrapStepId) {
        if let Some(step) = self.steps.get_mut(&id) {
            step.state = BootstrapStepState::Ready;
            step.diagnostic_code = None;
        }
    }

    pub fn fail(&mut self, id: BootstrapStepId, code: impl Into<String>) {
        if let Some(step) = self.steps.get_mut(&id) {
            step.state = BootstrapStepState::Failed;
            step.diagnostic_code = Some(code.into());
        }
        self.block_dependents(id);
    }

    /// Records a non-blocking degradation. Degraded steps remain eligible for
    /// recovery and do not block profile setup or the main application.
    pub fn degrade(&mut self, id: BootstrapStepId, code: impl Into<String>) {
        if let Some(step) = self.steps.get_mut(&id) {
            step.state = BootstrapStepState::Degraded;
            step.diagnostic_code = Some(code.into());
        }
    }

    fn block_dependents(&mut self, failed: BootstrapStepId) {
        let blocked: BTreeSet<BootstrapStepId> = match failed {
            BootstrapStepId::Preferences => self.steps.keys().copied().collect(),
            BootstrapStepId::NativeBridge => [
                BootstrapStepId::Contract,
                BootstrapStepId::SecureStorage,
                BootstrapStepId::Database,
                BootstrapStepId::DeviceIdentity,
                BootstrapStepId::CommunicationRuntime,
                BootstrapStepId::IncomingReachability,
                BootstrapStepId::Rendezvous,
                BootstrapStepId::UserProfile,
            ]
            .into_iter()
            .collect(),
            BootstrapStepId::Contract => [
                BootstrapStepId::SecureStorage,
                BootstrapStepId::Database,
                BootstrapStepId::DeviceIdentity,
                BootstrapStepId::CommunicationRuntime,
                BootstrapStepId::IncomingReachability,
                BootstrapStepId::Rendezvous,
                BootstrapStepId::UserProfile,
            ]
            .into_iter()
            .collect(),
            BootstrapStepId::SecureStorage => [
                BootstrapStepId::Database,
                BootstrapStepId::DeviceIdentity,
                BootstrapStepId::CommunicationRuntime,
                BootstrapStepId::IncomingReachability,
                BootstrapStepId::UserProfile,
            ]
            .into_iter()
            .collect(),
            BootstrapStepId::Database => [
                BootstrapStepId::DeviceIdentity,
                BootstrapStepId::CommunicationRuntime,
                BootstrapStepId::IncomingReachability,
                BootstrapStepId::UserProfile,
            ]
            .into_iter()
            .collect(),
            BootstrapStepId::DeviceIdentity => [
                BootstrapStepId::CommunicationRuntime,
                BootstrapStepId::IncomingReachability,
                BootstrapStepId::UserProfile,
            ]
            .into_iter()
            .collect(),
            BootstrapStepId::CommunicationRuntime => {
                [BootstrapStepId::IncomingReachability, BootstrapStepId::UserProfile]
                    .into_iter()
                    .collect()
            }
            BootstrapStepId::IncomingReachability => {
                [BootstrapStepId::UserProfile].into_iter().collect()
            }
            BootstrapStepId::Rendezvous | BootstrapStepId::UserProfile => [].into_iter().collect(),
        };
        for id in blocked {
            if let Some(step) = self.steps.get_mut(&id)
                && step.state == BootstrapStepState::Pending
            {
                step.state = BootstrapStepState::Blocked;
            }
        }
    }

    #[must_use]
    pub fn snapshot(&self) -> BootstrapSnapshot {
        let required_failed = self.steps.values().any(|step| {
            step.criticality == BootstrapCriticality::Required
                && matches!(step.state, BootstrapStepState::Failed | BootstrapStepState::Blocked)
        });
        let identity_ready = self
            .steps
            .get(&BootstrapStepId::DeviceIdentity)
            .is_some_and(|s| s.state == BootstrapStepState::Ready);
        let transport_ready =
            self.steps.get(&BootstrapStepId::IncomingReachability).is_some_and(|step| {
                step.state == BootstrapStepState::Ready
                    || step.criticality != BootstrapCriticality::Required
            });
        let profile_ready = self
            .steps
            .get(&BootstrapStepId::UserProfile)
            .is_some_and(|s| s.state == BootstrapStepState::Ready);
        let relay_degraded = self.steps.get(&BootstrapStepId::Rendezvous).is_some_and(|s| {
            matches!(s.state, BootstrapStepState::Failed | BootstrapStepState::Degraded)
        });
        let phase =
            if required_failed {
                BootstrapPhase::Failed
            } else if profile_ready {
                BootstrapPhase::Ready
            // Provider-owned incoming reachability is a recoverable,
            // demand-driven capability. Requiring its first probe here creates
            // a circular dependency: the provider sleeps until pairing demand
            // exists, while pairing UI is hidden until this phase completes.
            // Local identity plus the communication runtime are sufficient to
            // expose profile setup and the application shell; each provider
            // decides separately when incoming reachability is required.
            } else if identity_ready && transport_ready {
                BootstrapPhase::ReadyForProfile
            } else if relay_degraded {
                BootstrapPhase::Degraded
            } else if self.steps.values().any(|s| {
                matches!(s.state, BootstrapStepState::Running | BootstrapStepState::Verifying)
            }) {
                BootstrapPhase::Starting
            } else {
                BootstrapPhase::Idle
            };
        BootstrapSnapshot {
            generation: self.generation,
            phase,
            steps: self.steps.values().cloned().collect(),
            can_retry: required_failed || phase == BootstrapPhase::Starting,
            can_reset: required_failed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{BootstrapPhase, BootstrapState, BootstrapStepId, BootstrapStepState};

    #[test]
    fn identity_and_transport_unlock_profile_setup() {
        let mut state = BootstrapState::new();
        for step in [
            BootstrapStepId::Preferences,
            BootstrapStepId::NativeBridge,
            BootstrapStepId::Contract,
            BootstrapStepId::SecureStorage,
            BootstrapStepId::Database,
            BootstrapStepId::DeviceIdentity,
            BootstrapStepId::CommunicationRuntime,
            BootstrapStepId::IncomingReachability,
            BootstrapStepId::Rendezvous,
        ] {
            state.complete(step);
        }
        assert_eq!(state.snapshot().phase, BootstrapPhase::ReadyForProfile);
    }

    #[test]
    fn direct_provider_does_not_block_profile_on_incoming_reachability() {
        let mut state = BootstrapState::new();
        let commissioning = torca_transport_api::TransportKind::Iroh.deployment_profile();
        // A direct provider has no managed commissioning service. Its local
        // endpoint may still be publishing while the application shell is
        // already safe to expose.
        let projection = torca_transport_api::ProviderCommissioning {
            provider: torca_transport_api::TransportKind::Iroh,
            steps: vec![
                torca_transport_api::CommissioningStep {
                    stage: torca_transport_api::CommissioningStage::LocalRuntime,
                    state: torca_transport_api::CommissioningState::Ready,
                    required_for_local_shell: true,
                    required_for_pairing: true,
                },
                torca_transport_api::CommissioningStep {
                    stage: torca_transport_api::CommissioningStage::IncomingReachability,
                    state: torca_transport_api::CommissioningState::Pending,
                    required_for_local_shell: false,
                    required_for_pairing: false,
                },
            ],
            endpoint_summary: None,
            route_state: torca_transport_api::ProviderRouteState::Unavailable,
            pairing_bootstrap: None,
        };
        assert!(!commissioning.requires_service_readiness());
        state.configure_communication_requirements(&projection);
        for step in [
            BootstrapStepId::Preferences,
            BootstrapStepId::NativeBridge,
            BootstrapStepId::Contract,
            BootstrapStepId::SecureStorage,
            BootstrapStepId::Database,
            BootstrapStepId::DeviceIdentity,
            BootstrapStepId::CommunicationRuntime,
            BootstrapStepId::IncomingReachability,
            BootstrapStepId::Rendezvous,
        ] {
            state.complete(step);
        }
        assert_eq!(state.snapshot().phase, BootstrapPhase::ReadyForProfile);
    }

    #[test]
    fn failed_communication_runtime_blocks_profile() {
        let mut state = BootstrapState::new();
        state.fail(BootstrapStepId::CommunicationRuntime, "COMMUNICATION_RUNTIME_FAILED");
        let snapshot = state.snapshot();
        assert_eq!(snapshot.phase, BootstrapPhase::Failed);
        assert!(snapshot.can_retry);
    }

    #[test]
    fn degraded_relay_keeps_profile_route_available() {
        let mut state = BootstrapState::new();
        for step in [
            BootstrapStepId::Preferences,
            BootstrapStepId::NativeBridge,
            BootstrapStepId::Contract,
            BootstrapStepId::SecureStorage,
            BootstrapStepId::Database,
            BootstrapStepId::DeviceIdentity,
            BootstrapStepId::CommunicationRuntime,
            BootstrapStepId::IncomingReachability,
        ] {
            state.complete(step);
        }
        state.begin(BootstrapStepId::Rendezvous);
        state.degrade(BootstrapStepId::Rendezvous, "RENDEZVOUS_UNREACHABLE");
        let snapshot = state.snapshot();
        assert_eq!(snapshot.phase, BootstrapPhase::ReadyForProfile);
        assert_eq!(
            snapshot
                .steps
                .iter()
                .find(|step| step.id == BootstrapStepId::Rendezvous)
                .map(|step| step.state),
            Some(BootstrapStepState::Degraded)
        );
    }

    #[test]
    fn checking_relay_does_not_deadlock_profile_setup() {
        let mut state = BootstrapState::new();
        for step in [
            BootstrapStepId::Preferences,
            BootstrapStepId::NativeBridge,
            BootstrapStepId::Contract,
            BootstrapStepId::SecureStorage,
            BootstrapStepId::Database,
            BootstrapStepId::DeviceIdentity,
            BootstrapStepId::CommunicationRuntime,
            BootstrapStepId::IncomingReachability,
        ] {
            state.complete(step);
        }
        state.begin(BootstrapStepId::Rendezvous);
        state.verify(BootstrapStepId::Rendezvous);

        assert_eq!(state.snapshot().phase, BootstrapPhase::ReadyForProfile);
    }

    #[test]
    fn provider_without_incoming_requirement_can_unlock_local_profile() {
        let mut state = BootstrapState::new();
        let commissioning = torca_transport_api::ProviderCommissioning {
            provider: torca_transport_api::TransportKind::Iroh,
            steps: vec![torca_transport_api::CommissioningStep {
                stage: torca_transport_api::CommissioningStage::LocalRuntime,
                state: torca_transport_api::CommissioningState::Ready,
                required_for_local_shell: true,
                required_for_pairing: true,
            }],
            endpoint_summary: Some("iroh:test".into()),
            route_state: torca_transport_api::ProviderRouteState::Fresh,
            pairing_bootstrap: None,
        };
        state.configure_communication_requirements(&commissioning);
        for step in [
            BootstrapStepId::Preferences,
            BootstrapStepId::NativeBridge,
            BootstrapStepId::Contract,
            BootstrapStepId::SecureStorage,
            BootstrapStepId::Database,
            BootstrapStepId::DeviceIdentity,
            BootstrapStepId::CommunicationRuntime,
        ] {
            state.complete(step);
        }
        assert_eq!(state.snapshot().phase, BootstrapPhase::ReadyForProfile);
    }
}
