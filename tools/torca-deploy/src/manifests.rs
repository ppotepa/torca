use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::devices::Device;
use crate::domain::{CommunicationProvider, Configuration};
use crate::paths::RuntimePaths;

/// Projects the canonical client artifact manifest into the legacy build and
/// per-device manifests. This keeps older diagnostics/scripts readable while
/// ensuring every view is derived from the same endpoint and build identity.
pub fn synchronize(
    paths: &RuntimePaths,
    devices: &[Device],
    configuration: Configuration,
    provider: CommunicationProvider,
    endpoint: Option<&str>,
    provider_profile: Option<&str>,
) -> Result<(), ManifestError> {
    let mode = configuration.to_string();
    let canonical_path = paths.manifests.join(format!("clients-{mode}.json"));
    let canonical: Value = serde_json::from_slice(
        &fs::read(&canonical_path)
            .map_err(|source| ManifestError::Read { path: canonical_path.clone(), source })?,
    )
    .map_err(|source| ManifestError::Decode { path: canonical_path.clone(), source })?;
    let recorded_provider = canonical["communicationProvider"].as_str().ok_or_else(|| {
        ManifestError::MissingField { path: canonical_path.clone(), field: "communicationProvider" }
    })?;
    if recorded_provider != provider.wire_value() {
        return Err(ManifestError::ProviderMismatch {
            expected: provider.wire_value().to_owned(),
            actual: recorded_provider.to_owned(),
        });
    }
    if provider == CommunicationProvider::Iroh {
        let expected_profile = normalized_iroh_profile(provider_profile);
        let recorded_profile = canonical["irohProfile"].as_str().unwrap_or("always");
        if recorded_profile != expected_profile.as_str() {
            return Err(ManifestError::ProviderProfileMismatch {
                expected: expected_profile,
                actual: recorded_profile.to_owned(),
            });
        }
        let expected_services = crate::build::iroh_service_config_fingerprint();
        let recorded_services =
            canonical["irohServiceConfigFingerprint"].as_str().unwrap_or("default");
        if recorded_services != expected_services {
            return Err(ManifestError::ProviderServiceConfigMismatch {
                expected: expected_services,
                actual: recorded_services.to_owned(),
            });
        }
    }
    let manifest_endpoint = canonical["endpoint"].as_str();
    if manifest_endpoint != endpoint {
        return Err(ManifestError::EndpointMismatch {
            expected: endpoint.map(str::to_owned),
            actual: manifest_endpoint.map(str::to_owned),
        });
    }

    let release_path = paths.repo_root.join("release/version.json");
    let release: Value = serde_json::from_slice(
        &fs::read(&release_path)
            .map_err(|source| ManifestError::Read { path: release_path.clone(), source })?,
    )
    .map_err(|source| ManifestError::Decode { path: release_path, source })?;

    let legacy_build = json!({
        "Schema": 2,
        "CommunicationProvider": provider.wire_value(),
        "IrohProfile": iroh_profile_value(provider, provider_profile),
        "IrohServiceConfigFingerprint": (provider == CommunicationProvider::Iroh)
            .then(crate::build::iroh_service_config_fingerprint),
        "Endpoint": endpoint,
        "Targets": canonical["targets"],
        "Configuration": mode,
        "SourceFingerprint": canonical["buildId"],
        "BuildId": canonical["buildId"],
        "ContractVersion": release["contractSchema"],
        "Commit": canonical["sourceCommit"],
        "BuiltAt": canonical["builtAt"],
    });
    atomic_json(&paths.runtime_root.join("build-manifest.json"), &legacy_build)?;

    for device in devices {
        synchronize_device(
            paths,
            device,
            configuration,
            provider,
            endpoint,
            provider_profile,
            &canonical,
            &release,
        )?;
    }
    Ok(())
}

fn synchronize_device(
    paths: &RuntimePaths,
    device: &Device,
    configuration: Configuration,
    provider: CommunicationProvider,
    endpoint: Option<&str>,
    provider_profile: Option<&str>,
    canonical: &Value,
    release: &Value,
) -> Result<(), ManifestError> {
    let platform = device.target.to_string();
    let mut matching = fs::read_dir(&paths.devices)
        .map_err(|source| ManifestError::Read { path: paths.devices.clone(), source })?
        .flatten()
        .filter(|entry| entry.path().extension().is_some_and(|extension| extension == "json"))
        .filter_map(|entry| {
            let path = entry.path();
            let value: Value = serde_json::from_slice(&fs::read(&path).ok()?).ok()?;
            (value["Platform"].as_str() == Some(platform.as_str())).then_some((path, value))
        })
        .collect::<Vec<_>>();
    if matching.is_empty() {
        matching.push((paths.devices.join(format!("{}.json", safe_name(&device.id))), json!({})));
    }
    for (path, previous) in matching {
        let mut object = previous.as_object().cloned().unwrap_or_else(Map::new);
        object.insert("Schema".into(), json!(2));
        object.insert("DeviceId".into(), json!(device.id));
        object.entry("DeviceName").or_insert_with(|| json!(device.id));
        object.insert("Platform".into(), json!(platform));
        object.insert("ProductVersion".into(), release["version"].clone());
        object.insert("BuildNumber".into(), release["build"].clone());
        object.insert("BuildId".into(), canonical["buildId"].clone());
        object.insert("Configuration".into(), json!(configuration.to_string()));
        object.insert("CommunicationProvider".into(), json!(provider.wire_value()));
        object.insert("IrohProfile".into(), iroh_profile_value(provider, provider_profile));
        object.insert(
            "IrohServiceConfigFingerprint".into(),
            (provider == CommunicationProvider::Iroh)
                .then(crate::build::iroh_service_config_fingerprint)
                .map_or(Value::Null, Value::String),
        );
        object.insert("StorageEpoch".into(), release["storageEpoch"].clone());
        object.insert("SchemaVersion".into(), release["schemaVersion"].clone());
        object.insert("ContractSchema".into(), release["contractSchema"].clone());
        object.insert("WireVersion".into(), release["wireVersion"].clone());
        // Keep legacy keys for tooling, but make their absence explicit for
        // direct providers such as Iroh instead of retaining a stale Tor URL.
        object.insert("RelayEndpoint".into(), endpoint.map_or(Value::Null, |value| json!(value)));
        object.insert(
            "RelayEndpointHash".into(),
            endpoint.map_or(Value::Null, |value| json!(format!("{:x}", Sha256::digest(value)))),
        );
        object
            .insert("ProviderEndpoint".into(), endpoint.map_or(Value::Null, |value| json!(value)));
        object.insert(
            "DeployedAtMs".into(),
            json!(SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis()),
        );
        object.insert("Verified".into(), json!(true));
        atomic_json(&path, &Value::Object(object))?;
    }
    Ok(())
}

fn safe_name(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn iroh_profile_value(provider: CommunicationProvider, explicit: Option<&str>) -> Value {
    if provider != CommunicationProvider::Iroh {
        return Value::Null;
    }
    json!(normalized_iroh_profile(explicit))
}

fn normalized_iroh_profile(explicit: Option<&str>) -> String {
    explicit
        .map(str::to_owned)
        .or_else(|| std::env::var("TORCA_IROH_PROFILE").ok())
        .map(|value| value.trim().to_ascii_lowercase())
        .and_then(|value| match value.as_str() {
            "direct" | "direct-only" => Some("direct"),
            "local" | "local-only" => Some("local"),
            "always" | "always-reachable" => Some("always"),
            _ => None,
        })
        .unwrap_or("always")
        .to_owned()
}

fn atomic_json(path: &Path, value: &Value) -> Result<(), ManifestError> {
    let bytes = serde_json::to_vec_pretty(value).map_err(ManifestError::Encode)?;
    let temporary = path.with_extension("tmp");
    let backup = path.with_extension("bak");
    fs::write(&temporary, bytes)
        .map_err(|source| ManifestError::Write { path: temporary.clone(), source })?;
    if path.exists() {
        if backup.exists() {
            fs::remove_file(&backup)
                .map_err(|source| ManifestError::Write { path: backup.clone(), source })?;
        }
        fs::rename(path, &backup)
            .map_err(|source| ManifestError::Write { path: backup.clone(), source })?;
        if let Err(source) = fs::rename(&temporary, path) {
            let _ = fs::rename(&backup, path);
            return Err(ManifestError::Write { path: path.to_path_buf(), source });
        }
        let _ = fs::remove_file(backup);
        return Ok(());
    }
    fs::rename(&temporary, path)
        .map_err(|source| ManifestError::Write { path: path.to_path_buf(), source })
}

#[derive(Debug, Error)]
pub enum ManifestError {
    #[error("could not read manifest {path}: {source}")]
    Read { path: PathBuf, source: std::io::Error },
    #[error("could not decode manifest {path}: {source}")]
    Decode { path: PathBuf, source: serde_json::Error },
    #[error("manifest {path} is missing required field {field}")]
    MissingField { path: PathBuf, field: &'static str },
    #[error("could not encode manifest: {0}")]
    Encode(serde_json::Error),
    #[error("could not write manifest {path}: {source}")]
    Write { path: PathBuf, source: std::io::Error },
    #[error("artifact endpoint mismatch; expected {expected:?}, found {actual:?}")]
    EndpointMismatch { expected: Option<String>, actual: Option<String> },
    #[error("artifact provider mismatch; expected {expected}, found {actual}")]
    ProviderMismatch { expected: String, actual: String },
    #[error("artifact provider profile mismatch; expected {expected}, found {actual}")]
    ProviderProfileMismatch { expected: String, actual: String },
    #[error(
        "artifact provider service configuration mismatch; expected {expected}, found {actual}"
    )]
    ProviderServiceConfigMismatch { expected: String, actual: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Target;

    #[test]
    fn all_manifest_views_use_the_canonical_endpoint() {
        let root = std::env::temp_dir().join(format!("torca-manifests-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let paths = RuntimePaths::from_repo(root.clone());
        paths.ensure().expect("runtime paths");
        fs::create_dir_all(root.join("release")).expect("release directory");
        fs::write(
            root.join("release/version.json"),
            r#"{"version":"1.0.0","build":2,"contractSchema":3,"wireVersion":4,"storageEpoch":5,"schemaVersion":6}"#,
        )
        .expect("release metadata");
        let endpoint = format!("{}.onion:443", "a".repeat(56));
        fs::write(
            paths.manifests.join("clients-debug.json"),
            serde_json::to_vec(&json!({
                "endpoint": endpoint,
                "communicationProvider": "tor",
                "targets": ["windows"],
                "buildId": "BUILD",
                "sourceCommit": "COMMIT",
                "builtAt": "NOW"
            }))
            .expect("canonical manifest"),
        )
        .expect("canonical manifest");
        fs::write(
            paths.devices.join("windows.json"),
            r#"{"Platform":"windows","RelayEndpoint":"stale"}"#,
        )
        .expect("old device manifest");
        let devices = [Device { target: Target::Windows, id: "desktop".into(), android_abi: None }];
        synchronize(
            &paths,
            &devices,
            Configuration::Debug,
            CommunicationProvider::Tor,
            Some(&endpoint),
            None,
        )
        .expect("synchronize");
        let device: Value = serde_json::from_slice(
            &fs::read(paths.devices.join("windows.json")).expect("device manifest"),
        )
        .expect("device json");
        let build: Value = serde_json::from_slice(
            &fs::read(paths.runtime_root.join("build-manifest.json")).expect("build manifest"),
        )
        .expect("build json");
        assert_eq!(device["RelayEndpoint"], endpoint);
        assert_eq!(build["Endpoint"], endpoint);
        assert_eq!(device["BuildId"], "BUILD");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn direct_provider_clears_legacy_endpoint_fields() {
        let root =
            std::env::temp_dir().join(format!("torca-manifests-iroh-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let paths = RuntimePaths::from_repo(root.clone());
        paths.ensure().expect("runtime paths");
        fs::create_dir_all(root.join("release")).expect("release directory");
        fs::write(
            root.join("release/version.json"),
            r#"{"version":"1.0.0","build":2,"contractSchema":3,"wireVersion":4,"storageEpoch":5,"schemaVersion":6}"#,
        )
        .expect("release metadata");
        fs::write(
            paths.manifests.join("clients-debug.json"),
            serde_json::to_vec(&json!({
                "communicationProvider": "iroh",
                "irohProfile": "direct",
                "endpoint": null,
                "targets": ["windows"],
                "buildId": "BUILD",
                "sourceCommit": "COMMIT",
                "builtAt": "NOW"
            }))
            .expect("canonical manifest"),
        )
        .expect("manifest");
        let devices = [Device { target: Target::Windows, id: "desktop".into(), android_abi: None }];
        synchronize(
            &paths,
            &devices,
            Configuration::Debug,
            CommunicationProvider::Iroh,
            None,
            Some("direct"),
        )
        .expect("synchronize direct provider");
        let device: Value = serde_json::from_slice(
            &fs::read(paths.devices.join("desktop.json")).expect("device manifest"),
        )
        .expect("device json");
        let build: Value = serde_json::from_slice(
            &fs::read(paths.runtime_root.join("build-manifest.json")).expect("build json"),
        )
        .expect("build json");
        assert_eq!(device["CommunicationProvider"], "iroh");
        assert!(device["RelayEndpoint"].is_null());
        assert!(build["Endpoint"].is_null());
        let _ = fs::remove_dir_all(root);
    }
}
