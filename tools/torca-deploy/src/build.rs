use crate::devices::{AndroidAbi, Device};
use crate::domain::{BuildPolicy, Configuration, Target};
use crate::paths::RuntimePaths;
use crate::process::{CommandRunner, CommandSpec, ProcessError};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::path::{Path, PathBuf};
use std::time::Duration;
use thiserror::Error;

pub struct BuildController<'a> {
    paths: &'a RuntimePaths,
    runner: &'a dyn CommandRunner,
}

/// Verify that an artifact belongs to the requested target/configuration and
/// has not changed since the Rust build manifest was written.
pub fn verify_artifact_manifest(
    paths: &RuntimePaths,
    target: Target,
    configuration: Configuration,
    artifact: &Path,
) -> Result<(), String> {
    let mode = artifact_mode(configuration);
    let manifest_path = paths.manifests.join(format!("clients-{mode}.json"));
    let target_name = target.to_string();
    let content = std::fs::read_to_string(&manifest_path)
        .map_err(|error| format!("read artifact manifest {}: {error}", manifest_path.display()))?;
    let manifest: serde_json::Value = serde_json::from_str(&content)
        .map_err(|error| format!("parse artifact manifest {}: {error}", manifest_path.display()))?;
    let current_endpoint = paths.endpoint().ok_or_else(|| {
        format!(
            "current relay endpoint is unavailable; refusing to reuse artifact {}",
            artifact.display()
        )
    })?;
    let recorded_endpoint =
        manifest.get("endpoint").and_then(serde_json::Value::as_str).ok_or_else(|| {
            format!(
                "artifact manifest {} does not record the relay endpoint",
                manifest_path.display()
            )
        })?;
    if recorded_endpoint != current_endpoint {
        return Err(format!(
            "artifact endpoint mismatch for {}: manifest={}, current={}",
            artifact.display(),
            recorded_endpoint,
            current_endpoint
        ));
    }
    let expected = manifest
        .get("artifacts")
        .and_then(serde_json::Value::as_array)
        .and_then(|artifacts| {
            artifacts.iter().find(|entry| {
                entry.get("target").and_then(serde_json::Value::as_str)
                    == Some(target_name.as_str())
                    && entry.get("path").and_then(serde_json::Value::as_str).is_some_and(
                        |recorded| artifact_paths_match(Path::new(recorded), artifact, target),
                    )
            })
        })
        .and_then(|entry| entry.get("sha256"))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            format!(
                "artifact {} is not recorded for {} {}",
                artifact.display(),
                target,
                configuration
            )
        })?;
    let actual = hash_file(artifact)
        .ok_or_else(|| format!("artifact does not exist: {}", artifact.display()))?;
    if actual != expected {
        return Err(format!(
            "artifact hash mismatch for {}: expected {}, found {}",
            artifact.display(),
            expected,
            actual
        ));
    }
    Ok(())
}

fn artifact_paths_match(recorded: &Path, requested: &Path, target: Target) -> bool {
    fn normalized(path: &Path) -> String {
        std::fs::canonicalize(path)
            .unwrap_or_else(|_| path.to_path_buf())
            .to_string_lossy()
            .replace('\\', "/")
    }

    let recorded = normalized(recorded);
    let requested = normalized(requested);
    if matches!(target, Target::Windows) {
        recorded.eq_ignore_ascii_case(&requested)
    } else {
        recorded == requested
    }
}

/// SOAK2 uses a separate Android application id and data namespace while
/// retaining the debug native profile. The environment is set only by the
/// soak launcher and is intentionally not part of ordinary deploy plans.
pub fn soak_flavor_enabled() -> bool {
    env::var("TORCA_SOAK_FLAVOR")
        .is_ok_and(|value| value == "1" || value.eq_ignore_ascii_case("true"))
}

fn artifact_mode(configuration: Configuration) -> &'static str {
    if soak_flavor_enabled() && matches!(configuration, Configuration::Debug) {
        "soak"
    } else {
        match configuration {
            Configuration::Debug => "debug",
            Configuration::Release => "release",
        }
    }
}
impl<'a> BuildController<'a> {
    pub fn new(paths: &'a RuntimePaths, runner: &'a dyn CommandRunner) -> Self {
        Self { paths, runner }
    }
    pub fn build(
        &self,
        targets: &[Target],
        devices: &[Device],
        configuration: Configuration,
        policy: BuildPolicy,
        endpoint: Option<&str>,
    ) -> Result<(), BuildError> {
        if matches!(policy, BuildPolicy::Reuse) {
            return Ok(());
        }
        let endpoint = endpoint.ok_or(BuildError::MissingEndpoint)?;
        if !RuntimePaths::validate_endpoint(endpoint) {
            return Err(BuildError::InvalidEndpoint(endpoint.into()));
        }
        let mode = match configuration {
            Configuration::Debug => "debug",
            Configuration::Release => "release",
        };
        if targets.contains(&Target::Windows) {
            crate::windows_client::WorkspaceWindowsClient::new(self.paths, self.runner)
                .stop()
                .map_err(BuildError::StopRunningClient)?;
            let mut cargo_args = vec!["build", "-p", "torca-native", "--locked"];
            if matches!(configuration, Configuration::Release) {
                cargo_args.push("--release");
            }
            self.command_with_env("cargo", &cargo_args, endpoint, &self.paths.repo_root)?;
        }
        if targets.contains(&Target::Android) {
            let android_abis = selected_android_abis(devices);
            self.build_android_native(configuration, endpoint, &android_abis)?;
            let profile =
                if matches!(configuration, Configuration::Release) { "release" } else { "debug" };
            for target in android_targets(&android_abis) {
                let abi = target.abi.package_name();
                let triple = target.triple;
                let source = cargo_target_root(&self.paths.repo_root)
                    .join(triple)
                    .join(profile)
                    .join("libtorca_native.so");
                let destination = self.paths.repo_root.join(format!(
                    "apps/client/flutter/android/app/src/main/jniLibs/{abi}/libtorca_native.so"
                ));
                if source.is_file() {
                    if let Some(parent) = destination.parent() {
                        std::fs::create_dir_all(parent).map_err(BuildError::Io)?;
                    }
                    std::fs::copy(source, destination).map_err(BuildError::Io)?;
                } else {
                    return Err(BuildError::NativeArtifactMissing(source));
                }
            }
        }
        if targets.contains(&Target::Windows) {
            let define = format!("TORCA_RELAY_ENDPOINT={endpoint}");
            let flutter = flutter_program()?;
            let flutter_args = ["build", "windows", &format!("--{mode}"), "--dart-define", &define];
            let environment =
                [("TORCA_RELAY_ENDPOINT".to_owned(), endpoint.to_owned())].into_iter().collect();
            let output = self.runner.run(&CommandSpec {
                program: flutter.clone(),
                arguments: flutter_args.iter().map(|arg| (*arg).into()).collect(),
                working_directory: self.paths.repo_root.join("apps/client/flutter"),
                timeout: Duration::from_secs(3600),
                environment,
            })?;
            if !output.success {
                let diagnostic = self.windows_install_diagnostic(mode);
                return Err(BuildError::WindowsFlutter { output: output.text, diagnostic });
            }
            let source =
                cargo_target_root(&self.paths.repo_root).join(mode).join("torca_native.dll");
            let destination = self.paths.repo_root.join(format!(
                "apps/client/flutter/build/windows/x64/runner/{mode}/torca_native.dll"
            ));
            if source.is_file() {
                std::fs::copy(source, destination).map_err(BuildError::Io)?;
            } else {
                return Err(BuildError::NativeArtifactMissing(source));
            }
        }
        if targets.contains(&Target::Android) {
            let define = format!("TORCA_RELAY_ENDPOINT={endpoint}");
            let flutter = flutter_program()?;
            let target_platforms =
                flutter_target_platforms(&selected_android_abis(devices)).to_owned();
            let mut flutter_args = vec![
                "build".to_owned(),
                "apk".to_owned(),
                format!("--{mode}"),
                "--split-per-abi".to_owned(),
                "--target-platform".to_owned(),
                target_platforms,
                "--dart-define".to_owned(),
                define,
            ];
            if soak_flavor_enabled() && matches!(configuration, Configuration::Debug) {
                flutter_args.extend([
                    "--flavor".to_owned(),
                    "soak".to_owned(),
                    "--dart-define".to_owned(),
                    "TORCA_SOAK_MODE=true".to_owned(),
                ]);
            } else {
                // Explicitly select the normal flavor so introducing the
                // isolated soak flavor never changes normal artifact paths or
                // package identity implicitly.
                flutter_args.extend(["--flavor".to_owned(), "normal".to_owned()]);
            }
            let references = flutter_args.iter().map(String::as_str).collect::<Vec<_>>();
            self.command_with_env(
                &flutter,
                &references,
                endpoint,
                &self.paths.repo_root.join("apps/client/flutter"),
            )?;
        }
        let mut artifacts = Vec::new();
        for target in targets {
            let paths_for_target = match target {
                Target::Windows => vec![self.paths.repo_root.join(format!(
                    "apps/client/flutter/build/windows/x64/runner/{mode}/torca_app.exe"
                ))],
                Target::Android => selected_android_abis(devices)
                    .into_iter()
                    .map(|abi| {
                        if soak_flavor_enabled() && matches!(configuration, Configuration::Debug) {
                            self.paths.repo_root.join(format!(
                                "apps/client/flutter/build/app/outputs/flutter-apk/app-{}-soak-debug.apk",
                                abi.package_name()
                            ))
                        } else {
                            self.paths.repo_root.join(format!(
                                "apps/client/flutter/build/app/outputs/flutter-apk/app-{}-normal-{mode}.apk",
                                abi.package_name()
                            ))
                        }
                    })
                    .collect(),
            };
            for path in paths_for_target {
                artifacts.push(serde_json::json!({
                    "target": target.to_string(),
                    "path": path,
                    "sha256": hash_file(&path),
                }));
            }
        }
        let source_commit = self.source_commit();
        let build_id = build_identity(&source_commit, endpoint, configuration, targets);
        let manifest = serde_json::json!({
            "configuration": configuration.to_string(),
            "targets": targets.iter().map(ToString::to_string).collect::<Vec<_>>(),
            "endpoint": endpoint,
            "buildId": build_id,
            "sourceCommit": source_commit,
            "artifacts": artifacts,
            "builtAt": format!("{:?}", std::time::SystemTime::now()),
        });
        std::fs::write(
            self.paths.manifests.join(format!("clients-{}.json", artifact_mode(configuration))),
            serde_json::to_vec_pretty(&manifest).map_err(BuildError::Serialize)?,
        )
        .map_err(BuildError::Io)?;
        Ok(())
    }

    fn source_commit(&self) -> String {
        self.runner
            .run_quiet(&CommandSpec {
                program: "git".into(),
                arguments: vec!["rev-parse".into(), "HEAD".into()],
                working_directory: self.paths.repo_root.clone(),
                timeout: Duration::from_secs(15),
                environment: BTreeMap::new(),
            })
            .ok()
            .filter(|output| output.success)
            .map(|output| output.text.trim().to_owned())
            .filter(|commit| !commit.is_empty())
            .unwrap_or_else(|| "unknown".into())
    }

    fn build_android_native(
        &self,
        configuration: Configuration,
        endpoint: &str,
        abis: &[AndroidAbi],
    ) -> Result<(), BuildError> {
        let toolchain = AndroidToolchain::discover()?;
        for target in android_targets(abis) {
            let mut arguments = vec![
                "build".to_owned(),
                "-p".to_owned(),
                "torca-native".to_owned(),
                "--target".to_owned(),
                target.triple.to_owned(),
                "--locked".to_owned(),
            ];
            if matches!(configuration, Configuration::Release) {
                arguments.push("--release".to_owned());
            }
            let output = self.runner.run(&CommandSpec {
                program: "cargo".into(),
                arguments,
                working_directory: self.paths.repo_root.clone(),
                timeout: Duration::from_secs(3600),
                environment: toolchain.environment(target, endpoint),
            })?;
            if !output.success {
                return Err(BuildError::Command {
                    program: format!("cargo build {}", target.triple),
                    output: output.text,
                });
            }
        }
        Ok(())
    }
    fn command_with_env(
        &self,
        program: &str,
        args: &[&str],
        endpoint: &str,
        working_directory: &Path,
    ) -> Result<(), BuildError> {
        let environment =
            [("TORCA_RELAY_ENDPOINT".to_owned(), endpoint.to_owned())].into_iter().collect();
        let output = self.runner.run(&CommandSpec {
            program: program.into(),
            arguments: args.iter().map(|x| (*x).into()).collect(),
            working_directory: working_directory.to_path_buf(),
            timeout: Duration::from_secs(3600),
            environment,
        })?;
        if output.success {
            Ok(())
        } else {
            Err(BuildError::Command { program: program.into(), output: output.text })
        }
    }

    /// Flutter collapses CMake's real install failure into a generic MSB3073.
    /// Re-running only CMake's generated install script is safe and exposes
    /// the locked file or missing artifact that actually caused the failure.
    fn windows_install_diagnostic(&self, mode: &str) -> String {
        let build_root = self.paths.repo_root.join("apps/client/flutter/build/windows/x64");
        let install = build_root.join("cmake_install.cmake");
        let cache = build_root.join("CMakeCache.txt");
        if !install.is_file() || !cache.is_file() {
            return "CMake install script is unavailable; no additional diagnostic was produced."
                .into();
        }
        let Some(cache_contents) = std::fs::read_to_string(cache).ok() else {
            return "CMakeCache.txt could not be read.".into();
        };
        let Some(command) = cache_contents
            .lines()
            .find_map(|line| line.strip_prefix("CMAKE_COMMAND:INTERNAL="))
            .map(str::to_owned)
        else {
            return "CMake executable was not found in CMakeCache.txt.".into();
        };
        match self.runner.run(&CommandSpec {
            program: command,
            arguments: vec![
                format!("-DBUILD_TYPE={mode}"),
                "-P".into(),
                "cmake_install.cmake".into(),
            ],
            working_directory: build_root,
            timeout: Duration::from_secs(60),
            environment: BTreeMap::new(),
        }) {
            Ok(output) => output.text,
            Err(error) => format!("Could not run generated CMake install diagnostic: {error}"),
        }
    }
}

#[derive(Clone, Copy)]
struct AndroidTarget {
    abi: AndroidAbi,
    triple: &'static str,
    linker: &'static str,
}

fn selected_android_abis(devices: &[Device]) -> Vec<AndroidAbi> {
    let mut selected = devices.iter().filter_map(|device| device.android_abi).collect::<Vec<_>>();
    selected.sort_by_key(|abi| match abi {
        AndroidAbi::Arm64 => 0,
        AndroidAbi::X86_64 => 1,
    });
    selected.dedup();
    if selected.is_empty() { vec![AndroidAbi::Arm64, AndroidAbi::X86_64] } else { selected }
}

fn android_targets(abis: &[AndroidAbi]) -> Vec<AndroidTarget> {
    abis.iter()
        .map(|abi| match abi {
            AndroidAbi::Arm64 => AndroidTarget {
                abi: *abi,
                triple: "aarch64-linux-android",
                // CPAL's Android AAudio backend requires API 26.
                linker: "aarch64-linux-android26-clang.cmd",
            },
            AndroidAbi::X86_64 => AndroidTarget {
                abi: *abi,
                triple: "x86_64-linux-android",
                linker: "x86_64-linux-android26-clang.cmd",
            },
        })
        .collect()
}

fn flutter_target_platforms(abis: &[AndroidAbi]) -> &'static str {
    match abis {
        [AndroidAbi::Arm64] => "android-arm64",
        [AndroidAbi::X86_64] => "android-x64",
        [AndroidAbi::Arm64, AndroidAbi::X86_64] | [AndroidAbi::X86_64, AndroidAbi::Arm64] => {
            "android-arm64,android-x64"
        }
        _ => "android-arm64,android-x64",
    }
}

struct AndroidToolchain {
    ndk_bin: PathBuf,
    unix_perl: PathBuf,
}

impl AndroidToolchain {
    fn discover() -> Result<Self, BuildError> {
        let ndk_root = android_ndk_root().ok_or(BuildError::AndroidNdkUnavailable)?;
        let ndk_bin = ndk_root.join("toolchains/llvm/prebuilt/windows-x86_64/bin");
        if !ndk_bin.join("clang.exe").is_file() {
            return Err(BuildError::AndroidNdkClangUnavailable(ndk_bin));
        }
        let unix_perl = [
            PathBuf::from(r"C:\msys64\usr\bin\perl.exe"),
            PathBuf::from(r"C:\Program Files\Git\usr\bin\perl.exe"),
        ]
        .into_iter()
        .find(|path| path.is_file())
        .ok_or(BuildError::UnixPerlUnavailable)?;
        Ok(Self { ndk_bin, unix_perl })
    }

    fn environment(&self, target: AndroidTarget, endpoint: &str) -> BTreeMap<String, String> {
        let mut environment = BTreeMap::new();
        let existing_path = env::var_os("PATH").unwrap_or_default();
        let perl_dir = self.unix_perl.parent().expect("perl has a parent");
        // `PATH` is a list of components, not one path.  Treating the whole
        // inherited value as a single component silently discarded the
        // MSYS/Git Perl precedence and let a native Windows Perl get selected
        // by openssl-sys (which then rejects Unix-style paths).
        let mut path_components = vec![self.ndk_bin.clone(), perl_dir.to_path_buf()];
        path_components.extend(env::split_paths(&existing_path));
        let path = env::join_paths(path_components).expect("Windows PATH components are valid");
        environment.insert("PATH".into(), path.to_string_lossy().into_owned());
        // OpenSSL writes this value into a Makefile evaluated by MSYS `sh`.
        // An absolute Windows path would lose its backslashes there, so keep a
        // portable command name while placing its verified MSYS/Git directory
        // at the beginning of PATH.
        environment.insert("PERL".into(), "perl".into());
        environment.insert("TORCA_RELAY_ENDPOINT".into(), endpoint.into());
        // Keep compiler variables as portable command names.  `openssl-sys`
        // writes `CC_<target>` into a Makefile executed by MSYS sh; an
        // absolute Windows path is converted to `C:Android...clang.exe` and
        // fails before the first C file is compiled.  The verified NDK bin
        // directory is first on PATH, so these names still resolve to the
        // intended compiler and archiver.
        environment.insert(format!("CC_{}", target.triple), target.linker.into());
        environment.insert(format!("CC_{}", target.triple.replace('-', "_")), target.linker.into());
        environment.insert(format!("AR_{}", target.triple), "llvm-ar".into());
        environment.insert(format!("AR_{}", target.triple.replace('-', "_")), "llvm-ar".into());
        environment.insert(format!("RANLIB_{}", target.triple), "llvm-ranlib".into());
        environment
            .insert(format!("RANLIB_{}", target.triple.replace('-', "_")), "llvm-ranlib".into());
        environment.insert(
            format!("CARGO_TARGET_{}_LINKER", target.triple.replace('-', "_").to_uppercase()),
            // Keep this portable as well.  Cargo propagates the linker value
            // to openssl-sys; an absolute Windows path is later consumed by
            // MSYS `sh`, which strips the backslashes and produces the
            // unusable `C:Android...` form.  The verified NDK directory is
            // already first on PATH.
            target.linker.into(),
        );
        // Do not add a CFLAGS target here. The API-qualified wrapper above
        // already selects it. Adding another `--target` causes OpenSSL's
        // Configure step to see contradictory API levels when Cargo/NDK has
        // provided its own default.
        environment
    }
}

fn android_ndk_root() -> Option<PathBuf> {
    ["ANDROID_NDK_HOME", "ANDROID_NDK_ROOT"]
        .into_iter()
        .filter_map(|key| env::var_os(key).map(PathBuf::from))
        .find(|path| path.is_dir())
        .or_else(|| {
            [env::var_os("ANDROID_HOME"), env::var_os("ANDROID_SDK_ROOT")]
                .into_iter()
                .flatten()
                .map(PathBuf::from)
                .find_map(|sdk_root| newest_ndk(&sdk_root))
        })
        .or_else(|| newest_ndk(Path::new(r"C:\Android\android-sdk")))
}

fn newest_ndk(sdk_root: &Path) -> Option<PathBuf> {
    let ndk_root = sdk_root.join("ndk");
    let mut candidates = std::fs::read_dir(ndk_root)
        .ok()?
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    candidates.sort();
    candidates.pop()
}

fn flutter_program() -> Result<String, BuildError> {
    let configured =
        env::var_os("FLUTTER_ROOT").map(PathBuf::from).map(|root| root.join("bin/flutter.bat"));
    let from_path = env::var_os("PATH").and_then(|path| {
        env::split_paths(&path)
            .map(|directory| directory.join("flutter.bat"))
            .find(|candidate| candidate.is_file())
    });
    configured
        .filter(|candidate| candidate.is_file())
        .or(from_path)
        .or_else(|| {
            let candidate = PathBuf::from(r"C:\tools\flutter\bin\flutter.bat");
            candidate.is_file().then_some(candidate)
        })
        .map(|path| path.to_string_lossy().into_owned())
        .ok_or(BuildError::FlutterUnavailable)
}

fn hash_file(path: &std::path::Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    Some(format!("{:x}", Sha256::digest(bytes)))
}

fn build_identity(
    source_commit: &str,
    endpoint: &str,
    configuration: Configuration,
    targets: &[Target],
) -> String {
    let target_list = targets.iter().map(ToString::to_string).collect::<Vec<_>>().join(",");
    let material = format!("{source_commit}\n{endpoint}\n{configuration}\n{target_list}");
    format!("{:X}", Sha256::digest(material.as_bytes()))
}

fn cargo_target_root(repo_root: &Path) -> PathBuf {
    target_root_with_override(repo_root, env::var_os("CARGO_TARGET_DIR"))
}

fn target_root_with_override(
    repo_root: &Path,
    override_dir: Option<std::ffi::OsString>,
) -> PathBuf {
    override_dir
        .map(PathBuf::from)
        .map(|path| if path.is_absolute() { path } else { repo_root.join(path) })
        .unwrap_or_else(|| repo_root.join("target"))
}

#[derive(Debug, Error)]
pub enum BuildError {
    #[error("build requires a valid relay endpoint")]
    MissingEndpoint,
    #[error("invalid relay endpoint: {0}")]
    InvalidEndpoint(String),
    #[error("Android NDK location is unavailable; set ANDROID_NDK_HOME to the installed NDK")]
    AndroidNdkUnavailable,
    #[error("Android NDK clang was not found in {0}")]
    AndroidNdkClangUnavailable(PathBuf),
    #[error(
        "Android OpenSSL cross-build requires a Unix-compatible Perl. Install MSYS2 Perl or Git for Windows (expected C:\\msys64\\usr\\bin\\perl.exe or C:\\Program Files\\Git\\usr\\bin\\perl.exe)."
    )]
    UnixPerlUnavailable,
    #[error("Flutter SDK was not found; set FLUTTER_ROOT or add flutter/bin to PATH")]
    FlutterUnavailable,
    #[error("could not close the running workspace Windows client before build: {0}")]
    StopRunningClient(crate::windows_client::WindowsClientError),
    #[error("Flutter Windows build failed:\n{output}\n\nCMake install diagnostic:\n{diagnostic}")]
    WindowsFlutter { output: String, diagnostic: String },
    #[error("build command failed: {program}: {output}")]
    Command { program: String, output: String },
    #[error(
        "native artifact was not produced at {0}; check CARGO_TARGET_DIR and the target build output"
    )]
    NativeArtifactMissing(PathBuf),
    #[error("build process error: {0}")]
    Process(#[from] ProcessError),
    #[error("build I/O failed: {0}")]
    Io(std::io::Error),
    #[error("could not serialize build manifest: {0}")]
    Serialize(serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Configuration, Target};

    #[test]
    fn artifact_manifest_rejects_modified_binary() {
        let root =
            std::env::temp_dir().join(format!("torca-artifact-manifest-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let paths = RuntimePaths::from_repo(root.clone());
        paths.ensure().expect("runtime paths");
        let artifact = root.join("torca_app.exe");
        std::fs::write(&artifact, b"first build").expect("artifact");
        let endpoint = format!("{}.onion:443", "a".repeat(56));
        std::fs::write(&paths.relay_endpoint, &endpoint).expect("relay endpoint");
        let manifest = serde_json::json!({
            "endpoint": endpoint,
            "artifacts": [{
                "target": "windows",
                "path": artifact,
                "sha256": hash_file(&artifact),
            }]
        });
        std::fs::write(
            paths.manifests.join("clients-debug.json"),
            serde_json::to_vec(&manifest).expect("manifest json"),
        )
        .expect("manifest");
        verify_artifact_manifest(&paths, Target::Windows, Configuration::Debug, &artifact)
            .expect("matching artifact");
        std::fs::write(&artifact, b"modified build").expect("modified artifact");
        assert!(
            verify_artifact_manifest(&paths, Target::Windows, Configuration::Debug, &artifact)
                .is_err()
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn native_artifacts_follow_cargo_target_dir() {
        let repo = Path::new(r"G:\torca");
        assert_eq!(
            target_root_with_override(repo, Some(r"C:\cargo-target".into())),
            PathBuf::from(r"C:\cargo-target")
        );
        assert_eq!(
            target_root_with_override(repo, Some(".build-target".into())),
            repo.join(".build-target")
        );
        assert_eq!(target_root_with_override(repo, None), repo.join("target"));
    }

    #[test]
    fn windows_artifact_paths_are_case_insensitive() {
        assert!(artifact_paths_match(
            Path::new(r"G:\Torca\runner\debug\torca_app.exe"),
            Path::new(r"g:\torca\runner\Debug\torca_app.exe"),
            Target::Windows,
        ));
        assert!(!artifact_paths_match(
            Path::new("/tmp/torca/debug/app"),
            Path::new("/tmp/torca/Debug/app"),
            Target::Android,
        ));
    }

    #[test]
    fn connected_android_devices_limit_native_abis() {
        let devices = [Device {
            target: Target::Android,
            id: "phone".into(),
            android_abi: Some(AndroidAbi::Arm64),
        }];
        assert_eq!(selected_android_abis(&devices), vec![AndroidAbi::Arm64]);
        let targets = android_targets(&selected_android_abis(&devices));
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].linker, "aarch64-linux-android26-clang.cmd");
        assert_eq!(flutter_target_platforms(&[AndroidAbi::Arm64]), "android-arm64");
    }

    #[test]
    fn artifact_build_without_devices_produces_both_supported_abis() {
        assert_eq!(selected_android_abis(&[]), vec![AndroidAbi::Arm64, AndroidAbi::X86_64]);
        assert_eq!(
            flutter_target_platforms(&[AndroidAbi::Arm64, AndroidAbi::X86_64]),
            "android-arm64,android-x64"
        );
    }

    #[test]
    fn android_toolchain_uses_portable_linker_names_for_msys() {
        let toolchain = AndroidToolchain {
            ndk_bin: PathBuf::from(
                r"C:\Android\android-sdk\ndk\29.0.13113456\toolchains\llvm\prebuilt\windows-x86_64\bin",
            ),
            unix_perl: PathBuf::from(r"C:\msys64\usr\bin\perl.exe"),
        };
        let environment =
            toolchain.environment(android_targets(&[AndroidAbi::Arm64])[0], "a.onion:443");
        assert_eq!(environment["CC_aarch64-linux-android"], "aarch64-linux-android26-clang.cmd");
        assert_eq!(environment["CC_aarch64_linux_android"], "aarch64-linux-android26-clang.cmd");
        assert_eq!(
            environment["CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER"],
            "aarch64-linux-android26-clang.cmd"
        );
        assert_eq!(environment["PERL"], "perl");
    }

    #[test]
    fn artifact_manifest_rejects_another_relay_endpoint() {
        let root = std::env::temp_dir()
            .join(format!("torca-artifact-endpoint-manifest-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let paths = RuntimePaths::from_repo(root.clone());
        paths.ensure().expect("runtime paths");
        let artifact = root.join("torca_app.exe");
        std::fs::write(&artifact, b"build").expect("artifact");
        let current_endpoint = format!("{}.onion:443", "a".repeat(56));
        std::fs::write(&paths.relay_endpoint, &current_endpoint).expect("relay endpoint");
        let manifest = serde_json::json!({
            "endpoint": format!("{}.onion:443", "b".repeat(56)),
            "artifacts": [{
                "target": "windows",
                "path": artifact,
                "sha256": hash_file(&artifact),
            }]
        });
        std::fs::write(
            paths.manifests.join("clients-debug.json"),
            serde_json::to_vec(&manifest).expect("manifest json"),
        )
        .expect("manifest");
        let error =
            verify_artifact_manifest(&paths, Target::Windows, Configuration::Debug, &artifact)
                .expect_err("endpoint mismatch");
        assert!(error.contains("artifact endpoint mismatch"));
        let _ = std::fs::remove_dir_all(root);
    }
}
