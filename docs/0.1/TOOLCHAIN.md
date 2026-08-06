# Torca 0.1 toolchain

This document records the supported development baseline for the active 0.1 release.

## Pinned versions

| Tool | Version | Source of truth |
|---|---:|---|
| Rust | `1.97.1` | `rust-toolchain.toml` |
| Cargo | bundled with Rust `1.97.1` | Rust toolchain |
| Rust edition | `2024` | root `Cargo.toml` |
| Flutter CI baseline | `3.44.7` stable | CI workflow |
| Flutter local minimum | `3.44.0` | `pubspec.yaml` / build preflight |
| Dart local minimum | `3.12.0` | `pubspec.yaml` / build preflight |
| cargo-ndk | bootstrap baseline `4.1.2` | `tools/build/Torca.Build.psm1` |
| PowerShell | `7+` | public workflow scripts |

A newer compatible Flutter 3.44 patch is acceptable locally; the owner validation currently uses Flutter `3.44.9` / Dart `3.12.2`.

## Toolchain preflight

Developers do not invoke separate preflight scripts. `build.ps1`, `run.ps1` and `deploy.ps1` perform the relevant checks internally.

When Flutter is older than the supported baseline:

```powershell
flutter channel stable
flutter upgrade
flutter --version
```

Android native builds install `cargo-ndk` automatically when it is missing and ensure the required Rust Android targets are present. The Android NDK itself is provided by the normal Flutter/Android SDK installation.

## Canonical workflows

```powershell
./scripts/build.ps1
./scripts/run.ps1
./scripts/deploy.ps1
```

CI uses:

```powershell
./scripts/build.ps1 -Target check -CI
```

`build.ps1` owns formatting/codegen, release and architecture checks, Cargo dependency resolution, Rust check/Clippy/tests, Flutter dependency resolution, static analysis and Flutter tests. Platform builds add the native dynamic library and standard Flutter platform scaffold.

## Upgrade policy

A toolchain baseline change must update every affected pin and document in one coherent change, pass the canonical build command, and be recorded in `0.1_PROGRESS.md`. Toolchain upgrades must not be mixed with product-domain behavior changes.
