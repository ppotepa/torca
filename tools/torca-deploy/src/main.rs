use clap::Parser;
use std::sync::Arc;

use torca_deploy::{
    DeployExecutor,
    cli::{Cli, Command},
    domain::{ClientDataPolicy, DeployAction},
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
            Ok(run) => println!("run={} stage={:?}", run.run_id, run.stage),
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
    }
    Ok(())
}

fn mode(dry_run: bool) -> ExecutionMode {
    if dry_run { ExecutionMode::DryRun } else { ExecutionMode::Execute }
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
        "Plan: {}\n  targets: {:?}\n  device: {:?}\n  configuration: {}\n  client build: {:?}\n  client data: {:?}\n  validation: {:?}\n  launch: {:?}\n  privacy: {:?}\n  provider: Iroh\n  Iroh profile: {:?}\n  mode: {:?}",
        plan.action,
        plan.targets,
        plan.device,
        plan.configuration,
        plan.client_build,
        plan.client_data,
        plan.validation,
        plan.launch,
        plan.privacy,
        plan.provider_profile,
        mode
    );
    if show_steps {
        println!("  steps:");
        for step in plan.planned_steps() {
            let marker = match (step.disposition, no_color) {
                (torca_deploy::domain::StepDisposition::Execute, false) => "[OK]",
                (torca_deploy::domain::StepDisposition::Reuse, false) => "[->]",
                (torca_deploy::domain::StepDisposition::Skip, false) => "[ ]",
                (torca_deploy::domain::StepDisposition::Blocked, false) => "[X]",
                (torca_deploy::domain::StepDisposition::Execute, true) => "[ok]",
                (torca_deploy::domain::StepDisposition::Reuse, true) => "[reuse]",
                (torca_deploy::domain::StepDisposition::Skip, true) => "[skip]",
                (torca_deploy::domain::StepDisposition::Blocked, true) => "[blocked]",
            };
            println!("    {marker} {} - {}", step.label, step.reason);
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
        torca_deploy::domain::StepDisposition::Execute => "[OK]",
        torca_deploy::domain::StepDisposition::Reuse => "[->]",
        torca_deploy::domain::StepDisposition::Skip => "[ ]",
        torca_deploy::domain::StepDisposition::Blocked => "[X]",
    }
}
