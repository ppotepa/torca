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
    Tor,
    OnionService,
    Relay,
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
            (BootstrapStepId::Tor, BootstrapCriticality::Required),
            (BootstrapStepId::OnionService, BootstrapCriticality::Required),
            (BootstrapStepId::Relay, BootstrapCriticality::DegradedAllowed),
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

    fn block_dependents(&mut self, failed: BootstrapStepId) {
        let blocked: BTreeSet<BootstrapStepId> = match failed {
            BootstrapStepId::Preferences => self.steps.keys().copied().collect(),
            BootstrapStepId::NativeBridge => [
                BootstrapStepId::Contract,
                BootstrapStepId::SecureStorage,
                BootstrapStepId::Database,
                BootstrapStepId::DeviceIdentity,
                BootstrapStepId::Tor,
                BootstrapStepId::OnionService,
                BootstrapStepId::Relay,
                BootstrapStepId::UserProfile,
            ]
            .into_iter()
            .collect(),
            BootstrapStepId::Contract => [
                BootstrapStepId::SecureStorage,
                BootstrapStepId::Database,
                BootstrapStepId::DeviceIdentity,
                BootstrapStepId::Tor,
                BootstrapStepId::OnionService,
                BootstrapStepId::Relay,
                BootstrapStepId::UserProfile,
            ]
            .into_iter()
            .collect(),
            BootstrapStepId::SecureStorage => [
                BootstrapStepId::Database,
                BootstrapStepId::DeviceIdentity,
                BootstrapStepId::Tor,
                BootstrapStepId::OnionService,
                BootstrapStepId::UserProfile,
            ]
            .into_iter()
            .collect(),
            BootstrapStepId::Database => [
                BootstrapStepId::DeviceIdentity,
                BootstrapStepId::Tor,
                BootstrapStepId::OnionService,
                BootstrapStepId::UserProfile,
            ]
            .into_iter()
            .collect(),
            BootstrapStepId::DeviceIdentity => {
                [BootstrapStepId::Tor, BootstrapStepId::OnionService, BootstrapStepId::UserProfile]
                    .into_iter()
                    .collect()
            }
            BootstrapStepId::Tor => {
                [BootstrapStepId::OnionService, BootstrapStepId::UserProfile].into_iter().collect()
            }
            BootstrapStepId::OnionService => [BootstrapStepId::UserProfile].into_iter().collect(),
            BootstrapStepId::Relay | BootstrapStepId::UserProfile => [].into_iter().collect(),
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
        let transport_ready = self
            .steps
            .get(&BootstrapStepId::OnionService)
            .is_some_and(|s| s.state == BootstrapStepState::Ready);
        let relay_finished = self.steps.get(&BootstrapStepId::Relay).is_some_and(|s| {
            matches!(
                s.state,
                BootstrapStepState::Ready
                    | BootstrapStepState::Degraded
                    | BootstrapStepState::Failed
            )
        });
        let profile_ready = self
            .steps
            .get(&BootstrapStepId::UserProfile)
            .is_some_and(|s| s.state == BootstrapStepState::Ready);
        let relay_degraded = self
            .steps
            .get(&BootstrapStepId::Relay)
            .is_some_and(|s| s.state == BootstrapStepState::Failed);
        let phase =
            if required_failed {
                BootstrapPhase::Failed
            } else if profile_ready {
                BootstrapPhase::Ready
            } else if identity_ready && transport_ready && relay_finished {
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
    use super::{BootstrapPhase, BootstrapState, BootstrapStepId};

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
            BootstrapStepId::Tor,
            BootstrapStepId::OnionService,
            BootstrapStepId::Relay,
        ] {
            state.complete(step);
        }
        assert_eq!(state.snapshot().phase, BootstrapPhase::ReadyForProfile);
    }

    #[test]
    fn failed_tor_blocks_profile() {
        let mut state = BootstrapState::new();
        state.fail(BootstrapStepId::Tor, "TOR_RUNTIME_FAILED");
        let snapshot = state.snapshot();
        assert_eq!(snapshot.phase, BootstrapPhase::Failed);
        assert!(snapshot.can_retry);
    }
}
