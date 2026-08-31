use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use thiserror::Error;

use crate::domain::DeployRun;

#[derive(Clone, Debug)]
pub struct DeployPaths {
    pub repo_root: PathBuf,
    pub state_root: PathBuf,
}

impl DeployPaths {
    pub fn discover() -> Result<Self, PersistenceError> {
        let mut current = std::env::current_dir().map_err(PersistenceError::CurrentDirectory)?;
        loop {
            if current.join("Cargo.toml").is_file() && current.join("crates").is_dir() {
                return Ok(Self {
                    state_root: current.join(".torca").join("deploy"),
                    repo_root: current,
                });
            }
            if !current.pop() {
                return Err(PersistenceError::RepoRoot);
            }
        }
    }

    pub fn runs_root(&self) -> PathBuf {
        self.state_root.join("runs")
    }
    pub fn current_path(&self) -> PathBuf {
        self.state_root.join("current.json")
    }
    pub fn last_plan_path(&self) -> PathBuf {
        self.state_root.join("last-plan.json")
    }
    pub fn run_path(&self, run_id: &str) -> PathBuf {
        self.runs_root().join(format!("{run_id}.json"))
    }
}

#[derive(Clone)]
pub struct StateStore {
    paths: DeployPaths,
}

impl StateStore {
    pub const fn new(paths: DeployPaths) -> Self {
        Self { paths }
    }
    pub const fn paths(&self) -> &DeployPaths {
        &self.paths
    }

    pub fn acquire_lock(&self) -> Result<DeployLock, PersistenceError> {
        fs::create_dir_all(&self.paths.state_root).map_err(PersistenceError::Write)?;
        let path = self.paths.state_root.join("active.lock");
        match fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                writeln!(file, "pid={} started={:?}", std::process::id(), SystemTime::now())
                    .map_err(PersistenceError::Write)?;
                Ok(DeployLock { path })
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let existing = fs::read_to_string(&path).unwrap_or_default();
                let owner_pid = parse_owner_pid(&existing);
                // The lock owner is this process in the re-entrancy check
                // below.  On Windows `tasklist` can briefly omit a process
                // during startup/teardown, so never classify our own lock as
                // stale based on an external process listing.
                let owner_alive =
                    owner_pid.is_some_and(|pid| pid == std::process::id() || is_process_alive(pid));
                let stale = owner_pid.is_some() && !owner_alive;
                let malformed_old = owner_pid.is_none()
                    && fs::metadata(&path)
                        .and_then(|metadata| metadata.modified())
                        .ok()
                        .and_then(|modified| SystemTime::now().duration_since(modified).ok())
                        .is_some_and(|age| age > Duration::from_secs(60 * 60));
                if stale || malformed_old {
                    fs::remove_file(&path).map_err(PersistenceError::Write)?;
                    let mut file = fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(&path)
                        .map_err(|_| PersistenceError::ActiveDeployment(path.clone()))?;
                    writeln!(file, "pid={} started={:?}", std::process::id(), SystemTime::now())
                        .map_err(PersistenceError::Write)?;
                    Ok(DeployLock { path })
                } else {
                    Err(PersistenceError::ActiveDeployment(path))
                }
            }
            Err(error) => Err(PersistenceError::Write(error)),
        }
    }

    pub fn save(&self, run: &DeployRun) -> Result<(), PersistenceError> {
        fs::create_dir_all(self.paths.runs_root()).map_err(PersistenceError::Write)?;
        let bytes = serde_json::to_vec_pretty(run).map_err(PersistenceError::Serialize)?;
        atomic_write(&self.paths.run_path(&run.run_id), &bytes)?;
        atomic_write(&self.paths.current_path(), &bytes)
    }

    pub fn load_current(&self) -> Result<DeployRun, PersistenceError> {
        let path = self.paths.current_path();
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(primary) => {
                fs::read(backup_path(&path)).map_err(|_| PersistenceError::Read(primary))?
            }
        };
        serde_json::from_slice(&bytes).map_err(PersistenceError::Deserialize)
    }

    pub fn load_last_run(&self) -> Result<Option<DeployRun>, PersistenceError> {
        if !self.paths.current_path().exists() && !backup_path(&self.paths.current_path()).exists()
        {
            return Ok(None);
        }
        self.load_current().map(Some)
    }

    pub fn load_last_plan(&self) -> Result<Option<crate::domain::DeployPlan>, PersistenceError> {
        let path = self.paths.last_plan_path();
        if path.exists() || backup_path(&path).exists() {
            let bytes = match fs::read(&path) {
                Ok(bytes) => bytes,
                Err(primary) => {
                    fs::read(backup_path(&path)).map_err(|_| PersistenceError::Read(primary))?
                }
            };
            return serde_json::from_slice(&bytes).map(Some).map_err(PersistenceError::Deserialize);
        }
        self.load_last_run().map(|run| run.map(|run| run.plan))
    }

    /// Persist the most recently accepted deployment configuration separately
    /// from the resumable execution checkpoint. This lets the next wizard
    /// start restore the user's choices even when preflight blocks execution.
    pub fn save_last_plan(&self, plan: &crate::domain::DeployPlan) -> Result<(), PersistenceError> {
        fs::create_dir_all(&self.paths.state_root).map_err(PersistenceError::Write)?;
        let bytes = serde_json::to_vec_pretty(plan).map_err(PersistenceError::Serialize)?;
        atomic_write(&self.paths.last_plan_path(), &bytes)
    }

    pub fn has_resumable_run(&self) -> Result<bool, PersistenceError> {
        Ok(self.load_last_run()?.is_some_and(|run| run.is_resumable()))
    }

    pub fn append_event(
        &self,
        run: &DeployRun,
        event: impl AsRef<str>,
    ) -> Result<(), PersistenceError> {
        fs::create_dir_all(self.paths.runs_root()).map_err(PersistenceError::Write)?;
        let path = self.paths.runs_root().join(format!("{}.events.jsonl", run.run_id));
        let record = serde_json::json!({
            "runId": run.run_id,
            "stage": run.stage,
            "message": event.as_ref(),
        });
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(PersistenceError::Write)?;
        writeln!(file, "{}", serde_json::to_string(&record).map_err(PersistenceError::Serialize)?)
            .map_err(PersistenceError::Write)
    }
}

pub struct DeployLock {
    path: PathBuf,
}

impl Drop for DeployLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), PersistenceError> {
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, bytes).map_err(PersistenceError::Write)?;
    // Windows does not replace an existing destination with `rename`. Keep a
    // same-directory backup while replacing it; load_current can recover it if
    // the process is terminated between the two moves.
    if path.exists() {
        let backup = backup_path(path);
        if backup.exists() {
            fs::remove_file(&backup).map_err(PersistenceError::Write)?;
        }
        fs::rename(path, &backup).map_err(PersistenceError::Write)?;
        if let Err(error) = fs::rename(&temporary, path) {
            let _ = fs::rename(&backup, path);
            return Err(PersistenceError::Write(error));
        }
        let _ = fs::remove_file(backup);
        return Ok(());
    }
    fs::rename(temporary, path).map_err(PersistenceError::Write)
}

fn backup_path(path: &Path) -> PathBuf {
    path.with_extension("bak")
}

fn parse_owner_pid(contents: &str) -> Option<u32> {
    contents
        .lines()
        .find_map(|line| line.strip_prefix("pid=")?.split_whitespace().next())
        .and_then(|pid| pid.parse().ok())
}

#[cfg(windows)]
fn is_process_alive(pid: u32) -> bool {
    let filter = format!("PID eq {pid}");
    std::process::Command::new("tasklist")
        .args(["/FI", &filter, "/FO", "CSV", "/NH"])
        .output()
        .map(|output| {
            output.status.success()
                && String::from_utf8_lossy(&output.stdout).contains(&format!("\"{pid}\""))
        })
        .unwrap_or(true)
}

#[cfg(unix)]
fn is_process_alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()
        .is_ok_and(|status| status.success())
}

#[cfg(not(any(windows, unix)))]
fn is_process_alive(_pid: u32) -> bool {
    true
}

#[derive(Debug, Error)]
pub enum PersistenceError {
    #[error("could not resolve current directory: {0}")]
    CurrentDirectory(std::io::Error),
    #[error("could not locate Torca repository root from current directory")]
    RepoRoot,
    #[error("could not read deployment state: {0}")]
    Read(std::io::Error),
    #[error("could not write deployment state: {0}")]
    Write(std::io::Error),
    #[error("could not serialize deployment state: {0}")]
    Serialize(serde_json::Error),
    #[error("could not parse deployment state: {0}")]
    Deserialize(serde_json::Error),
    #[error("another Torca deployment is active (lock: {0})")]
    ActiveDeployment(PathBuf),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Configuration, DeployAction, DeployPlan, DeployStage, Target};

    #[test]
    fn last_plan_is_optional_and_terminal_runs_are_not_resumable() {
        let root = std::env::temp_dir().join(format!("torca-deploy-last-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let paths = DeployPaths { repo_root: root.clone(), state_root: root.join(".torca/deploy") };
        let store = StateStore::new(paths);
        assert!(store.load_last_plan().expect("missing state is valid").is_none());

        let mut run = DeployRun::new(DeployPlan::normal(
            DeployAction::RedeployCurrent,
            vec![Target::Windows],
            Configuration::Debug,
        ));
        store.save(&run).expect("save state");
        assert!(store.has_resumable_run().expect("read resumable state"));
        run.advance(DeployStage::Completed, "done");
        store.save(&run).expect("save completed state");
        assert!(!store.has_resumable_run().expect("read terminal state"));
        assert_eq!(
            store.load_last_plan().expect("load last plan").expect("plan").action,
            DeployAction::RedeployCurrent
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn explicit_last_plan_survives_without_creating_a_run() {
        let root =
            std::env::temp_dir().join(format!("torca-deploy-explicit-last-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let paths = DeployPaths { repo_root: root.clone(), state_root: root.join(".torca/deploy") };
        let store = StateStore::new(paths);
        let plan = DeployPlan::normal(
            DeployAction::FullRedeploy,
            vec![Target::Android],
            Configuration::Release,
        );

        store.save_last_plan(&plan).expect("save last plan");

        let restored = store.load_last_plan().expect("load last plan").expect("plan");
        assert_eq!(restored.action, DeployAction::FullRedeploy);
        assert_eq!(restored.targets, vec![Target::Android]);
        assert_eq!(restored.configuration, Configuration::Release);
        assert!(!store.has_resumable_run().expect("no run exists"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn state_round_trip_preserves_checkpoint() {
        let root = std::env::temp_dir().join(format!("torca-deploy-test-{}", std::process::id()));
        let paths = DeployPaths { repo_root: root.clone(), state_root: root.join(".torca/deploy") };
        let store = StateStore::new(paths);
        let run = DeployRun::new(DeployPlan::normal(
            DeployAction::Rebuild,
            vec![Target::Windows],
            Configuration::Debug,
        ));
        store.save(&run).expect("save state");
        let restored = store.load_current().expect("load state");
        assert_eq!(restored.run_id, run.run_id);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn state_load_recovers_from_interrupted_replacement_backup() {
        let root = std::env::temp_dir().join(format!("torca-deploy-backup-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let paths = DeployPaths { repo_root: root.clone(), state_root: root.join(".torca/deploy") };
        let store = StateStore::new(paths);
        let run = DeployRun::new(DeployPlan::normal(
            DeployAction::RunInstalled,
            vec![Target::Windows],
            Configuration::Debug,
        ));
        store.save(&run).expect("save state");
        let current = store.paths().current_path();
        fs::copy(&current, backup_path(&current)).expect("copy backup");
        fs::remove_file(current).expect("remove interrupted current");
        assert_eq!(store.load_current().expect("recover state").run_id, run.run_id);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn deployment_lock_prevents_concurrent_execution_and_releases_on_drop() {
        let root = std::env::temp_dir().join(format!("torca-deploy-lock-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let paths = DeployPaths { repo_root: root.clone(), state_root: root.join(".torca/deploy") };
        let store = StateStore::new(paths);
        let lock = store.acquire_lock().expect("first lock");
        assert!(matches!(store.acquire_lock(), Err(PersistenceError::ActiveDeployment(_))));
        drop(lock);
        assert!(store.acquire_lock().is_ok());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn stale_lock_owned_by_missing_process_is_reclaimed() {
        let root =
            std::env::temp_dir().join(format!("torca-deploy-stale-lock-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let paths = DeployPaths { repo_root: root.clone(), state_root: root.join(".torca/deploy") };
        fs::create_dir_all(&paths.state_root).expect("state directory");
        fs::write(paths.state_root.join("active.lock"), "pid=4294967294 started=unknown\n")
            .expect("stale lock");
        let store = StateStore::new(paths);
        assert!(store.acquire_lock().is_ok());
        let _ = fs::remove_dir_all(root);
    }
}
