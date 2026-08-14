use std::sync::Arc;
use thiserror::Error;

use crate::domain::{DeployPlan, DeployRun, DeployStage};
use crate::persistence::{PersistenceError, StateStore};
use crate::process::{CommandRunner, ProcessError, SystemCommandRunner};
use crate::{
    build::BuildController, data::DataController, devices::DeviceController,
    install::InstallController, launch::LaunchController, paths::RuntimePaths,
    relay::RelayController,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionMode {
    DryRun,
    Execute,
}

pub struct DeployExecutor {
    store: StateStore,
    runner: Arc<dyn CommandRunner>,
}

impl DeployExecutor {
    pub fn new(store: StateStore) -> Self {
        Self { store, runner: Arc::new(SystemCommandRunner) }
    }
    pub fn with_runner(store: StateStore, runner: Arc<dyn CommandRunner>) -> Self {
        Self { store, runner }
    }

    pub fn create_run(&self, plan: DeployPlan) -> Result<DeployRun, DeployError> {
        plan.validate().map_err(DeployError::Plan)?;
        let run = DeployRun::new(plan);
        self.store.save(&run).map_err(DeployError::State)?;
        Ok(run)
    }

    pub fn resume(&self, mode: ExecutionMode) -> Result<DeployRun, DeployError> {
        let run = self.store.load_current().map_err(DeployError::State)?;
        self.execute(run, mode)
    }

    pub fn relay_status(&self) -> Result<crate::relay::RelayStatus, DeployError> {
        let paths = RuntimePaths::from_repo(self.store.paths().repo_root.clone());
        paths.ensure().map_err(DeployError::Paths)?;
        RelayController::new(&paths, self.runner.as_ref()).status().map_err(DeployError::Relay)
    }

    pub fn execute(
        &self,
        mut run: DeployRun,
        mode: ExecutionMode,
    ) -> Result<DeployRun, DeployError> {
        if run.stage.terminal() {
            return Ok(run);
        }
        if mode == ExecutionMode::DryRun {
            return Ok(run);
        }
        let _lock = self.store.acquire_lock().map_err(DeployError::State)?;
        self.validate_endpoint(&run)?;
        if run.plan.needs_relay() {
            run.advance(DeployStage::RelayPrepared, "starting typed deployment transaction");
            self.checkpoint(&run)?;
        }
        if let Err(error) = self.run_native_orchestrator(&mut run) {
            run.stage = DeployStage::Interrupted;
            run.message = Some(error.to_string());
            let _ = self.checkpoint(&run);
            return Err(error);
        }
        run.relay_endpoint = self.read_relay_endpoint();
        if !matches!(run.plan.action, crate::domain::DeployAction::CollectLogs)
            && !matches!(run.plan.launch, crate::domain::LaunchPolicy::Skip)
        {
            run.advance(
                DeployStage::NetworkReady,
                "native launch completed; health verification delegated to client runtime",
            );
            self.checkpoint(&run)?;
        }
        run.advance(DeployStage::Completed, "deployment completed; Rust checkpoint recorded");
        self.store.save(&run).map_err(DeployError::State)?;
        Ok(run)
    }

    fn checkpoint(&self, run: &DeployRun) -> Result<(), DeployError> {
        self.store.save(run).map_err(DeployError::State)?;
        self.store
            .append_event(run, run.message.as_deref().unwrap_or("checkpoint"))
            .map_err(DeployError::State)
    }

    fn read_relay_endpoint(&self) -> Option<String> {
        std::fs::read_to_string(
            self.store.paths().repo_root.join(".torca/stack/relay_endpoint.txt"),
        )
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
    }

    fn validate_endpoint(&self, run: &DeployRun) -> Result<(), DeployError> {
        if let (Some(expected), Some(actual)) = (&run.relay_endpoint, self.read_relay_endpoint())
            && expected != &actual
        {
            return Err(DeployError::EndpointMismatch { expected: expected.clone(), actual });
        }
        Ok(())
    }

    fn run_native_orchestrator(&self, run: &mut DeployRun) -> Result<(), DeployError> {
        let paths = RuntimePaths::from_repo(self.store.paths().repo_root.clone());
        paths.ensure().map_err(DeployError::Paths)?;
        let endpoint = if run.plan.needs_relay()
            && !run.completed.contains(&DeployStage::EndpointVerified)
        {
            let previous = paths.endpoint();
            let relay = RelayController::new(&paths, self.runner.as_ref());
            let status =
                relay.ensure(run.plan.onion, run.plan.relay_build).map_err(DeployError::Relay)?;
            if !status.healthy {
                return Err(DeployError::RelayNotHealthy);
            }
            if matches!(run.plan.onion, crate::domain::OnionPolicy::RotateIdentity)
                && previous == status.endpoint
            {
                return Err(DeployError::EndpointMismatch {
                    expected: "new onion endpoint".into(),
                    actual: status.endpoint.unwrap_or_default(),
                });
            }
            run.relay_endpoint.clone_from(&status.endpoint);
            run.advance(
                DeployStage::RelayReachable,
                if status.onion_ready {
                    "relay protocol healthy and onion publication confirmed"
                } else {
                    "relay protocol healthy; onion publication continues in background"
                },
            );
            self.checkpoint(run)?;
            run.advance(
                DeployStage::EndpointVerified,
                "relay endpoint validated before client build",
            );
            self.checkpoint(run)?;
            status.endpoint
        } else {
            run.relay_endpoint
                .clone()
                .or_else(|| paths.endpoint())
                .or_else(|| std::env::var("TORCA_RELAY_ENDPOINT").ok())
        };
        // Artifact builds are intentionally host-only: a disconnected phone
        // must never prevent CI or a developer from producing an APK.
        let devices = if matches!(run.plan.action, crate::domain::DeployAction::BuildArtifacts) {
            Vec::new()
        } else {
            DeviceController::new(&paths, self.runner.as_ref())
                .discover(&run.plan.targets)
                .map_err(DeployError::Devices)?
        };
        if matches!(run.plan.action, crate::domain::DeployAction::CollectLogs) {
            let android = devices
                .iter()
                .filter(|device| matches!(device.target, crate::domain::Target::Android))
                .map(|device| device.id.clone())
                .collect::<Vec<_>>();
            let report =
                crate::diagnostics::collect_runtime(&paths, self.runner.as_ref(), &android);
            if !report.has_payload() {
                return Err(DeployError::DiagnosticsEmpty);
            }
            return Ok(());
        }
        if !run.completed.contains(&DeployStage::ArtifactsBuilt)
            && !matches!(
                run.plan.action,
                crate::domain::DeployAction::RunInstalled
                    | crate::domain::DeployAction::CollectLogs
            )
        {
            BuildController::new(&paths, self.runner.as_ref())
                .build(
                    &run.plan.targets,
                    &devices,
                    run.plan.configuration,
                    run.plan.client_build,
                    endpoint.as_deref(),
                )
                .map_err(DeployError::Build)?;
            run.advance(DeployStage::ArtifactsBuilt, "native Rust/Flutter artifacts built");
            self.checkpoint(run)?;
        }
        if !run.completed.contains(&DeployStage::ClientDataReset) {
            DataController::new(&paths, self.runner.as_ref())
                .reset(&devices, run.plan.client_data)
                .map_err(DeployError::Data)?;
        }
        if !matches!(run.plan.client_data, crate::domain::ClientDataPolicy::Preserve)
            && !run.completed.contains(&DeployStage::ClientDataReset)
        {
            run.advance(DeployStage::ClientDataReset, "selected client data reset");
            self.checkpoint(run)?;
        }
        if !run.completed.contains(&DeployStage::ClientsInstalled)
            && !matches!(
                run.plan.action,
                crate::domain::DeployAction::RunInstalled
                    | crate::domain::DeployAction::CollectLogs
                    | crate::domain::DeployAction::BuildArtifacts
            )
        {
            for device in &devices {
                InstallController::new(&paths, self.runner.as_ref())
                    .install(device, run.plan.configuration)
                    .map_err(DeployError::Install)?;
            }
            run.advance(DeployStage::ClientsInstalled, "selected client artifacts installed");
            self.checkpoint(run)?;
        }
        if !matches!(run.plan.launch, crate::domain::LaunchPolicy::Skip) {
            // Start every selected client before waiting for health.  The old
            // device-by-device loop made Android wait behind a fully warmed
            // Windows runtime (and vice versa), turning a slow onion publish
            // into a serial deployment stall.
            let launch = LaunchController::new(&paths, self.runner.as_ref());
            let receipts = if run.completed.contains(&DeployStage::ClientsLaunched) {
                let started_at = std::time::SystemTime::UNIX_EPOCH
                    + std::time::Duration::from_millis(
                        u64::try_from(run.started_at_ms).unwrap_or(u64::MAX),
                    );
                devices
                    .iter()
                    .map(|device| {
                        (device, crate::launch::LaunchReceipt::from_started_at(started_at))
                    })
                    .collect::<Vec<_>>()
            } else {
                let receipts = devices
                    .iter()
                    .map(|device| {
                        launch
                            .launch(device, run.plan.configuration)
                            .map(|receipt| (device, receipt))
                            .map_err(DeployError::Launch)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                for (device, _) in &receipts {
                    launch.wait_process(device).map_err(DeployError::Launch)?;
                    launch.wait_visible_surface(device).map_err(DeployError::Launch)?;
                }
                run.advance(
                    DeployStage::ClientsLaunched,
                    "selected clients started and exposed a visible surface",
                );
                self.checkpoint(run)?;
                receipts
            };
            for (device, receipt) in receipts {
                launch
                    .wait_network_ready(device, receipt, run.plan.validation)
                    .map_err(DeployError::Launch)?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum DeployError {
    #[error("invalid deployment plan: {0}")]
    Plan(crate::domain::PlanError),
    #[error("deployment state error: {0}")]
    State(PersistenceError),
    #[error("external command error: {0}")]
    Process(ProcessError),
    #[error("runtime path error: {0}")]
    Paths(crate::paths::PathError),
    #[error("relay deployment failed: {0}")]
    Relay(crate::relay::RelayError),
    #[error("relay is not healthy")]
    RelayNotHealthy,
    #[error("device discovery failed: {0}")]
    Devices(crate::devices::DeviceError),
    #[error("build failed: {0}")]
    Build(crate::build::BuildError),
    #[error("installation failed: {0}")]
    Install(crate::install::InstallError),
    #[error("data reset failed: {0}")]
    Data(crate::data::DataError),
    #[error("launch failed: {0}")]
    Launch(crate::launch::LaunchError),
    #[error("diagnostics collection produced no files")]
    DiagnosticsEmpty,
    #[error("deployment endpoint changed while resuming; expected {expected}, found {actual}")]
    EndpointMismatch { expected: String, actual: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Configuration, DeployAction, Target};
    use crate::process::{CommandOutput, CommandSpec};
    use std::path::PathBuf;

    struct FakeRunner;

    impl CommandRunner for FakeRunner {
        fn run(&self, _command: &CommandSpec) -> Result<CommandOutput, ProcessError> {
            Ok(CommandOutput { success: true, status: Some(0), text: "ok".into() })
        }
    }

    #[test]
    fn successful_execution_persists_completed_checkpoint() {
        let root =
            std::env::temp_dir().join(format!("torca-deploy-executor-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("scripts")).expect("create test root");
        let exe = root.join("apps/client/flutter/build/windows/x64/runner/Debug/torca_app.exe");
        std::fs::create_dir_all(exe.parent().expect("exe parent")).expect("create exe dir");
        std::fs::write(exe, "test").expect("exe");
        let paths = crate::persistence::DeployPaths {
            repo_root: PathBuf::from(&root),
            state_root: root.join(".torca/deploy"),
        };
        let deployment = DeployExecutor::with_runner(StateStore::new(paths), Arc::new(FakeRunner));
        let plan = DeployPlan::normal(
            DeployAction::RunInstalled,
            vec![Target::Windows],
            Configuration::Debug,
        );
        let mut plan = plan;
        plan.launch = crate::domain::LaunchPolicy::Skip;
        let run = deployment.create_run(plan).expect("create run");
        let completed = deployment.execute(run, ExecutionMode::Execute).expect("execute");
        assert_eq!(completed.stage, DeployStage::Completed);
        assert!(deployment.resume(ExecutionMode::DryRun).is_ok());
        let _ = std::fs::remove_dir_all(root);
    }
}
