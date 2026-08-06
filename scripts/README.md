# Development scripts

This directory contains supported repository entrypoints for validation, build, local infrastructure, deployment and diagnostics.

## Available now

### `validate.ps1`

Canonical repository-wide validation command:

```powershell
./scripts/validate.ps1
```

Optional layer-specific execution:

```powershell
./scripts/validate.ps1 -SkipFlutter
./scripts/validate.ps1 -SkipRust
```

The full command validates Rust formatting, compilation, Clippy, Rust tests, Flutter dependency resolution, Dart formatting, Flutter analysis and Flutter tests. GitHub Actions invokes this same script.

## Planned commands

Later milestones will add thin orchestration entrypoints for dependency-boundary checks, generated contract verification, local relay and Tor startup, platform deployment, diagnostics and release consistency.

Scripts must remain orchestration layers. Product behavior belongs in libraries and deployable applications.
