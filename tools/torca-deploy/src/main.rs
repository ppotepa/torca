use clap::Parser;

use torca_deploy::{
    DeployExecutor,
    cli::{Cli, Command, RelayAction},
    domain::{BuildPolicy, ClientDataPolicy, DeployAction, OnionPolicy},
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
    let store = StateStore::new(DeployPaths::discover()?);
    let executor = DeployExecutor::new(store);
    let Some(command) = cli.command else {
        let Some(selection) = tui::choose_plan()? else {
            return Ok(());
        };
        match selection {
            tui::WizardSelection::Resume => {
                let run = executor.resume(ExecutionMode::Execute)?;
                println!("Deployment {} reached {:?}", run.run_id, run.stage);
            }
            tui::WizardSelection::Plan(action) => {
                show_and_execute(&executor, action, ExecutionMode::Execute)?;
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
                run.relay_endpoint.unwrap_or_else(|| "unknown".into())
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
        Command::Plan(args) => show_and_execute(
            &executor,
            args.plan(DeployAction::RedeployCurrent),
            ExecutionMode::DryRun,
        )?,
        Command::Run(args) => {
            show_and_execute(&executor, args.plan(DeployAction::RunInstalled), mode(args.dry_run))?;
        }
        Command::Deploy(args) => show_and_execute(
            &executor,
            args.plan(DeployAction::RedeployCurrent),
            mode(args.dry_run),
        )?,
        Command::Rebuild(args) => {
            show_and_execute(&executor, args.plan(DeployAction::Rebuild), mode(args.dry_run))?;
        }
        Command::FullRedeploy(args) => {
            let mut plan = args.plan(DeployAction::FullRedeploy);
            if plan.client_data == ClientDataPolicy::Preserve {
                plan.client_data = ClientDataPolicy::ResetProfile;
            }
            show_and_execute(&executor, plan, mode(args.dry_run))?;
        }
        Command::Logs(args) => {
            show_and_execute(&executor, args.plan(DeployAction::CollectLogs), mode(args.dry_run))?;
        }
        Command::Build(args) => show_and_execute(
            &executor,
            args.plan(DeployAction::BuildArtifacts),
            mode(args.dry_run),
        )?,
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
                    DeployAction::RelayMaintenance,
                    Vec::new(),
                    configuration,
                );
                plan.relay_build = BuildPolicy::IfRequired;
                plan.onion = match args.action {
                    RelayAction::Ensure => OnionPolicy::Ensure,
                    RelayAction::Restart => OnionPolicy::Restart,
                    RelayAction::Repair => OnionPolicy::RepairDirectoryCache,
                    RelayAction::Rotate { confirm_rotate: true } => {
                        plan.action = DeployAction::FullRedeploy;
                        plan.targets = vec![
                            torca_deploy::domain::Target::Windows,
                            torca_deploy::domain::Target::Android,
                        ];
                        plan.client_build = BuildPolicy::Rebuild;
                        plan.relay_build = BuildPolicy::Rebuild;
                        OnionPolicy::RotateIdentity
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
    let plan = planner::normalize(plan);
    plan.validate()?;
    println!(
        "Plan: {}\n  targets: {:?}\n  configuration: {}\n  client build: {:?}\n  relay build: {:?}\n  onion: {:?}\n  client data: {:?}\n  validation: {:?}\n  launch: {:?}\n  mode: {:?}",
        plan.action,
        plan.targets,
        plan.configuration,
        plan.client_build,
        plan.relay_build,
        plan.onion,
        plan.client_data,
        plan.validation,
        plan.launch,
        mode
    );
    if mode == ExecutionMode::DryRun {
        return Ok(());
    }
    let run = deployment.create_run(plan)?;
    let result = deployment.execute(run, mode)?;
    println!("Deployment {} reached {:?}", result.run_id, result.stage);
    Ok(())
}
