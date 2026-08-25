use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};
pub use torca_transport_api::TransportKind as CommunicationProvider;

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
    #[serde(alias = "relay_maintenance")]
    ProviderMaintenance,
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
            Self::ProviderMaintenance => "provider maintenance",
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

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
/// Provider-owned maintenance requested for a managed commissioning service.
///
/// The concrete provider interprets the operation.  For Tor this currently
/// maps to relay/onion maintenance; direct providers may reject unsupported
/// operations instead of inheriting Tor semantics.
pub enum ProviderMaintenancePolicy {
    #[default]
    Ensure,
    Restart,
    RepairDirectoryCache,
    RotateIdentity,
}

/// Source-compatible name for older PowerShell adapters. New Rust code must
/// use [`ProviderMaintenancePolicy`]. The persisted field migration below
/// accepts the old `onion` key as well.
pub type OnionPolicy = ProviderMaintenancePolicy;

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
    /// Optional exact host/device id for deterministic deployments.
    #[serde(default)]
    pub device: Option<String>,
    pub configuration: Configuration,
    pub client_build: BuildPolicy,
    #[serde(alias = "relay_build")]
    pub provider_service_build: BuildPolicy,
    #[serde(default, alias = "onion")]
    pub provider_maintenance: ProviderMaintenancePolicy,
    pub client_data: ClientDataPolicy,
    pub validation: ValidationLevel,
    pub launch: LaunchPolicy,
    #[serde(default)]
    pub privacy: PrivacyPolicy,
    /// Exactly one provider is selected for new sessions by each deployment.
    #[serde(default)]
    pub communication_provider: CommunicationProvider,
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
            device: None,
            configuration,
            client_build: BuildPolicy::IfRequired,
            provider_service_build: BuildPolicy::IfRequired,
            provider_maintenance: ProviderMaintenancePolicy::Ensure,
            client_data: ClientDataPolicy::Preserve,
            validation: ValidationLevel::Quick,
            launch: LaunchPolicy::Restart,
            privacy: PrivacyPolicy::Strict,
            communication_provider: CommunicationProvider::Tor,
        }
    }

    pub fn validate(&self) -> Result<(), PlanError> {
        if self.targets.is_empty() && !matches!(self.action, DeployAction::ProviderMaintenance) {
            return Err(PlanError::NoTargets);
        }
        if self.provider_maintenance == ProviderMaintenancePolicy::RotateIdentity {
            if self.communication_provider != CommunicationProvider::Tor {
                return Err(PlanError::UnsupportedProviderMaintenance {
                    provider: self.communication_provider,
                });
            }
            if self.client_build != BuildPolicy::Rebuild
                || self.provider_service_build != BuildPolicy::Rebuild
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
        if !self.communication_provider.deployment_profile().is_deployment_ready() {
            return Err(PlanError::ProviderNotReady(self.communication_provider));
        }
        Ok(())
    }

    /// Applies invariants which are implied by the selected operation.  Keeping
    /// these defaults in the domain means the TUI and the CLI cannot drift.
    pub fn normalized(mut self) -> Self {
        if self.action == DeployAction::RunInstalled {
            self.client_build = BuildPolicy::Reuse;
            self.provider_service_build = BuildPolicy::Reuse;
            self.client_data = ClientDataPolicy::Preserve;
            self.launch = LaunchPolicy::Restart;
        }
        if self.action == DeployAction::CollectLogs {
            self.client_build = BuildPolicy::Reuse;
            self.provider_service_build = BuildPolicy::Reuse;
            self.client_data = ClientDataPolicy::Preserve;
            self.launch = LaunchPolicy::Skip;
        }
        if self.action == DeployAction::BuildArtifacts {
            self.provider_service_build = BuildPolicy::Reuse;
            self.client_data = ClientDataPolicy::Preserve;
            self.launch = LaunchPolicy::Skip;
        }
        if self.action == DeployAction::Rebuild {
            self.client_build = BuildPolicy::Rebuild;
            self.provider_service_build = BuildPolicy::Rebuild;
        }
        if self.action == DeployAction::FullRedeploy {
            self.client_build = BuildPolicy::Rebuild;
            self.provider_service_build = BuildPolicy::Rebuild;
        }
        if self.action == DeployAction::ProviderMaintenance {
            self.client_data = ClientDataPolicy::Preserve;
            self.launch = LaunchPolicy::Skip;
        }
        if self.provider_maintenance == ProviderMaintenancePolicy::RotateIdentity {
            self.action = DeployAction::FullRedeploy;
            self.targets = vec![Target::Windows, Target::Android];
            self.client_build = BuildPolicy::Rebuild;
            self.provider_service_build = BuildPolicy::Rebuild;
        }
        if self.action == DeployAction::FullRedeploy
            && self.client_data == ClientDataPolicy::Preserve
        {
            self.client_data = ClientDataPolicy::ResetProfile;
        }
        self
    }

    pub fn needs_provider_service(&self) -> bool {
        self.communication_provider.deployment_profile().commissioning_service.is_managed()
            && !matches!(
                self.action,
                DeployAction::RunInstalled
                    | DeployAction::CollectLogs
                    | DeployAction::BuildArtifacts
            )
    }

    /// Compatibility alias for older callers. New deploy orchestration must
    /// use the provider-neutral name because Iroh/WebRTC do not necessarily
    /// have a relay service.
    #[deprecated(note = "use needs_provider_service")]
    pub fn needs_relay(&self) -> bool {
        self.needs_provider_service()
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
    #[error("communication provider '{0}' is not ready for deployment")]
    ProviderNotReady(CommunicationProvider),
    #[error("provider '{provider}' does not support the selected maintenance action")]
    UnsupportedProviderMaintenance { provider: CommunicationProvider },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeployStage {
    Planned,
    #[serde(alias = "relay_prepared")]
    ProviderServicePrepared,
    #[serde(alias = "relay_reachable")]
    ProviderServiceReachable,
    #[serde(alias = "endpoint_verified")]
    ProviderEndpointVerified,
    ArtifactsBuilt,
    ClientDataReset,
    ClientsInstalled,
    ClientsLaunched,
    RuntimeReady,
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
    #[serde(alias = "relay_endpoint")]
    pub provider_endpoint: Option<String>,
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
            provider_endpoint: None,
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
        plan.provider_maintenance = ProviderMaintenancePolicy::RotateIdentity;
        assert!(matches!(plan.validate(), Err(PlanError::RotationRequiresRebuild)));
        plan.client_build = BuildPolicy::Rebuild;
        plan.provider_service_build = BuildPolicy::Rebuild;
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

    #[test]
    fn incomplete_provider_is_rejected_before_deployment() {
        let mut plan = DeployPlan::normal(
            DeployAction::RedeployCurrent,
            vec![Target::Android],
            Configuration::Debug,
        );
        // Iroh is a validated direct provider; WebRTC still requires a host
        // session/signaling binding and must be rejected until those exist.
        plan.communication_provider = CommunicationProvider::WebRtc;
        assert!(matches!(plan.validate(), Err(PlanError::ProviderNotReady(_))));
    }

    #[test]
    fn relay_requirement_is_provider_metadata_not_an_action_assumption() {
        let plan = DeployPlan::normal(
            DeployAction::RedeployCurrent,
            vec![Target::Android],
            Configuration::Debug,
        );
        assert!(plan.needs_provider_service());

        let mut direct = plan;
        direct.communication_provider = CommunicationProvider::WebRtc;
        assert!(!direct.needs_provider_service());
    }

    #[test]
    fn persisted_tor_named_fields_migrate_to_provider_neutral_plan_fields() {
        let plan = DeployPlan::normal(
            DeployAction::RedeployCurrent,
            vec![Target::Android],
            Configuration::Debug,
        );
        let mut value = serde_json::to_value(plan).expect("serialize deployment plan");
        let object = value.as_object_mut().expect("plan object");
        let maintenance =
            object.remove("provider_maintenance").expect("provider maintenance field");
        object.insert("onion".into(), maintenance);
        let service_build =
            object.remove("provider_service_build").expect("provider service build field");
        object.insert("relay_build".into(), service_build);

        let migrated: DeployPlan = serde_json::from_value(value).expect("deserialize old plan");
        assert_eq!(migrated.provider_maintenance, ProviderMaintenancePolicy::Ensure);
        assert_eq!(migrated.provider_service_build, BuildPolicy::IfRequired);
    }

    #[test]
    fn legacy_checkpoint_action_and_stage_names_remain_readable() {
        assert_eq!(
            serde_json::from_str::<DeployAction>("\"relay_maintenance\"")
                .expect("legacy deployment action"),
            DeployAction::ProviderMaintenance
        );
        assert_eq!(
            serde_json::from_str::<DeployStage>("\"relay_reachable\"")
                .expect("legacy deployment stage"),
            DeployStage::ProviderServiceReachable
        );
    }
}
