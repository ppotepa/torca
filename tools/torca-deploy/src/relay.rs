use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use thiserror::Error;

use crate::domain::{BuildPolicy, OnionPolicy};
use crate::paths::{PathError, RuntimePaths};
use crate::process::{CommandRunner, CommandSpec, ProcessError};

const STARTUP_TIMEOUT: Duration = Duration::from_secs(90);
const REQUIRED_HEALTHY_CHECKS: u8 = 3;

#[derive(Clone, Debug)]
pub struct RelayStatus {
    pub endpoint: Option<String>,
    pub running: bool,
    pub healthy: bool,
    pub onion_ready: bool,
}

pub struct RelayController<'a> {
    paths: &'a RuntimePaths,
    runner: &'a dyn CommandRunner,
}

impl<'a> RelayController<'a> {
    pub fn new(paths: &'a RuntimePaths, runner: &'a dyn CommandRunner) -> Self {
        Self { paths, runner }
    }

    pub fn ensure(
        &self,
        onion: OnionPolicy,
        build: BuildPolicy,
    ) -> Result<RelayStatus, RelayError> {
        self.paths.ensure()?;
        self.require_compose()?;
        let should_build = matches!(build, BuildPolicy::Rebuild)
            || (matches!(build, BuildPolicy::IfRequired) && !self.relay_manifest().is_file());
        let destructive = matches!(
            onion,
            OnionPolicy::Restart | OnionPolicy::RepairDirectoryCache | OnionPolicy::RotateIdentity
        ) || should_build;
        if destructive {
            self.compose(&["down", "--timeout", "60", "--remove-orphans"])?;
        }
        match onion {
            OnionPolicy::RepairDirectoryCache => {
                self.remove_scoped(self.paths.stack_root.join("tor/cache"))?;
            }
            OnionPolicy::RotateIdentity => self.remove_scoped(self.paths.stack_root.join("tor"))?,
            OnionPolicy::Ensure | OnionPolicy::Restart => {}
        }
        let mut args = vec!["up", "-d"];
        if should_build {
            args.push("--build");
        }
        if matches!(build, BuildPolicy::Reuse) {
            args.push("--no-build");
        }
        self.compose(&args)?;
        if should_build {
            fs::write(
                self.relay_manifest(),
                format!("built-at={:?}\n", std::time::SystemTime::now()),
            )
            .map_err(RelayError::Io)?;
        }
        // The endpoint is allocated before descriptor publication completes.
        // Client installation must not wait several minutes for Tor consensus;
        // clients supervise relay reachability and reconnect independently.
        self.wait_ready(STARTUP_TIMEOUT)
    }

    pub fn stop(&self) -> Result<(), RelayError> {
        self.require_compose()?;
        self.compose(&["down", "--timeout", "60", "--remove-orphans"])
    }

    pub fn status(&self) -> Result<RelayStatus, RelayError> {
        let output = self.command(
            "docker",
            &[
                "compose",
                "-f",
                &self.paths.docker_compose.display().to_string(),
                "ps",
                "-q",
                "relay",
            ],
        )?;
        let running = output.success && !output.text.trim().is_empty();
        let healthy = if running {
            let id = output.text.trim().lines().last().unwrap_or_default();
            self.command("docker", &["inspect", "--format", "{{.State.Health.Status}}", id])
                .map(|result| result.text.trim() == "healthy")
                .unwrap_or(false)
        } else {
            false
        };
        let endpoint = self.paths.endpoint();
        let onion_ready = endpoint
            .as_deref()
            .is_some_and(|value| relay_ready_matches(&self.paths.relay_ready, value));
        Ok(RelayStatus { endpoint, running, healthy, onion_ready })
    }

    fn wait_ready(&self, timeout: Duration) -> Result<RelayStatus, RelayError> {
        let deadline = Instant::now() + timeout;
        let started = Instant::now();
        let mut next_heartbeat = Instant::now();
        let mut last = String::new();
        let mut last_reported = None;
        let mut consecutive_healthy = 0_u8;
        while Instant::now() < deadline {
            let ps = self.quiet_command(
                "docker",
                &[
                    "compose",
                    "-f",
                    &self.paths.docker_compose.display().to_string(),
                    "ps",
                    "-q",
                    "relay",
                ],
            )?;
            if ps.success && !ps.text.trim().is_empty() {
                let id = ps.text.trim().lines().last().unwrap_or_default();
                let health = self.quiet_command(
                    "docker",
                    &["inspect", "--format", "{{.State.Health.Status}}", id],
                )?;
                health.text.trim().clone_into(&mut last);
                let endpoint = self.paths.endpoint();
                let onion_ready = endpoint
                    .as_deref()
                    .is_some_and(|value| relay_ready_matches(&self.paths.relay_ready, value));
                let state = format!("container={last}, onion_ready={onion_ready}");
                if last_reported.as_deref() != Some(state.as_str()) {
                    eprintln!("torca-deploy: relay warm-up {state}");
                    last_reported = Some(state);
                }
                if Instant::now() >= next_heartbeat {
                    eprintln!(
                        "torca-deploy: relay warm-up heartbeat elapsed_s={} health={} onion_ready={}",
                        started.elapsed().as_secs(),
                        last,
                        onion_ready
                    );
                    next_heartbeat = Instant::now() + Duration::from_secs(10);
                }
                if matches!(last.as_str(), "healthy" | "none") {
                    if endpoint.as_deref().is_some_and(RuntimePaths::validate_endpoint) {
                        let check = self.quiet_command(
                            "docker",
                            &["exec", id, "/usr/local/bin/torca-relay", "health-check"],
                        )?;
                        if startup_gate_passed(&mut consecutive_healthy, check.success) {
                            if !onion_ready {
                                eprintln!(
                                    "torca-deploy: relay protocol is healthy after {consecutive_healthy} checks; onion publication continues in background"
                                );
                            }
                            return Ok(RelayStatus {
                                endpoint,
                                running: true,
                                healthy: true,
                                onion_ready,
                            });
                        }
                    }
                }
            }
            std::thread::sleep(Duration::from_secs(2));
        }
        Err(RelayError::NotReady { last_health: last })
    }

    fn compose(&self, args: &[&str]) -> Result<(), RelayError> {
        let compose = self.paths.docker_compose.display().to_string();
        let mut command = vec!["compose", "-f", compose.as_str()];
        command.extend(args.iter().copied());
        let mut attempts = 0;
        loop {
            let output = self.command("docker", &command)?;
            if output.success {
                return Ok(());
            }
            attempts += 1;
            if attempts >= 3 || !args.contains(&"down") {
                return Err(RelayError::Compose { args: command.join(" "), output: output.text });
            }
            std::thread::sleep(Duration::from_secs(2));
        }
    }

    fn command(
        &self,
        program: &str,
        arguments: &[&str],
    ) -> Result<crate::process::CommandOutput, RelayError> {
        self.runner
            .run(&CommandSpec {
                program: program.into(),
                arguments: arguments.iter().map(|x| (*x).into()).collect(),
                working_directory: self.paths.repo_root.clone(),
                timeout: Duration::from_secs(120),
                environment: std::collections::BTreeMap::new(),
            })
            .map_err(RelayError::Process)
    }

    fn quiet_command(
        &self,
        program: &str,
        arguments: &[&str],
    ) -> Result<crate::process::CommandOutput, RelayError> {
        self.runner
            .run_quiet(&CommandSpec {
                program: program.into(),
                arguments: arguments.iter().map(|value| (*value).to_owned()).collect(),
                working_directory: self.paths.repo_root.clone(),
                timeout: Duration::from_secs(60),
                environment: std::collections::BTreeMap::new(),
            })
            .map_err(RelayError::Process)
    }

    fn require_compose(&self) -> Result<(), RelayError> {
        if !self.paths.docker_compose.is_file() {
            return Err(RelayError::ComposeFile(self.paths.docker_compose.clone()));
        }
        Ok(())
    }

    fn relay_manifest(&self) -> std::path::PathBuf {
        self.paths.manifests.join("relay.json")
    }

    fn remove_scoped(&self, path: impl AsRef<Path>) -> Result<(), RelayError> {
        let path = path.as_ref();
        let configured_root = &self.paths.stack_root;
        let relative = path
            .strip_prefix(configured_root)
            .map_err(|_| RelayError::UnsafeReset(path.to_path_buf()))?;
        if relative.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        }) {
            return Err(RelayError::UnsafeReset(path.to_path_buf()));
        }
        // Docker can remove the target directory before a reset policy runs.
        // Compare the lexical path to the configured root first, then resolve
        // existing paths to prevent a junction/symlink from escaping the stack.
        let root = fs::canonicalize(configured_root).map_err(RelayError::Io)?;
        if path.exists() {
            let target = fs::canonicalize(path).map_err(RelayError::Io)?;
            if !target.starts_with(&root) {
                return Err(RelayError::UnsafeReset(path.to_path_buf()));
            }
            fs::remove_dir_all(target).map_err(RelayError::Io)?;
        }
        Ok(())
    }
}

fn relay_ready_matches(path: &Path, endpoint: &str) -> bool {
    std::fs::read_to_string(path).map(|value| value.trim() == endpoint).unwrap_or(false)
}

fn startup_gate_passed(consecutive_healthy: &mut u8, protocol_healthy: bool) -> bool {
    if protocol_healthy {
        *consecutive_healthy = consecutive_healthy.saturating_add(1);
    } else {
        *consecutive_healthy = 0;
    }
    *consecutive_healthy >= REQUIRED_HEALTHY_CHECKS
}

#[derive(Debug, Error)]
pub enum RelayError {
    #[error("relay paths: {0}")]
    Paths(#[from] PathError),
    #[error("Docker Compose file not found: {0}")]
    ComposeFile(std::path::PathBuf),
    #[error("relay process error: {0}")]
    Process(#[from] ProcessError),
    #[error("relay compose failed ({args}): {output}")]
    Compose { args: String, output: String },
    #[error("relay did not become ready: last health={last_health}")]
    NotReady { last_health: String },
    #[error("refusing to remove relay path outside stack root: {0}")]
    UnsafeReset(std::path::PathBuf),
    #[error("relay onion rotation did not change endpoint")]
    RotationDidNotChange,
    #[error("relay filesystem error: {0}")]
    Io(std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoopRunner;

    impl CommandRunner for NoopRunner {
        fn run(
            &self,
            _command: &CommandSpec,
        ) -> Result<crate::process::CommandOutput, ProcessError> {
            unreachable!("the scoped-reset test does not execute a command")
        }
    }

    #[test]
    fn scoped_reset_accepts_an_absent_child_after_compose_down() {
        let root = std::env::temp_dir().join(format!("torca-deploy-relay-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let paths = RuntimePaths::from_repo(root.clone());
        paths.ensure().expect("runtime paths");
        let controller = RelayController::new(&paths, &NoopRunner);

        controller.remove_scoped(paths.stack_root.join("tor")).expect("absent child is in scope");
        assert!(!paths.stack_root.join("tor").exists());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn scoped_reset_rejects_a_path_outside_stack() {
        let root =
            std::env::temp_dir().join(format!("torca-deploy-relay-outside-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let paths = RuntimePaths::from_repo(root.clone());
        paths.ensure().expect("runtime paths");
        let controller = RelayController::new(&paths, &NoopRunner);

        assert!(matches!(
            controller.remove_scoped(root.join("outside")),
            Err(RelayError::UnsafeReset(_))
        ));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn stale_ready_marker_is_not_accepted_for_a_different_endpoint() {
        let root =
            std::env::temp_dir().join(format!("torca-deploy-relay-marker-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("marker directory");
        let marker = root.join("relay_ready.txt");
        fs::write(&marker, "old.onion:443\n").expect("marker");
        assert!(!relay_ready_matches(&marker, "new.onion:443"));
        assert!(relay_ready_matches(&marker, "old.onion:443"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn startup_gate_requires_three_consecutive_protocol_checks() {
        let mut healthy = 0;
        assert!(!startup_gate_passed(&mut healthy, true));
        assert!(!startup_gate_passed(&mut healthy, true));
        assert!(!startup_gate_passed(&mut healthy, false));
        assert_eq!(healthy, 0);
        assert!(!startup_gate_passed(&mut healthy, true));
        assert!(!startup_gate_passed(&mut healthy, true));
        assert!(startup_gate_passed(&mut healthy, true));
    }
}
