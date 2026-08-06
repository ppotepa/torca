# Torca 0.1 toolchain

This document records the supported development baseline for the active 0.1 release.

## Pinned versions

| Tool | Version | Source of truth |
|---|---:|---|
| Rust | `1.97.1` | `rust-toolchain.toml` |
| Cargo | bundled with Rust `1.97.1` | Rust toolchain |
| Rust edition | `2024` | root `Cargo.toml` |
| Flutter CI baseline | `3.44.7` stable | CI workflow and this document |
| Flutter local minimum | `3.44.0` | `pubspec.yaml` and toolchain preflight |
| Dart local minimum | `3.12.0` | `pubspec.yaml` and toolchain preflight |
| PowerShell | `7+` | validation script runtime |

Rust `1.97.1` is used instead of `1.97.0` because the point release fixes an LLVM miscompilation. Flutter 3.44 is the supported release line for Torca 0.1 and bundles Dart 3.12. CI remains pinned to Flutter `3.44.7`; a newer stable 3.44 patch is acceptable for local validation as long as it satisfies the minimum versions above.

## Toolchain preflight

Before Flutter dependency resolution, `scripts/validate.ps1` runs:

```powershell
./scripts/check-flutter-toolchain.ps1
```

The preflight reads `flutter --version --machine` and rejects local Flutter/Dart installations older than the supported baseline. This prevents a package-solver failure from being mistaken for an application dependency defect.

When the preflight reports an older SDK, update the stable Flutter installation and verify the bundled Dart version:

```powershell
flutter channel stable
flutter upgrade
flutter --version
```

## Canonical validation

The only supported repository-wide validation entrypoint is:

```powershell
./scripts/validate.ps1
```

It runs Rust formatting, generated-contract verification, build checks, Clippy, Rust tests, Flutter toolchain preflight, Flutter dependency resolution, Dart formatting, Flutter static analysis and Flutter tests. The GitHub Actions workflow calls the same script.

## Upgrade policy

A toolchain baseline change must update every affected pin and document in one commit, run the canonical validation command, and record the result in `0.1_PROGRESS.md`. Toolchain upgrades must not be mixed with domain behavior changes.
