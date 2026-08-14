use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Configuration {
    #[default]
    Debug,
    Release,
}

impl fmt::Display for Configuration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Debug => "debug",
            Self::Release => "release",
        })
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Target {
    Windows,
    Android,
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Windows => "windows",
            Self::Android => "android",
        })
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeployAction {
    RunInstalled,
    RedeployCurrent,
    Rebuild,
    FullRedeploy,
    RelayMaintenance,
    CollectLogs,
    BuildArtifacts,
}

impl fmt::Display for DeployAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::RunInstalled => "run installed clients",
            Self::RedeployCurrent => "redeploy current artifacts",
            Self::Rebuild => "rebuild",
            Self::FullRedeploy => "full redeploy",
            Self::RelayMaintenance => "relay maintenance",
            Self::CollectLogs => "collect logs",
            Self::BuildArtifacts => "build artifacts",
        })
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildPolicy {
    Reuse,
    IfRequired,
    Rebuild,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OnionPolicy {
    Ensure,
    Restart,
    RepairDirectoryCache,
    RotateIdentity,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClientDataPolicy {
    Preserve,
    ResetProfile,
    ResetAll,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationLevel {
    Skip,
    Quick,
    Full,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LaunchPolicy {
    Skip,
    Start,
    Restart,
}

/// Controls Android's OS-level screen capture protection for a deployment.
/// Strict is the safe default; AllowCapture is an explicit local-development
/// opt-out and does not change Torca transport or message privacy.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PrivacyPolicy {
    #[default]
    Strict,
    AllowCapture,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DeployPlan {
    pub action: DeployAction,
    pub targets: Vec<Target>,
    pub configuration: Configuration,
    pub client_build: BuildPolicy,
    pub relay_build: BuildPolicy,
    pub onion: OnionPolicy,
    pub client_data: ClientDataPolicy,
    pub validation: ValidationLevel,
    pub launch: LaunchPolicy,
    #[serde(default)]
    pub privacy: PrivacyPolicy,
}

impl DeployPlan {
    pub fn normal(
        action: DeployAction,
        targets: Vec<Target>,
        configuration: Configuration,
    ) -> Self {
        Self {
            action,
            targets,
            configuration,
            client_build: BuildPolicy::IfRequired,
            relay_build: BuildPolicy::IfRequired,
            onion: OnionPolicy::Ensure,
            client_data: ClientDataPolicy::Preserve,
            validation: ValidationLevel::Quick,
            launch: LaunchPolicy::Restart,
            privacy: PrivacyPolicy::Strict,
        }
    }

    pub fn validate(&self) -> Result<(), PlanError> {
        if self.targets.is_empty() && !matches!(self.action, DeployAction::RelayMaintenance) {
            return Err(PlanError::NoTargets);
        }
        if self.onion == OnionPolicy::RotateIdentity {
            if self.client_build != BuildPolicy::Rebuild || self.relay_build != BuildPolicy::Rebuild
            {
                return Err(PlanError::RotationRequiresRebuild);
            }
            if self.targets.len() != 2
                || !self.targets.contains(&Target::Windows)
                || !self.targets.contains(&Target::Android)
            {
                return Err(PlanError::RotationRequiresAllTargets);
            }
        }
        Ok(())
    }

    /// Applies invariants which are implied by the selected operation.  Keeping
    /// these defaults in the domain means the TUI and the CLI cannot drift.
    pub fn normalized(mut self) -> Self {
        if self.action == DeployAction::RunInstalled {
            self.client_build = BuildPolicy::Reuse;
            self.relay_build = BuildPolicy::Reuse;
            self.client_data = ClientDataPolicy::Preserve;
            self.launch = LaunchPolicy::Restart;
        }
        if self.action == DeployAction::CollectLogs {
            self.client_build = BuildPolicy::Reuse;
            self.relay_build = BuildPolicy::Reuse;
            self.client_data = ClientDataPolicy::Preserve;
            self.launch = LaunchPolicy::Skip;
        }
        if self.action == DeployAction::BuildArtifacts {
            self.relay_build = BuildPolicy::Reuse;
            self.client_data = ClientDataPolicy::Preserve;
            self.launch = LaunchPolicy::Skip;
        }
        if self.action == DeployAction::Rebuild {
            self.client_build = BuildPolicy::Rebuild;
            self.relay_build = BuildPolicy::Rebuild;
        }
        if self.action == DeployAction::FullRedeploy {
            self.client_build = BuildPolicy::Rebuild;
            self.relay_build = BuildPolicy::Rebuild;
        }
        if self.action == DeployAction::RelayMaintenance {
            self.client_data = ClientDataPolicy::Preserve;
            self.launch = LaunchPolicy::Skip;
        }
        if self.onion == OnionPolicy::RotateIdentity {
            self.action = DeployAction::FullRedeploy;
            self.targets = vec![Target::Windows, Target::Android];
            self.client_build = BuildPolicy::Rebuild;
            self.relay_build = BuildPolicy::Rebuild;
        }
        if self.action == DeployAction::FullRedeploy
            && self.client_data == ClientDataPolicy::Preserve
        {
            self.client_data = ClientDataPolicy::ResetProfile;
        }
        self
    }

    pub fn needs_relay(&self) -> bool {
        !matches!(
            self.action,
            DeployAction::RunInstalled | DeployAction::CollectLogs | DeployAction::BuildArtifacts
        )
    }
}

#[derive(Debug, Error)]
pub enum PlanError {
    #[error("a client deployment requires at least one target")]
    NoTargets,
    #[error("rotating the relay onion requires relay and client rebuilds")]
    RotationRequiresRebuild,
    #[error("rotating the relay onion requires Windows and Android to be selected")]
    RotationRequiresAllTargets,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeployStage {
    Planned,
    RelayPrepared,
    RelayReachable,
    EndpointVerified,
    ArtifactsBuilt,
    ClientDataReset,
    ClientsInstalled,
    ClientsLaunched,
    NetworkReady,
    Completed,
    Interrupted,
    Failed,
}

impl DeployStage {
    pub const fn terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DeployRun {
    pub schema: u32,
    pub run_id: String,
    pub started_at_ms: u128,
    pub plan: DeployPlan,
    pub stage: DeployStage,
    pub relay_endpoint: Option<String>,
    pub completed: Vec<DeployStage>,
    pub message: Option<String>,
}

impl DeployRun {
    pub fn new(plan: DeployPlan) -> Self {
        let started_at_ms =
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis();
        Self {
            schema: 1,
            run_id: format!("{started_at_ms:x}-{}", std::process::id()),
            started_at_ms,
            plan,
            stage: DeployStage::Planned,
            relay_endpoint: None,
            completed: Vec::new(),
            message: None,
        }
    }

    pub fn advance(&mut self, stage: DeployStage, message: impl Into<String>) {
        if !self.completed.contains(&stage) {
            self.completed.push(stage);
        }
        self.stage = stage;
        self.message = Some(message.into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn onion_rotation_requires_every_rebuild_and_target() {
        let mut plan = DeployPlan::normal(
            DeployAction::FullRedeploy,
            vec![Target::Windows],
            Configuration::Debug,
        );
        plan.onion = OnionPolicy::RotateIdentity;
        assert!(matches!(plan.validate(), Err(PlanError::RotationRequiresRebuild)));
        plan.client_build = BuildPolicy::Rebuild;
        plan.relay_build = BuildPolicy::Rebuild;
        assert!(matches!(plan.validate(), Err(PlanError::RotationRequiresAllTargets)));
        plan.targets.push(Target::Android);
        assert!(plan.validate().is_ok());
    }

    #[test]
    fn non_mutating_actions_cannot_reset_client_data() {
        let mut plan = DeployPlan::normal(
            DeployAction::RunInstalled,
            vec![Target::Windows],
            Configuration::Debug,
        );
        plan.client_data = ClientDataPolicy::ResetAll;
        assert_eq!(plan.normalized().client_data, ClientDataPolicy::Preserve);
    }

    #[test]
    fn privacy_is_strict_by_default_and_is_preserved_by_normalization() {
        let plan = DeployPlan::normal(
            DeployAction::RunInstalled,
            vec![Target::Android],
            Configuration::Debug,
        );
        assert_eq!(plan.privacy, PrivacyPolicy::Strict);
        let mut relaxed = plan;
        relaxed.privacy = PrivacyPolicy::AllowCapture;
        assert_eq!(relaxed.normalized().privacy, PrivacyPolicy::AllowCapture);
    }
}
