use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use thiserror::Error;

use crate::domain::{
    CheckStatus, DeployPlan, DeployRun, DeployStage, PreflightCheck, PreflightReport,
};
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeployProgress {
    pub run_id: String,
    pub stage: DeployStage,
    pub completed_steps: usize,
    pub total_steps: usize,
    pub message: String,
}

pub type ProgressSink = Arc<dyn Fn(DeployProgress) + Send + Sync>;

#[derive(Clone, Default)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

#[derive(Clone)]
pub struct DeployExecutor {
    store: StateStore,
    runner: Arc<dyn CommandRunner>,
    progress_sink: Option<ProgressSink>,
}

impl DeployExecutor {
    pub fn new(store: StateStore) -> Self {
        Self { store, runner: Arc::new(SystemCommandRunner::default()), progress_sink: None }
    }
    pub fn with_runner(store: StateStore, runner: Arc<dyn CommandRunner>) -> Self {
        Self { store, runner, progress_sink: None }
    }
    pub fn with_progress(store: StateStore, sink: ProgressSink) -> Self {
        Self { store, runner: Arc::new(SystemCommandRunner::default()), progress_sink: Some(sink) }
    }
    pub fn with_runner_and_progress(
        store: StateStore,
        runner: Arc<dyn CommandRunner>,
        sink: ProgressSink,
    ) -> Self {
        Self { store, runner, progress_sink: Some(sink) }
    }

    pub fn with_progress_sink(mut self, sink: ProgressSink) -> Self {
        self.progress_sink = Some(sink);
        self
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

    /// Resume the persisted checkpoint after a failed or interrupted run.
    /// The caller supplies a fresh cancellation token so retry remains
    /// cancellable without replaying completed stages.
    pub fn retry_failed_stage(
        &self,
        mode: ExecutionMode,
        cancellation: &CancellationToken,
    ) -> Result<DeployRun, DeployError> {
        let run = self.store.load_current().map_err(DeployError::State)?;
        self.execute_with_cancel(run, mode, cancellation)
    }

    pub fn relay_status(&self) -> Result<crate::relay::RelayStatus, DeployError> {
        let paths = RuntimePaths::from_repo(self.store.paths().repo_root.clone());
        paths.ensure().map_err(DeployError::Paths)?;
        RelayController::new(&paths, self.runner.as_ref()).status().map_err(DeployError::Relay)
    }

    /// Collect a bounded incident snapshot without changing the deployment
    /// checkpoint.  This is intentionally best-effort for device discovery:
    /// diagnostics must remain useful when the device is precisely what failed.
    pub fn collect_diagnostics(
        &self,
        run: &DeployRun,
    ) -> Result<crate::diagnostics::DiagnosticsReport, DeployError> {
        let paths = RuntimePaths::from_repo(self.store.paths().repo_root.clone());
        paths.ensure().map_err(DeployError::Paths)?;
        let android_devices = DeviceController::new(&paths, self.runner.as_ref())
            .discover(&run.plan.targets)
            .ok()
            .and_then(|devices| {
                crate::devices::select_device(devices, run.plan.device.as_deref()).ok()
            })
            .unwrap_or_default()
            .into_iter()
            .filter(|device| matches!(device.target, crate::domain::Target::Android))
            .map(|device| device.id)
            .collect::<Vec<_>>();
        let report =
            crate::diagnostics::collect_runtime(&paths, self.runner.as_ref(), &android_devices);
        if !report.has_payload() {
            return Err(DeployError::DiagnosticsEmpty);
        }
        Ok(report)
    }

    /// Collect the bounded log bundle requested from the failure screen.
    ///
    /// Log collection intentionally shares the same read-only snapshot as
    /// diagnostics: it must work when the failed stage is device discovery
    /// or installation, and it must not alter the deployment checkpoint.
    pub fn collect_logs(
        &self,
        run: &DeployRun,
    ) -> Result<crate::diagnostics::DiagnosticsReport, DeployError> {
        self.collect_diagnostics(run)
    }

    /// Performs read-only checks needed before execution. Device discovery is
    /// deliberately kept here, next to the executor's actual device
    /// selection, so CLI and TUI can present the same operational blockers.
    pub fn preflight(&self, plan: &DeployPlan) -> PreflightReport {
        let mut report = plan.preflight();
        if !report.can_execute {
            return report;
        }
        if matches!(
            plan.action,
            crate::domain::DeployAction::ProviderMaintenance
                | crate::domain::DeployAction::BuildArtifacts
        ) && plan.device.is_none()
        {
            report.checks.push(PreflightCheck {
                name: "Devices".into(),
                status: CheckStatus::Skipped,
                detail: "this action does not require a selected device".into(),
                remediation: None,
            });
            return report;
        }
        let paths = RuntimePaths::from_repo(self.store.paths().repo_root.clone());
        match DeviceController::new(&paths, self.runner.as_ref()).discover(&plan.targets) {
            Ok(devices) => {
                let selected = crate::devices::select_device(devices, plan.device.as_deref());
                match selected {
                    Ok(devices) => report.checks.push(PreflightCheck {
                        name: "Devices".into(),
                        status: CheckStatus::Pass,
                        detail: if devices.is_empty() {
                            "no target devices required".into()
                        } else {
                            devices
                                .iter()
                                .map(|device| format!("{}: {}", device.target, device.id))
                                .collect::<Vec<_>>()
                                .join(", ")
                        },
                        remediation: None,
                    }),
                    Err(error) => {
                        report.can_execute = false;
                        report.checks.push(PreflightCheck {
                            name: "Devices".into(),
                            status: CheckStatus::Fail,
                            detail: error.to_string(),
                            remediation: Some("connect or authorize the selected device".into()),
                        });
                    }
                }
            }
            Err(error) => {
                report.can_execute = false;
                report.checks.push(PreflightCheck {
                    name: "Devices".into(),
                    status: CheckStatus::Fail,
                    detail: error.to_string(),
                    remediation: Some("connect the requested targets and retry discovery".into()),
                });
            }
        }
        report
    }

    pub fn execute(&self, run: DeployRun, mode: ExecutionMode) -> Result<DeployRun, DeployError> {
        self.execute_with_cancel(run, mode, &CancellationToken::default())
    }

    pub fn execute_with_cancel(
        &self,
        mut run: DeployRun,
        mode: ExecutionMode,
        cancellation: &CancellationToken,
    ) -> Result<DeployRun, DeployError> {
        let fingerprint = run.plan.fingerprint();
        if !run.plan_fingerprint.is_empty() && run.plan_fingerprint != fingerprint {
            return Err(DeployError::Plan(crate::domain::PlanError::PlanFingerprintMismatch));
        }
        if run.plan_fingerprint.is_empty() {
            run.plan_fingerprint = fingerprint;
        }
        if run.stage.terminal() {
            return Ok(run);
        }
        if mode == ExecutionMode::DryRun {
            return Ok(run);
        }
        let _lock = self.store.acquire_lock().map_err(DeployError::State)?;
        if self.cancelled(&mut run, cancellation) {
            return Err(DeployError::Cancelled);
        }
        self.validate_endpoint(&run)?;
        if run.plan.needs_provider_service() {
            run.advance(
                DeployStage::ProviderServicePrepared,
                "starting typed deployment transaction",
            );
            self.checkpoint(&run)?;
        }
        if let Err(error) = self.run_native_orchestrator(&mut run, cancellation) {
            run.stage = DeployStage::Interrupted;
            run.message = Some(error.to_string());
            let _ = self.checkpoint(&run);
            return Err(error);
        }
        if self.cancelled(&mut run, cancellation) {
            return Err(DeployError::Cancelled);
        }
        if run.plan.communication_provider.deployment_profile().commissioning_service.is_managed() {
            run.provider_endpoint = self.read_relay_endpoint();
        } else {
            run.provider_endpoint = None;
        }
        if !matches!(run.plan.action, crate::domain::DeployAction::CollectLogs)
            && !matches!(run.plan.launch, crate::domain::LaunchPolicy::Skip)
        {
            match run.plan.validation {
                crate::domain::ValidationLevel::Skip => {}
                crate::domain::ValidationLevel::Quick => {
                    run.advance(
                        DeployStage::RuntimeReady,
                        "all selected clients reached fresh provider local-ready evidence",
                    );
                    self.checkpoint(&run)?;
                }
                crate::domain::ValidationLevel::Full => {
                    run.advance(
                        DeployStage::NetworkReady,
                        "all selected clients reached fresh provider network-ready evidence",
                    );
                    self.checkpoint(&run)?;
                }
            }
        }
        run.advance(DeployStage::Completed, "deployment completed; Rust checkpoint recorded");
        self.store.save(&run).map_err(DeployError::State)?;
        self.emit_progress(&run);
        Ok(run)
    }

    fn checkpoint(&self, run: &DeployRun) -> Result<(), DeployError> {
        self.store.save(run).map_err(DeployError::State)?;
        self.store
            .append_event(run, run.message.as_deref().unwrap_or("checkpoint"))
            .map_err(DeployError::State)?;
        self.emit_progress(run);
        Ok(())
    }

    fn emit_progress(&self, run: &DeployRun) {
        let Some(sink) = &self.progress_sink else {
            return;
        };
        let total_steps = run.plan.planned_steps().len();
        sink(DeployProgress {
            run_id: run.run_id.clone(),
            stage: run.stage,
            completed_steps: run.completed.len().min(total_steps),
            total_steps,
            message: run.message.clone().unwrap_or_default(),
        });
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
        if !run.plan.communication_provider.deployment_profile().commissioning_service.is_managed()
        {
            return Ok(());
        }
        if let (Some(expected), Some(actual)) = (&run.provider_endpoint, self.read_relay_endpoint())
            && expected != &actual
        {
            return Err(DeployError::EndpointMismatch { expected: expected.clone(), actual });
        }
        Ok(())
    }

    fn cancelled(&self, run: &mut DeployRun, cancellation: &CancellationToken) -> bool {
        if !cancellation.is_cancelled() {
            return false;
        }
        run.stage = DeployStage::Interrupted;
        run.message = Some("deployment cancellation requested".into());
        let _ = self.checkpoint(run);
        true
    }

    fn run_native_orchestrator(
        &self,
        run: &mut DeployRun,
        cancellation: &CancellationToken,
    ) -> Result<(), DeployError> {
        let paths = RuntimePaths::from_repo(self.store.paths().repo_root.clone());
        paths.ensure().map_err(DeployError::Paths)?;
        let endpoint = if run.plan.needs_provider_service()
            && !run.completed.contains(&DeployStage::ProviderEndpointVerified)
        {
            let previous = paths.endpoint();
            let relay = RelayController::new(&paths, self.runner.as_ref());
            let status = relay
                .ensure(run.plan.provider_maintenance, run.plan.provider_service_build)
                .map_err(DeployError::Relay)?;
            if !status.healthy {
                return Err(DeployError::RelayNotHealthy);
            }
            if matches!(
                run.plan.provider_maintenance,
                crate::domain::ProviderMaintenancePolicy::RotateIdentity
            ) && previous == status.endpoint
            {
                return Err(DeployError::EndpointMismatch {
                    expected: "new onion endpoint".into(),
                    actual: status.endpoint.unwrap_or_default(),
                });
            }
            run.provider_endpoint.clone_from(&status.endpoint);
            run.advance(
                DeployStage::ProviderServiceReachable,
                if status.onion_ready {
                    "relay protocol healthy and onion publication confirmed"
                } else {
                    "relay protocol healthy; onion publication continues in background"
                },
            );
            self.checkpoint(run)?;
            run.advance(
                DeployStage::ProviderEndpointVerified,
                "relay endpoint validated before client build",
            );
            self.checkpoint(run)?;
            status.endpoint
        } else if run
            .plan
            .communication_provider
            .deployment_profile()
            .commissioning_service
            .is_managed()
        {
            run.provider_endpoint
                .clone()
                .or_else(|| paths.endpoint())
                .or_else(|| std::env::var("TORCA_PROVIDER_ENDPOINT").ok())
                .or_else(|| std::env::var("TORCA_RELAY_ENDPOINT").ok())
        } else {
            None
        };
        // Generic artifact builds are intentionally host-only: a disconnected
        // phone must never prevent CI from producing portable APKs. An exact
        // device opt-in (used by the soak cockpit) is different: discover it
        // so native/Flutter builds can be restricted to its actual ABI.
        let devices = if matches!(run.plan.action, crate::domain::DeployAction::BuildArtifacts)
            && run.plan.device.is_none()
        {
            Vec::new()
        } else {
            let discovered = DeviceController::new(&paths, self.runner.as_ref())
                .discover_with_retry(&run.plan.targets)
                .map_err(DeployError::Devices)?;
            crate::devices::select_device(discovered, run.plan.device.as_deref())
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
            if self.cancelled(run, cancellation) {
                return Err(DeployError::Cancelled);
            }
            BuildController::new(&paths, self.runner.as_ref())
                .build(
                    &run.plan.targets,
                    &devices,
                    run.plan.configuration,
                    run.plan.client_build,
                    endpoint.as_deref(),
                    run.plan.communication_provider,
                    run.plan.provider_profile.as_deref(),
                )
                .map_err(DeployError::Build)?;
            run.advance(DeployStage::ArtifactsBuilt, "native Rust/Flutter artifacts built");
            self.checkpoint(run)?;
        }
        if !run.completed.contains(&DeployStage::ClientDataReset) {
            if self.cancelled(run, cancellation) {
                return Err(DeployError::Cancelled);
            }
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
        // A persisted checkpoint is only valid while the selected package and
        // its launchable activity still exist on every Android target. This
        // prevents a stale `ClientsInstalled` stage from skipping install
        // after an uninstall, flavor switch, or device data reset.
        let clients_installed = run.completed.contains(&DeployStage::ClientsInstalled)
            && devices.iter().all(|device| {
                matches!(device.target, crate::domain::Target::Windows)
                    || InstallController::new(&paths, self.runner.as_ref())
                        .verify_installed(&device.id)
                        .is_ok()
            });
        if !clients_installed
            && !matches!(
                run.plan.action,
                crate::domain::DeployAction::RunInstalled
                    | crate::domain::DeployAction::CollectLogs
                    | crate::domain::DeployAction::BuildArtifacts
                    | crate::domain::DeployAction::ProviderMaintenance
            )
        {
            if self.cancelled(run, cancellation) {
                return Err(DeployError::Cancelled);
            }
            for device in &devices {
                InstallController::new(&paths, self.runner.as_ref())
                    .install(
                        device,
                        run.plan.configuration,
                        run.plan.communication_provider,
                        run.plan.provider_profile.as_deref(),
                    )
                    .map_err(DeployError::Install)?;
            }
            run.advance(DeployStage::ClientsInstalled, "selected client artifacts installed");
            self.checkpoint(run)?;
        }
        if !matches!(run.plan.launch, crate::domain::LaunchPolicy::Skip) {
            if self.cancelled(run, cancellation) {
                return Err(DeployError::Cancelled);
            }
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
                            .launch(
                                device,
                                run.plan.configuration,
                                run.plan.privacy,
                                run.plan.communication_provider,
                                run.plan.provider_profile.as_deref(),
                                matches!(run.plan.launch, crate::domain::LaunchPolicy::Restart),
                            )
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
                    .wait_network_ready(
                        device,
                        receipt,
                        run.plan.validation,
                        run.plan.communication_provider,
                    )
                    .map_err(DeployError::Launch)?;
            }
        }
        if !matches!(run.plan.action, crate::domain::DeployAction::CollectLogs) {
            let endpoint = endpoint.as_deref();
            if run
                .plan
                .communication_provider
                .deployment_profile()
                .commissioning_service
                .is_managed()
                && endpoint.is_none()
            {
                return Err(DeployError::MissingEndpoint);
            }
            crate::manifests::synchronize(
                &paths,
                &devices,
                run.plan.configuration,
                run.plan.communication_provider,
                endpoint,
                run.plan.provider_profile.as_deref(),
            )
            .map_err(DeployError::Manifest)?;
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
    #[error("deployment manifest synchronization failed: {0}")]
    Manifest(crate::manifests::ManifestError),
    #[error("deployment manifest synchronization requires a relay endpoint")]
    MissingEndpoint,
    #[error("deployment cancellation requested")]
    Cancelled,
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
        std::fs::write(&exe, "test").expect("exe");
        let endpoint = format!("{}.onion:443", "a".repeat(56));
        std::fs::create_dir_all(root.join(".torca/stack")).expect("stack directory");
        std::fs::create_dir_all(root.join(".torca/manifests")).expect("manifest directory");
        std::fs::create_dir_all(root.join("release")).expect("release directory");
        std::fs::write(root.join(".torca/stack/relay_endpoint.txt"), &endpoint).expect("endpoint");
        std::fs::write(
            root.join(".torca/manifests/clients-debug.json"),
            serde_json::to_vec(&serde_json::json!({
                "endpoint": endpoint,
                "communicationProvider": "tor",
                "targets": ["windows"],
                "buildId": "BUILD",
                "sourceCommit": "COMMIT",
                "compiledFeatures": "provider-tor,radio-audio",
                "artifacts": [{
                    "target": "windows",
                    "path": exe,
                    "sha256": "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08"
                }],
                "builtAt": "NOW"
            }))
            .expect("manifest json"),
        )
        .expect("manifest");
        std::fs::write(
            root.join("release/version.json"),
            r#"{"version":"1.0.0","build":1,"contractSchema":1,"wireVersion":1,"storageEpoch":1,"schemaVersion":1}"#,
        )
        .expect("release metadata");
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

    #[test]
    fn resume_rejects_a_plan_changed_after_checkpoint_creation() {
        let root =
            std::env::temp_dir().join(format!("torca-deploy-fingerprint-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let paths = crate::persistence::DeployPaths {
            repo_root: root.clone(),
            state_root: root.join(".torca/deploy"),
        };
        let deployment = DeployExecutor::with_runner(StateStore::new(paths), Arc::new(FakeRunner));
        let plan = DeployPlan::normal(
            crate::domain::DeployAction::RunInstalled,
            vec![crate::domain::Target::Windows],
            crate::domain::Configuration::Debug,
        );
        let mut run = deployment.create_run(plan).expect("create run");
        run.plan.configuration = crate::domain::Configuration::Release;
        assert!(matches!(
            deployment.execute(run, ExecutionMode::DryRun),
            Err(DeployError::Plan(crate::domain::PlanError::PlanFingerprintMismatch))
        ));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn progress_sink_receives_checkpoint_projection() {
        let root =
            std::env::temp_dir().join(format!("torca-deploy-progress-{}", std::process::id()));
        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let receiver = Arc::clone(&seen);
        let deployment = DeployExecutor::with_progress(
            StateStore::new(crate::persistence::DeployPaths {
                repo_root: root,
                state_root: std::env::temp_dir().join("torca-deploy-progress-state"),
            }),
            Arc::new(move |event| receiver.lock().expect("progress lock").push(event)),
        );
        let run = DeployRun::new(DeployPlan::normal(
            DeployAction::RunInstalled,
            vec![Target::Windows],
            Configuration::Debug,
        ));
        deployment.emit_progress(&run);
        let events = seen.lock().expect("progress lock");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].stage, DeployStage::Planned);
        assert_eq!(events[0].total_steps, run.plan.planned_steps().len());
    }

    #[test]
    fn cancellation_persists_interrupted_checkpoint_before_device_work() {
        let root = std::env::temp_dir().join(format!("torca-deploy-cancel-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let deployment = DeployExecutor::with_runner(
            StateStore::new(crate::persistence::DeployPaths {
                repo_root: root.clone(),
                state_root: root.join(".torca/deploy"),
            }),
            Arc::new(FakeRunner),
        );
        let run = deployment
            .create_run(DeployPlan::normal(
                DeployAction::RunInstalled,
                vec![Target::Windows],
                Configuration::Debug,
            ))
            .expect("create run");
        let cancellation = CancellationToken::default();
        cancellation.cancel();
        assert!(matches!(
            deployment.execute_with_cancel(run, ExecutionMode::Execute, &cancellation),
            Err(DeployError::Cancelled)
        ));
        assert_eq!(
            deployment.resume(ExecutionMode::DryRun).expect("load checkpoint").stage,
            DeployStage::Interrupted
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
