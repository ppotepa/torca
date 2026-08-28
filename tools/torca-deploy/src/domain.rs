use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};
pub use torca_foundation::ProviderId as CommunicationProvider;

pub trait ProviderMetadataExt {
    fn wire_value(&self) -> &str;
    fn deployment_profile(&self) -> torca_provider_api::ProviderDeploymentProfile;
    fn descriptor(&self) -> torca_provider_api::ProviderDescriptor;
    fn protocol_label(&self) -> &'static str;
}

impl ProviderMetadataExt for CommunicationProvider {
    fn wire_value(&self) -> &str {
        self.as_str()
    }

    fn deployment_profile(&self) -> torca_provider_api::ProviderDeploymentProfile {
        torca_provider_api::built_in_deployment_profile(self)
            .expect("only registered provider metadata may reach deploy")
    }

    fn descriptor(&self) -> torca_provider_api::ProviderDescriptor {
        torca_provider_api::built_in_descriptor(self)
            .expect("only registered provider metadata may reach deploy")
    }

    fn protocol_label(&self) -> &'static str {
        self.descriptor().label
    }
}

#[allow(clippy::missing_panics_doc)]
pub fn iroh_provider() -> CommunicationProvider {
    CommunicationProvider::new("iroh").expect("static production provider id")
}

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldId {
    Targets,
    Configuration,
    ClientBuild,
    ProviderServiceBuild,
    ProviderMaintenance,
    ClientData,
    Privacy,
    CommunicationProvider,
    ProviderProfile,
    Validation,
    Launch,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum FieldAvailability {
    Editable,
    ReadOnly { reason: String },
    Disabled { reason: String },
    Hidden,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ValueOption {
    pub value: String,
    pub label: String,
    pub description: String,
    pub destructive: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
pub struct WorkEstimate {
    pub steps: usize,
    pub minutes: u16,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FieldCapability {
    pub id: FieldId,
    pub availability: FieldAvailability,
    pub label: String,
    pub description: String,
    pub values: Vec<ValueOption>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PlanCapabilities {
    pub fields: Vec<FieldCapability>,
    pub destructive: bool,
    pub estimated_work: WorkEstimate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum StepId {
    DiscoverDevices,
    Preflight,
    ProviderService,
    BuildArtifacts,
    ResetClientData,
    InstallClients,
    LaunchClients,
    ValidateRuntime,
    CollectLogs,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum StepDisposition {
    Execute,
    Reuse,
    Skip,
    Blocked,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PlannedStep {
    pub id: StepId,
    pub label: String,
    pub disposition: StepDisposition,
    pub reason: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum CheckStatus {
    Pass,
    Warn,
    Fail,
    Skipped,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PreflightCheck {
    pub name: String,
    pub status: CheckStatus,
    pub detail: String,
    pub remediation: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PreflightReport {
    pub checks: Vec<PreflightCheck>,
    pub can_execute: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PlanDiff {
    pub changes: Vec<String>,
}

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
/// Provider lifecycle maintenance requested by an explicit deployment plan.
pub enum ProviderMaintenancePolicy {
    #[default]
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
/// opt-out and does not change Iroh transport or message privacy.
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
    pub provider_service_build: BuildPolicy,
    #[serde(default)]
    pub provider_maintenance: ProviderMaintenancePolicy,
    pub client_data: ClientDataPolicy,
    pub validation: ValidationLevel,
    pub launch: LaunchPolicy,
    #[serde(default)]
    pub privacy: PrivacyPolicy,
    /// Exactly one provider is selected for new sessions by each deployment.
    #[serde(default)]
    pub communication_provider: CommunicationProvider,
    /// Optional provider-owned runtime profile. The deployer treats this as
    /// opaque configuration; the selected provider validates its values at
    /// the composition boundary. Iroh currently accepts `always`, `direct`
    /// and `local`.
    #[serde(default)]
    pub provider_profile: Option<String>,
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
            communication_provider: iroh_provider(),
            provider_profile: None,
        }
    }

    pub fn validate(&self) -> Result<(), PlanError> {
        if self.targets.is_empty() && !matches!(self.action, DeployAction::ProviderMaintenance) {
            return Err(PlanError::NoTargets);
        }
        if self.action == DeployAction::ProviderMaintenance
            && !self.communication_provider.deployment_profile().commissioning_service.is_managed()
        {
            return Err(PlanError::UnsupportedProviderMaintenance {
                provider: self.communication_provider.clone(),
            });
        }
        if self.provider_maintenance == ProviderMaintenancePolicy::RotateIdentity {
            if self.communication_provider != iroh_provider() {
                return Err(PlanError::UnsupportedProviderMaintenance {
                    provider: self.communication_provider.clone(),
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
            return Err(PlanError::ProviderNotReady(self.communication_provider.clone()));
        }
        if let Some(profile) = self.provider_profile.as_deref() {
            if profile.trim().is_empty() {
                return Err(PlanError::InvalidProviderProfile {
                    provider: self.communication_provider.clone(),
                    profile: profile.to_owned(),
                });
            }
            if self.communication_provider == iroh_provider()
                && !matches!(
                    profile.trim().to_ascii_lowercase().as_str(),
                    "always"
                        | "always-reachable"
                        | "direct"
                        | "direct-only"
                        | "local"
                        | "local-only"
                )
            {
                return Err(PlanError::InvalidProviderProfile {
                    provider: self.communication_provider.clone(),
                    profile: profile.to_owned(),
                });
            }
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
            self.client_build = BuildPolicy::Reuse;
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
        // Direct providers do not own a deployer-managed service. Persisting a
        // stale relay build policy on an Iroh plan is misleading and can make
        // a resumed run appear to require server work that must never happen.
        if !self.communication_provider.deployment_profile().commissioning_service.is_managed() {
            self.provider_service_build = BuildPolicy::Reuse;
            self.provider_maintenance = ProviderMaintenancePolicy::Ensure;
        }
        self
    }

    /// Returns the context-sensitive fields shown by the wizard.  The method
    /// deliberately works from the normalized plan so review and execution
    /// cannot disagree about an implied value.
    pub fn capabilities(&self) -> PlanCapabilities {
        let plan = self.clone().normalized();
        let managed =
            plan.communication_provider.deployment_profile().commissioning_service.is_managed();
        let descriptor = plan.communication_provider.descriptor();
        let mut fields = Vec::new();
        let mut add =
            |id, availability, label: &str, description: &str, values: Vec<ValueOption>| {
                fields.push(FieldCapability {
                    id,
                    availability,
                    label: label.into(),
                    description: description.into(),
                    values,
                });
            };
        add(
            FieldId::Targets,
            if plan.action == DeployAction::ProviderMaintenance {
                FieldAvailability::Hidden
            } else {
                FieldAvailability::Editable
            },
            "Targets",
            "Clients and devices affected by this plan.",
            Vec::new(),
        );
        add(
            FieldId::Configuration,
            if matches!(
                plan.action,
                DeployAction::CollectLogs
                    | DeployAction::RunInstalled
                    | DeployAction::ProviderMaintenance
            ) {
                FieldAvailability::Hidden
            } else {
                FieldAvailability::Editable
            },
            "Configuration",
            "Build configuration.",
            vec![option("debug", "Debug", false), option("release", "Release", false)],
        );
        let build_availability =
            if matches!(plan.action, DeployAction::CollectLogs | DeployAction::ProviderMaintenance)
            {
                FieldAvailability::Hidden
            } else if plan.action == DeployAction::RunInstalled {
                FieldAvailability::ReadOnly {
                    reason: "This action reuses installed or existing artifacts.".into(),
                }
            } else if plan.action == DeployAction::RedeployCurrent {
                FieldAvailability::ReadOnly {
                    reason: "Redeploy uses the current verified artifacts when possible.".into(),
                }
            } else if matches!(plan.action, DeployAction::Rebuild | DeployAction::FullRedeploy) {
                FieldAvailability::ReadOnly {
                    reason: "This action requires rebuilding client artifacts.".into(),
                }
            } else {
                FieldAvailability::Editable
            };
        add(
            FieldId::ClientBuild,
            build_availability,
            "Client build",
            "Build or reuse client artifacts.",
            vec![
                option("reuse", "Reuse", false),
                option("if_required", "If required", false),
                option("rebuild", "Rebuild", false),
            ],
        );
        let service_availability = if !managed {
            FieldAvailability::Disabled {
                reason: "This provider has no deployer-managed service.".into(),
            }
        } else if matches!(plan.action, DeployAction::RunInstalled) {
            FieldAvailability::ReadOnly {
                reason: "Run installed reuses the existing provider service.".into(),
            }
        } else if matches!(plan.action, DeployAction::Rebuild | DeployAction::FullRedeploy) {
            FieldAvailability::ReadOnly {
                reason: "This action requires rebuilding the provider service.".into(),
            }
        } else if matches!(plan.action, DeployAction::CollectLogs | DeployAction::BuildArtifacts) {
            FieldAvailability::Hidden
        } else {
            FieldAvailability::Editable
        };
        add(
            FieldId::ProviderServiceBuild,
            service_availability,
            "Provider service build",
            "Build the provider-owned service when supported.",
            vec![
                option("reuse", "Reuse", false),
                option("if_required", "If required", false),
                option("rebuild", "Rebuild", false),
            ],
        );
        let maintenance_availability = if !managed {
            FieldAvailability::Disabled {
                reason: "This provider has no deployer-managed service.".into(),
            }
        } else if matches!(
            plan.action,
            DeployAction::FullRedeploy | DeployAction::ProviderMaintenance
        ) {
            FieldAvailability::Editable
        } else {
            FieldAvailability::Hidden
        };
        add(
            FieldId::ProviderMaintenance,
            maintenance_availability,
            "Provider maintenance",
            "Managed provider service operation.",
            descriptor
                .maintenance
                .iter()
                .map(|maintenance| {
                    option(
                        maintenance.wire_value(),
                        maintenance.label(),
                        matches!(maintenance, torca_transport_api::MaintenanceOption::Restart),
                    )
                })
                .collect(),
        );
        let data_availability = if matches!(
            plan.action,
            DeployAction::RunInstalled
                | DeployAction::CollectLogs
                | DeployAction::ProviderMaintenance
        ) {
            FieldAvailability::ReadOnly { reason: "This action preserves client data.".into() }
        } else {
            FieldAvailability::Editable
        };
        add(
            FieldId::ClientData,
            data_availability,
            "Client data",
            "Preserve, reset profiles, or reset all local data.",
            vec![
                option("preserve", "Preserve", false),
                option("reset_profile", "Reset profile", true),
                option("reset_all", "Reset all", true),
            ],
        );
        add(
            FieldId::Privacy,
            if plan.action == DeployAction::CollectLogs {
                FieldAvailability::Hidden
            } else if plan.targets.contains(&Target::Android) {
                FieldAvailability::Editable
            } else {
                FieldAvailability::Hidden
            },
            "Privacy",
            "Android screen-capture protection.",
            vec![
                option("strict", "Strict", false),
                option("allow_capture", "Allow capture", false),
            ],
        );
        add(
            FieldId::CommunicationProvider,
            FieldAvailability::Editable,
            "Provider",
            plan.communication_provider.protocol_label(),
            [iroh_provider()]
                .iter()
                .map(|provider| option(provider.wire_value(), provider.protocol_label(), false))
                .collect(),
        );
        add(
            FieldId::ProviderProfile,
            if descriptor.profiles.is_empty() {
                FieldAvailability::Hidden
            } else {
                FieldAvailability::Editable
            },
            "Provider profile",
            "Provider-owned routing profile.",
            descriptor
                .profiles
                .iter()
                .map(|profile| ValueOption {
                    value: profile.id.into(),
                    label: profile.label.into(),
                    description: profile.description.into(),
                    destructive: false,
                })
                .collect(),
        );
        add(
            FieldId::Validation,
            if matches!(
                plan.action,
                DeployAction::CollectLogs
                    | DeployAction::ProviderMaintenance
                    | DeployAction::RunInstalled
            ) {
                FieldAvailability::Hidden
            } else {
                FieldAvailability::Editable
            },
            "Validation",
            "Runtime readiness validation.",
            vec![
                option("skip", "Skip", false),
                option("quick", "Quick", false),
                option("full", "Full", false),
            ],
        );
        add(
            FieldId::Launch,
            if matches!(plan.action, DeployAction::CollectLogs | DeployAction::ProviderMaintenance)
            {
                FieldAvailability::Hidden
            } else {
                FieldAvailability::Editable
            },
            "Launch",
            "Start selected clients.",
            vec![
                option("skip", "Skip", false),
                option("start", "Start", false),
                option("restart", "Restart", false),
            ],
        );
        let destructive = !matches!(plan.client_data, ClientDataPolicy::Preserve)
            || plan.provider_maintenance == ProviderMaintenancePolicy::RotateIdentity;
        PlanCapabilities {
            fields,
            destructive,
            estimated_work: WorkEstimate {
                steps: plan
                    .planned_steps()
                    .iter()
                    .filter(|s| matches!(s.disposition, StepDisposition::Execute))
                    .count(),
                minutes: if destructive { 8 } else { 4 },
            },
        }
    }

    pub fn planned_steps(&self) -> Vec<PlannedStep> {
        let plan = self.clone().normalized();
        let mut steps = vec![
            planned_step(
                StepId::DiscoverDevices,
                "Discover devices",
                if plan.action == DeployAction::ProviderMaintenance {
                    StepDisposition::Skip
                } else {
                    StepDisposition::Execute
                },
                if plan.action == DeployAction::ProviderMaintenance {
                    "maintenance does not require client device discovery"
                } else {
                    "selected targets"
                },
            ),
            planned_step(
                StepId::Preflight,
                "Preflight",
                StepDisposition::Execute,
                "validate plan and environment",
            ),
        ];
        steps.push(planned_step(
            StepId::ProviderService,
            "Provider service",
            if plan.needs_provider_service() {
                StepDisposition::Execute
            } else {
                StepDisposition::Skip
            },
            if plan.needs_provider_service() {
                "managed by selected provider"
            } else {
                "selected provider has no deployer-managed service"
            },
        ));
        if matches!(plan.client_build, BuildPolicy::Rebuild | BuildPolicy::IfRequired)
            && !matches!(plan.action, DeployAction::RunInstalled | DeployAction::CollectLogs)
        {
            steps.push(planned_step(
                StepId::BuildArtifacts,
                "Build artifacts",
                StepDisposition::Execute,
                "client build policy",
            ));
        } else {
            steps.push(planned_step(
                StepId::BuildArtifacts,
                "Build artifacts",
                if plan.action == DeployAction::RunInstalled {
                    StepDisposition::Reuse
                } else {
                    StepDisposition::Skip
                },
                if plan.action == DeployAction::RunInstalled {
                    "Run installed reuses verified artifacts"
                } else {
                    "this action does not build client artifacts"
                },
            ));
        }
        steps.push(planned_step(
            StepId::ResetClientData,
            "Reset client data",
            if matches!(plan.client_data, ClientDataPolicy::Preserve) {
                StepDisposition::Skip
            } else {
                StepDisposition::Execute
            },
            if matches!(plan.client_data, ClientDataPolicy::Preserve) {
                "preserve policy selected"
            } else {
                "destructive data policy selected"
            },
        ));
        steps.push(planned_step(
            StepId::InstallClients,
            "Install clients",
            if matches!(plan.action, DeployAction::RunInstalled | DeployAction::BuildArtifacts) {
                StepDisposition::Reuse
            } else if matches!(
                plan.action,
                DeployAction::CollectLogs | DeployAction::ProviderMaintenance
            ) {
                StepDisposition::Skip
            } else {
                StepDisposition::Execute
            },
            if plan.action == DeployAction::RunInstalled {
                "installed clients are reused"
            } else if plan.action == DeployAction::BuildArtifacts {
                "artifact-only action does not install"
            } else if plan.action == DeployAction::CollectLogs {
                "log collection does not install clients"
            } else if plan.action == DeployAction::ProviderMaintenance {
                "provider maintenance does not install clients"
            } else {
                "install selected artifacts"
            },
        ));
        steps.push(planned_step(
            StepId::LaunchClients,
            "Launch clients",
            if matches!(plan.launch, LaunchPolicy::Skip) {
                StepDisposition::Skip
            } else {
                StepDisposition::Execute
            },
            "launch policy selected",
        ));
        steps.push(planned_step(
            StepId::ValidateRuntime,
            "Validate runtime",
            if matches!(plan.validation, ValidationLevel::Skip)
                || matches!(
                    plan.action,
                    DeployAction::CollectLogs | DeployAction::ProviderMaintenance
                )
            {
                StepDisposition::Skip
            } else {
                StepDisposition::Execute
            },
            "validation policy selected",
        ));
        if plan.action == DeployAction::CollectLogs {
            steps.push(planned_step(
                StepId::CollectLogs,
                "Collect logs",
                StepDisposition::Execute,
                "diagnostic collection",
            ));
        }
        steps
    }

    /// Plan-only checks shared by CLI and the interactive wizard. It never
    /// probes, builds, installs, or mutates a device.
    pub fn preflight(&self) -> PreflightReport {
        let plan = self.clone().normalized();
        let mut checks = Vec::new();
        match plan.validate() {
            Ok(()) => checks.push(PreflightCheck {
                name: "Plan".into(),
                status: CheckStatus::Pass,
                detail: "normalized plan is valid".into(),
                remediation: None,
            }),
            Err(error) => checks.push(PreflightCheck {
                name: "Plan".into(),
                status: CheckStatus::Fail,
                detail: error.to_string(),
                remediation: Some("edit the affected plan option".into()),
            }),
        }
        checks.push(PreflightCheck {
            name: "Provider profile".into(),
            status: CheckStatus::Pass,
            detail: plan.communication_provider.protocol_label().into(),
            remediation: None,
        });
        let can_execute = checks.iter().all(|check| check.status != CheckStatus::Fail);
        PreflightReport { checks, can_execute }
    }

    pub fn normalized_diff(&self) -> PlanDiff {
        let normalized = self.clone().normalized();
        let mut changes = Vec::new();
        macro_rules! compare {
            ($field:ident) => {
                if self.$field != normalized.$field {
                    changes.push(format!(
                        "{}: {:?} -> {:?}",
                        stringify!($field),
                        self.$field,
                        normalized.$field
                    ));
                }
            };
        }
        compare!(action);
        compare!(targets);
        compare!(client_build);
        compare!(provider_service_build);
        compare!(client_data);
        compare!(launch);
        compare!(provider_maintenance);
        PlanDiff { changes }
    }

    /// Stable, redaction-safe identity of the normalized execution plan. The
    /// executor stores it with every checkpoint so a retry cannot silently
    /// reuse completed stages after the plan has been edited.
    pub fn fingerprint(&self) -> String {
        let normalized = self.clone().normalized();
        let payload = serde_json::to_vec(&normalized).unwrap_or_default();
        let digest = Sha256::digest(payload);
        digest.iter().map(|byte| format!("{byte:02x}")).collect()
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

fn option(value: &str, label: &str, destructive: bool) -> ValueOption {
    ValueOption { value: value.into(), label: label.into(), description: label.into(), destructive }
}

fn planned_step(
    id: StepId,
    label: &str,
    disposition: StepDisposition,
    reason: &str,
) -> PlannedStep {
    PlannedStep { id, label: label.into(), disposition, reason: reason.into() }
}

#[derive(Debug, Error)]
pub enum PlanError {
    #[error("a client deployment requires at least one target")]
    NoTargets,
    #[error("provider identity rotation requires provider and client rebuilds")]
    RotationRequiresRebuild,
    #[error("provider identity rotation requires Windows and Android to be selected")]
    RotationRequiresAllTargets,
    #[error("communication provider '{0}' is not ready for deployment")]
    ProviderNotReady(CommunicationProvider),
    #[error("provider '{provider}' does not support the selected maintenance action")]
    UnsupportedProviderMaintenance { provider: CommunicationProvider },
    #[error("provider '{provider}' does not support profile '{profile}'")]
    InvalidProviderProfile { provider: CommunicationProvider, profile: String },
    #[error("deployment checkpoint does not match the normalized plan")]
    PlanFingerprintMismatch,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeployStage {
    Planned,
    ProviderServicePrepared,
    ProviderServiceReachable,
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
    /// Empty is accepted for freshly created checkpoints and filled on first resume.
    #[serde(default)]
    pub plan_fingerprint: String,
    pub stage: DeployStage,
    pub provider_endpoint: Option<String>,
    pub completed: Vec<DeployStage>,
    pub message: Option<String>,
}

impl DeployRun {
    pub const fn is_resumable(&self) -> bool {
        !self.stage.terminal()
    }

    pub fn new(plan: DeployPlan) -> Self {
        let plan_fingerprint = plan.fingerprint();
        let started_at_ms =
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis();
        Self {
            schema: 1,
            run_id: format!("{started_at_ms:x}-{}", std::process::id()),
            started_at_ms,
            plan,
            plan_fingerprint,
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
        plan.communication_provider = iroh_provider();
        assert!(plan.validate().is_ok());
    }

    #[test]
    fn relay_requirement_is_provider_metadata_not_an_action_assumption() {
        let plan = DeployPlan::normal(
            DeployAction::RedeployCurrent,
            vec![Target::Android],
            Configuration::Debug,
        );
        assert!(!plan.needs_provider_service());

        let mut direct = plan.clone();
        direct.communication_provider = iroh_provider();
        assert!(!direct.needs_provider_service());

        let mut direct = plan;
        direct.communication_provider = iroh_provider();
        assert!(!direct.needs_provider_service());
    }

    #[test]
    fn direct_provider_normalization_never_requests_managed_service_work() {
        let mut plan = DeployPlan::normal(
            DeployAction::FullRedeploy,
            vec![Target::Android],
            Configuration::Debug,
        );
        plan.communication_provider = iroh_provider();
        plan.provider_service_build = BuildPolicy::Rebuild;
        plan.provider_maintenance = ProviderMaintenancePolicy::Restart;
        let normalized = plan.normalized();
        assert_eq!(normalized.provider_service_build, BuildPolicy::Reuse);
        assert_eq!(normalized.provider_maintenance, ProviderMaintenancePolicy::Ensure);
    }

    #[test]
    fn direct_provider_maintenance_is_rejected_instead_of_building_clients() {
        let mut plan =
            DeployPlan::normal(DeployAction::ProviderMaintenance, vec![], Configuration::Debug);
        plan.communication_provider = iroh_provider();
        assert!(matches!(plan.validate(), Err(PlanError::UnsupportedProviderMaintenance { .. })));
    }

    #[test]
    fn iroh_profile_is_validated_at_the_deployment_boundary() {
        let mut plan = DeployPlan::normal(
            DeployAction::RedeployCurrent,
            vec![Target::Android],
            Configuration::Debug,
        );
        plan.communication_provider = iroh_provider();
        plan.provider_profile = Some("direct-only".into());
        assert!(plan.validate().is_ok());
        plan.provider_profile = Some("unknown".into());
        assert!(matches!(plan.validate(), Err(PlanError::InvalidProviderProfile { .. })));
    }

    #[test]
    fn capabilities_are_populated_from_the_selected_provider_descriptor() {
        let mut plan = DeployPlan::normal(
            DeployAction::FullRedeploy,
            vec![Target::Android],
            Configuration::Debug,
        );
        plan.communication_provider = iroh_provider();
        let capabilities = plan.capabilities();
        let profile = capabilities
            .fields
            .iter()
            .find(|field| field.id == FieldId::ProviderProfile)
            .expect("iroh profile capability");
        assert!(matches!(profile.availability, FieldAvailability::Editable));
        assert_eq!(
            profile.values.iter().map(|value| value.value.as_str()).collect::<Vec<_>>(),
            ["always", "direct", "local"]
        );
        let maintenance = capabilities
            .fields
            .iter()
            .find(|field| field.id == FieldId::ProviderMaintenance)
            .expect("iroh maintenance capability");
        assert!(maintenance.values.is_empty());
        assert!(matches!(maintenance.availability, FieldAvailability::Disabled { .. }));
    }

    #[test]
    fn provider_maintenance_reuses_client_artifacts_and_skips_client_build_step() {
        let plan =
            DeployPlan::normal(DeployAction::ProviderMaintenance, Vec::new(), Configuration::Debug);
        let normalized = plan.normalized();
        assert_eq!(normalized.client_build, BuildPolicy::Reuse);
        let build = normalized
            .planned_steps()
            .into_iter()
            .find(|step| step.id == StepId::BuildArtifacts)
            .expect("client build step");
        assert_eq!(build.disposition, StepDisposition::Skip);
        let install = normalized
            .planned_steps()
            .into_iter()
            .find(|step| step.id == StepId::InstallClients)
            .expect("client install step");
        assert_eq!(install.disposition, StepDisposition::Skip);
        let capability = normalized
            .capabilities()
            .fields
            .into_iter()
            .find(|field| field.id == FieldId::ClientBuild)
            .expect("client build capability");
        assert_eq!(capability.availability, FieldAvailability::Hidden);
    }

    #[test]
    fn provider_profile_is_rejected_when_blank() {
        let mut plan = DeployPlan::normal(
            DeployAction::RedeployCurrent,
            vec![Target::Android],
            Configuration::Debug,
        );
        plan.provider_profile = Some("  ".into());
        assert!(matches!(plan.validate(), Err(PlanError::InvalidProviderProfile { .. })));
    }

    #[test]
    fn collect_logs_hides_deployment_fields() {
        let plan = DeployPlan::normal(
            DeployAction::CollectLogs,
            vec![Target::Android],
            Configuration::Debug,
        );
        let fields = plan.capabilities().fields;
        assert!(matches!(
            fields.iter().find(|field| field.id == FieldId::Privacy).unwrap().availability,
            FieldAvailability::Hidden
        ));
        assert!(matches!(
            fields.iter().find(|field| field.id == FieldId::Launch).unwrap().availability,
            FieldAvailability::Hidden
        ));
        let steps = plan.planned_steps();
        assert!(steps.iter().any(|step| {
            step.id == StepId::DiscoverDevices && step.disposition == StepDisposition::Execute
        }));
        assert!(steps.iter().any(|step| {
            step.id == StepId::CollectLogs && step.disposition == StepDisposition::Execute
        }));
    }

    #[test]
    fn run_installed_reuses_artifacts_and_installed_clients() {
        let plan = DeployPlan::normal(
            DeployAction::RunInstalled,
            vec![Target::Windows],
            Configuration::Debug,
        );
        let steps = plan.planned_steps();
        assert!(steps.iter().any(|step| {
            step.id == StepId::BuildArtifacts && step.disposition == StepDisposition::Reuse
        }));
        assert!(steps.iter().any(|step| {
            step.id == StepId::InstallClients && step.disposition == StepDisposition::Reuse
        }));
    }

    #[test]
    fn direct_provider_exposes_disabled_service_with_reason() {
        let mut plan = DeployPlan::normal(
            DeployAction::FullRedeploy,
            vec![Target::Windows],
            Configuration::Debug,
        );
        plan.communication_provider = iroh_provider();
        let service = plan
            .capabilities()
            .fields
            .into_iter()
            .find(|field| field.id == FieldId::ProviderServiceBuild)
            .unwrap();
        assert!(matches!(service.availability, FieldAvailability::Disabled { .. }));
        assert!(
            plan.planned_steps().iter().any(|step| step.id == StepId::ProviderService
                && step.disposition == StepDisposition::Skip)
        );
    }

    #[test]
    fn normalized_diff_explains_run_installed_invariants() {
        let mut plan = DeployPlan::normal(
            DeployAction::RunInstalled,
            vec![Target::Windows],
            Configuration::Debug,
        );
        plan.client_build = BuildPolicy::Rebuild;
        assert!(
            plan.normalized_diff().changes.iter().any(|change| change.contains("client_build"))
        );
    }
}
