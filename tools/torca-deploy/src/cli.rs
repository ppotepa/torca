use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::domain::{
    BuildPolicy, ClientDataPolicy, Configuration, DeployAction, DeployPlan, LaunchPolicy,
    PrivacyPolicy, RunTarget, Target, ValidationLevel,
};

#[derive(Debug, Parser)]
#[command(name = "torca-deploy", version, about = "Torca deployment wizard and automation CLI")]
pub struct Cli {
    /// Theme used by the interactive wizard.
    #[arg(long, global = true, value_enum, default_value = "aurora")]
    pub theme: ThemeArg,
    /// Disable terminal colours while retaining textual status markers.
    #[arg(long, global = true)]
    pub no_color: bool,
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
    /// Rebuild clients while preserving client data.
    Rebuild(PlanArgs),
    /// Reset selected client data and redeploy; preserves provider identity by default.
    FullRedeploy(PlanArgs),
    /// Continue the most recent interrupted deployment.
    Resume {
        #[arg(long)]
        dry_run: bool,
    },
    /// Collect logs from all selected devices.
    Logs(PlanArgs),
    /// Build client artifacts without starting devices.
    Build(PlanArgs),
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, Args)]
pub struct PlanArgs {
    #[arg(long, value_enum, default_value = "all")]
    pub target: TargetArg,
    /// Device id to deploy; wireless ADB mDNS collision counters are tolerated.
    #[arg(long)]
    pub device: Option<String>,
    /// Runtime destination(s). Repeat for multiple destinations.
    #[arg(long = "run-target", value_enum)]
    pub run_targets: Vec<RunTargetArg>,
    #[arg(long, value_enum, default_value = "debug")]
    pub configuration: ConfigurationArg,
    #[arg(long, value_enum, default_value = "if-required")]
    pub client_build: BuildPolicyArg,
    #[arg(long, value_enum, default_value = "preserve")]
    pub client_data: ClientDataPolicyArg,
    #[arg(long, value_enum, default_value = "quick")]
    pub validation: ValidationArg,
    #[arg(long, value_enum, default_value = "restart")]
    pub launch: LaunchArg,
    /// Android screen-capture policy. Strict is the default.
    #[arg(long, value_enum, default_value = "strict")]
    pub privacy: PrivacyArg,
    /// Iroh runtime profile: `always`, `direct` or `local`.
    #[arg(long = "provider-profile", visible_alias = "iroh-profile")]
    pub provider_profile: Option<String>,
    #[arg(long)]
    pub dry_run: bool,
    /// Print the normalized execution graph used by the executor.
    #[arg(long)]
    pub show_steps: bool,
    /// Run read-only plan and device checks before displaying or executing the plan.
    #[arg(long)]
    pub preflight: bool,
    #[arg(long, value_enum, default_value = "aurora")]
    pub theme: ThemeArg,
    #[arg(long)]
    pub no_color: bool,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ThemeArg {
    Aurora,
    Amber,
    HighContrast,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum TargetArg {
    Windows,
    Android,
    All,
}
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum RunTargetArg {
    Windows,
    Android,
    Emulator,
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
        // Freeze the Iroh profile into the persisted deployment plan so a
        // resumed run cannot silently change routing policy.
        let provider_profile = self.provider_profile.clone().or_else(|| {
            Some(std::env::var("TORCA_IROH_PROFILE").unwrap_or_else(|_| "always".to_owned()))
        });
        let mut targets = match self.target {
            TargetArg::Windows => vec![Target::Windows],
            TargetArg::Android => vec![Target::Android],
            TargetArg::All => vec![Target::Windows, Target::Android],
        };
        let run_targets = self
            .run_targets
            .iter()
            .map(|target| match target {
                RunTargetArg::Windows => {
                    if !targets.contains(&Target::Windows) {
                        targets.push(Target::Windows);
                    }
                    RunTarget::Windows
                }
                RunTargetArg::Android => {
                    if !targets.contains(&Target::Android) {
                        targets.push(Target::Android);
                    }
                    RunTarget::Android
                }
                RunTargetArg::Emulator => {
                    if !targets.contains(&Target::Android) {
                        targets.push(Target::Android);
                    }
                    RunTarget::Emulator
                }
            })
            .collect();
        DeployPlan {
            action,
            targets,
            run_targets,
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
    fn provider_profile_is_carried_by_the_plan() {
        let args = plan_args(&["torca-deploy", "plan", "--provider-profile", "direct-only"]);
        assert_eq!(
            args.plan(DeployAction::RedeployCurrent).provider_profile.as_deref(),
            Some("direct-only")
        );
    }

    #[test]
    fn iroh_gets_a_stable_default_profile_in_the_plan() {
        let args = plan_args(&["torca-deploy", "plan"]);
        assert_eq!(
            args.plan(DeployAction::RedeployCurrent).provider_profile.as_deref(),
            Some("always")
        );
    }

    #[test]
    fn explicit_runtime_targets_are_carried_into_the_plan() {
        let args =
            plan_args(&["torca-deploy", "plan", "--target", "windows", "--run-target", "emulator"]);
        let plan = args.plan(DeployAction::FullRedeploy);
        assert_eq!(plan.run_targets, vec![RunTarget::Emulator]);
        assert!(plan.targets.contains(&Target::Android));
    }
}
