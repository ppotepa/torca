use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::domain::{
    BuildPolicy, ClientDataPolicy, CommunicationProvider, Configuration, DeployAction, DeployPlan,
    LaunchPolicy, PrivacyPolicy, ProviderMaintenancePolicy, Target, ValidationLevel,
};

#[derive(Debug, Parser)]
#[command(name = "torca-deploy", version, about = "Torca deployment wizard and automation CLI")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Show current deployment checkpoint and discovered configuration.
    Status,
    /// Show a plan without changing the host or devices.
    Plan(PlanArgs),
    /// Run already installed clients.
    Run(PlanArgs),
    /// Deploy current or rebuilt artifacts.
    Deploy(PlanArgs),
    /// Rebuild clients and relay, preserving client data and relay identity.
    Rebuild(PlanArgs),
    /// Reset selected client data and redeploy; preserves provider identity by default.
    FullRedeploy(PlanArgs),
    /// Relay-only operations.
    Relay(RelayArgs),
    /// Continue the most recent interrupted deployment.
    Resume {
        #[arg(long)]
        dry_run: bool,
    },
    /// Collect logs from all selected devices.
    Logs(PlanArgs),
    /// Build client artifacts without starting relay or devices.
    Build(PlanArgs),
}

#[derive(Clone, Debug, Args)]
pub struct PlanArgs {
    #[arg(long, value_enum, default_value = "all")]
    pub target: TargetArg,
    /// Exact device id to deploy; without it all ready devices are selected.
    #[arg(long)]
    pub device: Option<String>,
    #[arg(long, value_enum, default_value = "debug")]
    pub configuration: ConfigurationArg,
    #[arg(long, value_enum, default_value = "if-required")]
    pub client_build: BuildPolicyArg,
    #[arg(
        long = "provider-service-build",
        visible_alias = "relay-build",
        value_enum,
        default_value = "if-required"
    )]
    pub provider_service_build: BuildPolicyArg,
    #[arg(
        long = "provider-maintenance",
        visible_alias = "onion",
        value_enum,
        default_value = "ensure"
    )]
    pub provider_maintenance: ProviderMaintenancePolicyArg,
    #[arg(long, value_enum, default_value = "preserve")]
    pub client_data: ClientDataPolicyArg,
    #[arg(long, value_enum, default_value = "quick")]
    pub validation: ValidationArg,
    #[arg(long, value_enum, default_value = "restart")]
    pub launch: LaunchArg,
    /// Android screen-capture policy. Strict is the default.
    #[arg(long, value_enum, default_value = "strict")]
    pub privacy: PrivacyArg,
    /// Communication protocol selected for new peer sessions. Exactly one
    /// protocol is used by a deployment; Tor remains the default.
    #[arg(long, visible_alias = "communication-protocol", value_enum, default_value = "tor")]
    pub communication_provider: CommunicationProviderArg,
    /// Provider-owned runtime profile. This remains opaque to the generic
    /// deploy domain; Iroh uses `always`, `direct` or `local`.
    #[arg(long = "provider-profile", visible_alias = "iroh-profile")]
    pub provider_profile: Option<String>,
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct RelayArgs {
    #[command(subcommand)]
    pub action: RelayAction,
    #[arg(long, value_enum, default_value = "debug")]
    pub configuration: ConfigurationArg,
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Clone, Copy, Debug, Subcommand)]
pub enum RelayAction {
    Status,
    Ensure,
    Restart,
    Repair,
    Rotate {
        #[arg(long)]
        confirm_rotate: bool,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum TargetArg {
    Windows,
    Android,
    All,
}
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ConfigurationArg {
    Debug,
    Release,
}
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum BuildPolicyArg {
    Reuse,
    #[value(name = "if-required")]
    IfRequired,
    Rebuild,
}
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ProviderMaintenancePolicyArg {
    Ensure,
    Restart,
    Repair,
    Rotate,
}
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ClientDataPolicyArg {
    Preserve,
    #[value(name = "reset-profile")]
    ResetProfile,
    #[value(name = "reset-all")]
    ResetAll,
}
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ValidationArg {
    Skip,
    Quick,
    Full,
}
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum LaunchArg {
    Skip,
    Start,
    Restart,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum PrivacyArg {
    Strict,
    #[value(name = "allow-capture")]
    AllowCapture,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum CommunicationProviderArg {
    Tor,
    Iroh,
    #[value(name = "web-rtc", alias = "webrtc")]
    WebRtc,
}

impl PlanArgs {
    pub fn plan(&self, action: DeployAction) -> DeployPlan {
        let communication_provider = match self.communication_provider {
            CommunicationProviderArg::Tor => CommunicationProvider::Tor,
            CommunicationProviderArg::Iroh => CommunicationProvider::Iroh,
            CommunicationProviderArg::WebRtc => CommunicationProvider::WebRtc,
        };
        // Freeze the provider profile into the persisted deployment plan. A
        // resumed run must not silently change from direct to relay-backed
        // Iroh because the developer shell changed in the meantime.
        let provider_profile = self.provider_profile.clone().or_else(|| {
            (communication_provider == CommunicationProvider::Iroh).then(|| {
                std::env::var("TORCA_IROH_PROFILE").unwrap_or_else(|_| "always".to_owned())
            })
        });
        DeployPlan {
            action,
            targets: match self.target {
                TargetArg::Windows => vec![Target::Windows],
                TargetArg::Android => vec![Target::Android],
                TargetArg::All => vec![Target::Windows, Target::Android],
            },
            device: self.device.clone(),
            configuration: match self.configuration {
                ConfigurationArg::Debug => Configuration::Debug,
                ConfigurationArg::Release => Configuration::Release,
            },
            client_build: match self.client_build {
                BuildPolicyArg::Reuse => BuildPolicy::Reuse,
                BuildPolicyArg::IfRequired => BuildPolicy::IfRequired,
                BuildPolicyArg::Rebuild => BuildPolicy::Rebuild,
            },
            provider_service_build: match self.provider_service_build {
                BuildPolicyArg::Reuse => BuildPolicy::Reuse,
                BuildPolicyArg::IfRequired => BuildPolicy::IfRequired,
                BuildPolicyArg::Rebuild => BuildPolicy::Rebuild,
            },
            provider_maintenance: match self.provider_maintenance {
                ProviderMaintenancePolicyArg::Ensure => ProviderMaintenancePolicy::Ensure,
                ProviderMaintenancePolicyArg::Restart => ProviderMaintenancePolicy::Restart,
                ProviderMaintenancePolicyArg::Repair => {
                    ProviderMaintenancePolicy::RepairDirectoryCache
                }
                ProviderMaintenancePolicyArg::Rotate => ProviderMaintenancePolicy::RotateIdentity,
            },
            client_data: match self.client_data {
                ClientDataPolicyArg::Preserve => ClientDataPolicy::Preserve,
                ClientDataPolicyArg::ResetProfile => ClientDataPolicy::ResetProfile,
                ClientDataPolicyArg::ResetAll => ClientDataPolicy::ResetAll,
            },
            validation: match self.validation {
                ValidationArg::Skip => ValidationLevel::Skip,
                ValidationArg::Quick => ValidationLevel::Quick,
                ValidationArg::Full => ValidationLevel::Full,
            },
            launch: match self.launch {
                LaunchArg::Skip => LaunchPolicy::Skip,
                LaunchArg::Start => LaunchPolicy::Start,
                LaunchArg::Restart => LaunchPolicy::Restart,
            },
            privacy: match self.privacy {
                PrivacyArg::Strict => PrivacyPolicy::Strict,
                PrivacyArg::AllowCapture => PrivacyPolicy::AllowCapture,
            },
            communication_provider,
            provider_profile,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan_args(arguments: &[&str]) -> PlanArgs {
        let cli = Cli::try_parse_from(arguments).expect("valid deploy arguments");
        match cli.command.expect("subcommand") {
            Command::Plan(args) => args,
            other => panic!("expected plan command, got {other:?}"),
        }
    }

    #[test]
    fn communication_protocol_defaults_to_tor() {
        let args = plan_args(&["torca-deploy", "plan"]);
        assert!(matches!(
            args.plan(DeployAction::RedeployCurrent).communication_provider,
            CommunicationProvider::Tor
        ));
    }

    #[test]
    fn communication_protocol_can_be_selected_by_canonical_flag() {
        let args = plan_args(&["torca-deploy", "plan", "--communication-provider", "iroh"]);
        assert!(matches!(
            args.plan(DeployAction::RedeployCurrent).communication_provider,
            CommunicationProvider::Iroh
        ));
    }

    #[test]
    fn communication_protocol_alias_and_webrtc_alias_are_supported() {
        let args = plan_args(&["torca-deploy", "plan", "--communication-protocol", "webrtc"]);
        assert!(matches!(
            args.plan(DeployAction::RedeployCurrent).communication_provider,
            CommunicationProvider::WebRtc
        ));
    }

    #[test]
    fn provider_profile_is_carried_by_the_plan() {
        let args = plan_args(&[
            "torca-deploy",
            "plan",
            "--communication-provider",
            "iroh",
            "--provider-profile",
            "direct-only",
        ]);
        assert_eq!(
            args.plan(DeployAction::RedeployCurrent).provider_profile.as_deref(),
            Some("direct-only")
        );
    }

    #[test]
    fn iroh_gets_a_stable_default_profile_in_the_plan() {
        let args = plan_args(&["torca-deploy", "plan", "--communication-provider", "iroh"]);
        assert_eq!(
            args.plan(DeployAction::RedeployCurrent).provider_profile.as_deref(),
            Some("always")
        );
    }

    #[test]
    fn incomplete_communication_provider_cannot_pass_plan_validation() {
        let args = plan_args(&["torca-deploy", "plan", "--communication-provider", "webrtc"]);
        assert!(args.plan(DeployAction::RedeployCurrent).validate().is_err());
    }
}
