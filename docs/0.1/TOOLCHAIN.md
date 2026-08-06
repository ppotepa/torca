# Torca 0.1 toolchain

This document records the supported development baseline for the active 0.1 release.

## Pinned versions

| Tool | Version | Source of truth |
|---|---:|---|
| Rust | `1.97.1` | `rust-toolchain.toml` |
| Cargo | bundled with Rust `1.97.1` | Rust toolchain |
| Rust edition | `2024` | root `Cargo.toml` |
| Flutter | `3.44.7` stable | CI workflow and this document |
| Dart | `3.12.0` | bundled with Flutter `3.44.x` |
| PowerShell | `7+` | validation script runtime |

Rust `1.97.1` is used instead of `1.97.0` because the point release fixes an LLVM miscompilation. Flutter is pinned to the current `3.44.7` stable patch line.

## Canonical validation

The only supported repository-wide validation entrypoint is:

```powershell
./scripts/validate.ps1
```

It runs Rust formatting, build checks, Clippy, Rust tests, Flutter dependency resolution, Dart formatting, Flutter static analysis and Flutter tests. The GitHub Actions workflow calls the same script.

## Upgrade policy

A toolchain change must update every affected pin and document in one commit, run the canonical validation command, and record the result in `0.1_PROGRESS.md`. Toolchain upgrades must not be mixed with domain behavior changes.
