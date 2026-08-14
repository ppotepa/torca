use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::domain::{
    BuildPolicy, ClientDataPolicy, Configuration, DeployAction, DeployPlan, LaunchPolicy,
    OnionPolicy, PrivacyPolicy, Target, ValidationLevel,
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
    /// Reset selected client data and redeploy; preserves relay onion by default.
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
    #[arg(long, value_enum, default_value = "debug")]
    pub configuration: ConfigurationArg,
    #[arg(long, value_enum, default_value = "if-required")]
    pub client_build: BuildPolicyArg,
    #[arg(long, value_enum, default_value = "if-required")]
    pub relay_build: BuildPolicyArg,
    #[arg(long, value_enum, default_value = "ensure")]
    pub onion: OnionPolicyArg,
    #[arg(long, value_enum, default_value = "preserve")]
    pub client_data: ClientDataPolicyArg,
    #[arg(long, value_enum, default_value = "quick")]
    pub validation: ValidationArg,
    #[arg(long, value_enum, default_value = "restart")]
    pub launch: LaunchArg,
    /// Android screen-capture policy. Strict is the default.
    #[arg(long, value_enum, default_value = "strict")]
    pub privacy: PrivacyArg,
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
pub enum OnionPolicyArg {
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

impl PlanArgs {
    pub fn plan(&self, action: DeployAction) -> DeployPlan {
        DeployPlan {
            action,
            targets: match self.target {
                TargetArg::Windows => vec![Target::Windows],
                TargetArg::Android => vec![Target::Android],
                TargetArg::All => vec![Target::Windows, Target::Android],
            },
            configuration: match self.configuration {
                ConfigurationArg::Debug => Configuration::Debug,
                ConfigurationArg::Release => Configuration::Release,
            },
            client_build: match self.client_build {
                BuildPolicyArg::Reuse => BuildPolicy::Reuse,
                BuildPolicyArg::IfRequired => BuildPolicy::IfRequired,
                BuildPolicyArg::Rebuild => BuildPolicy::Rebuild,
            },
            relay_build: match self.relay_build {
                BuildPolicyArg::Reuse => BuildPolicy::Reuse,
                BuildPolicyArg::IfRequired => BuildPolicy::IfRequired,
                BuildPolicyArg::Rebuild => BuildPolicy::Rebuild,
            },
            onion: match self.onion {
                OnionPolicyArg::Ensure => OnionPolicy::Ensure,
                OnionPolicyArg::Restart => OnionPolicy::Restart,
                OnionPolicyArg::Repair => OnionPolicy::RepairDirectoryCache,
                OnionPolicyArg::Rotate => OnionPolicy::RotateIdentity,
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
        }
    }
}
