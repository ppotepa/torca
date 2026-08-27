use clap::Parser;
use std::sync::Arc;

use torca_deploy::{
    DeployExecutor,
    cli::{Cli, Command, RelayAction},
    domain::{BuildPolicy, ClientDataPolicy, DeployAction, ProviderMaintenancePolicy},
    executor::ExecutionMode,
    persistence::{DeployPaths, StateStore},
    planner, tui,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("torca-deploy: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let global_no_color = cli.no_color || std::env::var_os("NO_COLOR").is_some();
    let store = StateStore::new(DeployPaths::discover()?);
    let executor = DeployExecutor::with_progress(
        store,
        Arc::new(|progress| {
            println!(
                "progress {}/{} {:?}: {}",
                progress.completed_steps, progress.total_steps, progress.stage, progress.message
            );
        }),
    );
    let Some(command) = cli.command else {
        let theme = match cli.theme {
            torca_deploy::cli::ThemeArg::Aurora => tui::theme::ThemeKind::Aurora,
            torca_deploy::cli::ThemeArg::Amber => tui::theme::ThemeKind::Amber,
            torca_deploy::cli::ThemeArg::HighContrast => tui::theme::ThemeKind::HighContrast,
        };
        let Some(selection) = tui::choose_plan(theme, global_no_color)? else {
            return Ok(());
        };
        match selection {
            tui::WizardSelection::Resume => {
                let run = executor.resume(ExecutionMode::Execute)?;
                println!("Deployment {} reached {:?}", run.run_id, run.stage);
            }
            tui::WizardSelection::Plan(action) => {
                tui::app::execute_plan_with_dashboard(
                    executor.clone(),
                    action,
                    theme,
                    global_no_color,
                )?;
            }
        }
        return Ok(());
    };
    match command {
        Command::Status => match executor.resume(ExecutionMode::DryRun) {
            Ok(run) => println!(
                "run={} stage={:?} endpoint={}",
                run.run_id,
                run.stage,
                run.provider_endpoint.unwrap_or_else(|| "unknown".into())
            ),
            Err(_) => println!("No resumable Torca deployment checkpoint found."),
        },
        Command::Resume { dry_run } => {
            let run = executor.resume(if dry_run {
                ExecutionMode::DryRun
            } else {
                ExecutionMode::Execute
            })?;
            println!("run={} stage={:?}", run.run_id, run.stage);
        }
        Command::Plan(args) => {
            let plan = args.plan(DeployAction::RedeployCurrent);
            show_and_execute_with_options(
                &executor,
                plan,
                ExecutionMode::DryRun,
                OutputOptions {
                    show_steps: args.show_steps,
                    preflight: args.preflight,
                    no_color: global_no_color || args.no_color,
                },
            )?;
        }
        Command::Run(args) => {
            let plan = args.plan(DeployAction::RunInstalled);
            show_and_execute_with_options(
                &executor,
                plan,
                mode(args.dry_run),
                OutputOptions {
                    show_steps: args.show_steps,
                    preflight: args.preflight,
                    no_color: global_no_color || args.no_color,
                },
            )?;
        }
        Command::Deploy(args) => {
            let plan = args.plan(DeployAction::RedeployCurrent);
            show_and_execute_with_options(
                &executor,
                plan,
                mode(args.dry_run),
                OutputOptions {
                    show_steps: args.show_steps,
                    preflight: args.preflight,
                    no_color: global_no_color || args.no_color,
                },
            )?;
        }
        Command::Rebuild(args) => {
            let plan = args.plan(DeployAction::Rebuild);
            show_and_execute_with_options(
                &executor,
                plan,
                mode(args.dry_run),
                OutputOptions {
                    show_steps: args.show_steps,
                    preflight: args.preflight,
                    no_color: global_no_color || args.no_color,
                },
            )?;
        }
        Command::FullRedeploy(args) => {
            let mut plan = args.plan(DeployAction::FullRedeploy);
            if plan.client_data == ClientDataPolicy::Preserve {
                plan.client_data = ClientDataPolicy::ResetProfile;
            }
            show_and_execute_with_options(
                &executor,
                plan,
                mode(args.dry_run),
                OutputOptions {
                    show_steps: args.show_steps,
                    preflight: args.preflight,
                    no_color: global_no_color || args.no_color,
                },
            )?;
        }
        Command::Logs(args) => {
            let plan = args.plan(DeployAction::CollectLogs);
            show_and_execute_with_options(
                &executor,
                plan,
                mode(args.dry_run),
                OutputOptions {
                    show_steps: args.show_steps,
                    preflight: args.preflight,
                    no_color: global_no_color || args.no_color,
                },
            )?;
        }
        Command::Build(args) => {
            let plan = args.plan(DeployAction::BuildArtifacts);
            show_and_execute_with_options(
                &executor,
                plan,
                mode(args.dry_run),
                OutputOptions {
                    show_steps: args.show_steps,
                    preflight: args.preflight,
                    no_color: global_no_color || args.no_color,
                },
            )?;
        }
        Command::Relay(args) => match args.action {
            RelayAction::Status => {
                let status = executor.relay_status()?;
                println!(
                    "relay running={} healthy={} onion_ready={} endpoint={}",
                    status.running,
                    status.healthy,
                    status.onion_ready,
                    status.endpoint.unwrap_or_else(|| "unknown".into())
                );
            }
            RelayAction::Ensure
            | RelayAction::Restart
            | RelayAction::Repair
            | RelayAction::Rotate { .. } => {
                let configuration = match args.configuration {
                    torca_deploy::cli::ConfigurationArg::Debug => {
                        torca_deploy::domain::Configuration::Debug
                    }
                    torca_deploy::cli::ConfigurationArg::Release => {
                        torca_deploy::domain::Configuration::Release
                    }
                };
                let mut plan = torca_deploy::domain::DeployPlan::normal(
                    DeployAction::ProviderMaintenance,
                    Vec::new(),
                    configuration,
                );
                plan.provider_service_build = BuildPolicy::IfRequired;
                plan.provider_maintenance = match args.action {
                    RelayAction::Ensure => ProviderMaintenancePolicy::Ensure,
                    RelayAction::Restart => ProviderMaintenancePolicy::Restart,
                    RelayAction::Repair => ProviderMaintenancePolicy::RepairDirectoryCache,
                    RelayAction::Rotate { confirm_rotate: true } => {
                        plan.action = DeployAction::FullRedeploy;
                        plan.targets = vec![
                            torca_deploy::domain::Target::Windows,
                            torca_deploy::domain::Target::Android,
                        ];
                        plan.client_build = BuildPolicy::Rebuild;
                        plan.provider_service_build = BuildPolicy::Rebuild;
                        ProviderMaintenancePolicy::RotateIdentity
                    }
                    RelayAction::Rotate { confirm_rotate: false } => {
                        return Err("relay rotate requires --confirm-rotate".into());
                    }
                    RelayAction::Status => unreachable!(),
                };
                show_and_execute(&executor, plan, mode(args.dry_run))?;
            }
        },
    }
    Ok(())
}

fn mode(dry_run: bool) -> ExecutionMode {
    if dry_run { ExecutionMode::DryRun } else { ExecutionMode::Execute }
}

fn show_and_execute(
    deployment: &DeployExecutor,
    plan: torca_deploy::DeployPlan,
    mode: ExecutionMode,
) -> Result<(), Box<dyn std::error::Error>> {
    show_and_execute_with_options(deployment, plan, mode, OutputOptions::default())
}

#[derive(Clone, Copy, Debug, Default)]
struct OutputOptions {
    show_steps: bool,
    preflight: bool,
    no_color: bool,
}

fn show_and_execute_with_options(
    deployment: &DeployExecutor,
    plan: torca_deploy::DeployPlan,
    mode: ExecutionMode,
    options: OutputOptions,
) -> Result<(), Box<dyn std::error::Error>> {
    let original_plan = plan.clone();
    let plan = planner::normalize(plan);
    let no_color = options.no_color || std::env::var_os("NO_COLOR").is_some();
    let show_steps = options.show_steps || mode == ExecutionMode::DryRun;
    plan.validate()?;
    println!(
        "Plan: {}\n  targets: {:?}\n  device: {:?}\n  configuration: {}\n  client build: {:?}\n  provider service build: {:?}\n  provider maintenance: {:?}\n  client data: {:?}\n  validation: {:?}\n  launch: {:?}\n  privacy: {:?}\n  communication protocol: {}\n  provider profile: {:?}\n  mode: {:?}",
        plan.action,
        plan.targets,
        plan.device,
        plan.configuration,
        plan.client_build,
        plan.provider_service_build,
        plan.provider_maintenance,
        plan.client_data,
        plan.validation,
        plan.launch,
        plan.privacy,
        plan.communication_provider,
        plan.provider_profile,
        mode
    );
    if show_steps {
        println!("  steps:");
        for step in plan.planned_steps() {
            let marker = match (step.disposition, no_color) {
                (torca_deploy::domain::StepDisposition::Execute, false) => "✓",
                (torca_deploy::domain::StepDisposition::Reuse, false) => "→",
                (torca_deploy::domain::StepDisposition::Skip, false) => "○",
                (torca_deploy::domain::StepDisposition::Blocked, false) => "✗",
                (torca_deploy::domain::StepDisposition::Execute, true) => "[ok]",
                (torca_deploy::domain::StepDisposition::Reuse, true) => "[reuse]",
                (torca_deploy::domain::StepDisposition::Skip, true) => "[skip]",
                (torca_deploy::domain::StepDisposition::Blocked, true) => "[blocked]",
            };
            println!("    {marker} {} — {}", step.label, step.reason);
        }
    }
    if options.preflight {
        let report = deployment.preflight(&plan);
        println!("\n{}", torca_deploy::tui::screens::preflight::text(&report));
        if !report.can_execute {
            return Err("preflight blocked execution".into());
        }
    }
    let diff = original_plan.normalized_diff();
    if !diff.changes.is_empty() {
        println!("  normalized changes:");
        for change in diff.changes {
            println!("    ! {change}");
        }
    }
    if mode == ExecutionMode::DryRun {
        return Ok(());
    }
    println!("\nTORCA DEPLOY  RUNNING");
    println!("Overall progress: 0/{}", plan.planned_steps().len());
    for step in plan.planned_steps() {
        println!("  {} {}", step_marker(step.disposition), step.label);
    }
    let run = deployment.create_run(plan)?;
    println!(
        "{}",
        torca_deploy::tui::screens::execution::title(
            &torca_deploy::tui::model::ExecutionDashboard::new(run.clone()),
        )
    );
    let result = match deployment.execute(run, mode) {
        Ok(result) => result,
        Err(error) => {
            eprintln!(
                "\nDeployment failed\nReason: {error}\nSuggested action: {} after fixing the reported stage.",
                torca_deploy::tui::screens::failure::action_label(
                    torca_deploy::tui::model::FailureAction::RetryFailedStage,
                )
            );
            return Err(error.into());
        }
    };
    println!("Deployment {} reached {:?}", result.run_id, result.stage);
    Ok(())
}

fn step_marker(disposition: torca_deploy::domain::StepDisposition) -> &'static str {
    match disposition {
        torca_deploy::domain::StepDisposition::Execute => "✓",
        torca_deploy::domain::StepDisposition::Reuse => "→",
        torca_deploy::domain::StepDisposition::Skip => "○",
        torca_deploy::domain::StepDisposition::Blocked => "✗",
    }
}
