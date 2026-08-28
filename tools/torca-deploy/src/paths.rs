use std::path::PathBuf;

use thiserror::Error;

#[derive(Clone, Debug)]
pub struct RuntimePaths {
    pub repo_root: PathBuf,
    pub runtime_root: PathBuf,
    pub stack_root: PathBuf,
    pub provider_endpoint_file: PathBuf,
    pub provider_ready: PathBuf,
    pub provider_status: PathBuf,
    pub relay_logs: PathBuf,
    pub docker_compose: PathBuf,
    pub artifacts: PathBuf,
    pub manifests: PathBuf,
    pub devices: PathBuf,
}

impl RuntimePaths {
    pub fn discover() -> Result<Self, PathError> {
        let mut root = std::env::current_dir().map_err(PathError::CurrentDirectory)?;
        loop {
            if root.join("Cargo.toml").is_file() && root.join("crates").is_dir() {
                return Ok(Self::from_repo(root));
            }
            if !root.pop() {
                return Err(PathError::RepositoryNotFound);
            }
        }
    }

    pub fn from_repo(repo_root: PathBuf) -> Self {
        let runtime_root = repo_root.join(".torca");
        let stack_root = runtime_root.join("stack");
        Self {
            repo_root: repo_root.clone(),
            runtime_root: runtime_root.clone(),
            stack_root: stack_root.clone(),
            provider_endpoint_file: stack_root.join("provider_endpoint_file.txt"),
            provider_ready: stack_root.join("provider_ready.txt"),
            provider_status: stack_root.join("provider_status.json"),
            relay_logs: runtime_root.join("logs"),
            docker_compose: repo_root.join("infra/docker/compose.yml"),
            artifacts: repo_root.join("artifacts"),
            manifests: runtime_root.join("manifests"),
            devices: runtime_root.join("devices"),
        }
    }

    pub fn ensure(&self) -> Result<(), PathError> {
        for path in
            [&self.runtime_root, &self.stack_root, &self.relay_logs, &self.manifests, &self.devices]
        {
            std::fs::create_dir_all(path)
                .map_err(|source| PathError::Create { path: path.clone(), source })?;
        }
        Ok(())
    }

    pub fn endpoint(&self) -> Option<String> {
        std::fs::read_to_string(&self.provider_endpoint_file)
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
    }

    pub fn new_incident_dir(&self) -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        self.relay_logs.join("incidents").join(format!("{stamp}-{}", std::process::id()))
    }

    pub fn validate_endpoint(endpoint: &str) -> bool {
        let mut parts = endpoint.split(':');
        let host = parts.next().unwrap_or_default();
        let port = parts.next().unwrap_or_default();
        let (prefix, suffix_ok) =
            host.strip_suffix(".onion").map_or(("", false), |prefix| (prefix, true));
        parts.next().is_none()
            && suffix_ok
            && prefix.len() == 56
            && prefix.chars().all(|c| "abcdefghijklmnopqrstuvwxyz234567".contains(c))
            && port.parse::<u16>().is_ok()
    }
}

#[derive(Debug, Error)]
pub enum PathError {
    #[error("could not resolve current directory: {0}")]
    CurrentDirectory(std::io::Error),
    #[error("could not locate Torca repository root")]
    RepositoryNotFound,
    #[error("could not create runtime directory {path}: {source}")]
    Create { path: PathBuf, source: std::io::Error },
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn endpoint_validation_is_strict() {
        assert!(RuntimePaths::validate_endpoint(&format!("{}.onion:443", "a".repeat(56))));
        assert!(!RuntimePaths::validate_endpoint("short.onion:443"));
        assert!(!RuntimePaths::validate_endpoint("a.onion:443:extra"));
    }
}
