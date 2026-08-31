# Development and deployment helpers

The canonical Torca development/deployment workflow is the Rust `torca-deploy` tool. The scripts in this directory are compatibility, validation and maintenance helpers; they are not a second deployment architecture.

## Canonical entry point

From the repository root:

```powershell
cargo run -p torca-deploy
```

With no subcommand, the tool opens the Ratatui wizard. For automation, use the typed CLI:

```powershell
cargo run -p torca-deploy -- status
cargo run -p torca-deploy -- plan --target all --configuration debug
cargo run -p torca-deploy -- run --target all
cargo run -p torca-deploy -- deploy --target all --configuration debug
cargo run -p torca-deploy -- rebuild --target all --configuration debug
cargo run -p torca-deploy -- full-redeploy --target all --configuration debug
cargo run -p torca-deploy -- logs --target all
cargo run -p torca-deploy -- resume
```

Use `--dry-run` on plan-based commands when you want to inspect execution without changing the host or devices.

The plan model makes destructive choices explicit. In particular:

- client data defaults to preservation for normal work;
- full redeploy is the deliberate client-reset path;
- Iroh endpoint identity is persisted and managed by the native runtime;
- Android screen capture is blocked by default (`--privacy strict`); and
- `--privacy allow-capture` is a local-development opt-out from the Android window capture flag, not a transport/security change.

See `cargo run -p torca-deploy -- --help` and the subcommand help for the current option set rather than copying every flag into long-lived documentation.

Iroh is the sole production provider and Memory is test-only. The compatibility
scripts are retained for validation and historical workflows; provider-aware
deployment should use the Rust entry point above.

## What the Rust tool owns

`tools/torca-deploy` owns the normal developer lifecycle:

- target/device discovery;
- plan normalization and destructive-action policy;
- source-aware client builds;
- installation and launch;
- durable deployment checkpoints/resume;
- runtime health waits; and
- diagnostic collection.

Deployment state and run history live under `.torca/` and are ignored by Git.

## PowerShell scripts

Files under `scripts/` remain for repository validation, compatibility and narrow maintenance tasks while Rust equivalents mature. They may also be used internally by CI or source-policy checks.

Do not add new public workflows that bypass or duplicate `torca-deploy`. If a capability belongs in the normal deployment lifecycle, add it to the Rust planner/executor and expose it through the wizard/CLI.

In particular, older `wizard.ps1`, `deploy.ps1`, `redeploy.ps1`, `torca.ps1` and related entry points should be treated as compatibility surfaces unless a current source path explicitly depends on them. Documentation should not present them as the preferred path.

## Validation helpers

Repository policy scripts under `scripts/modules/` enforce source/architecture constraints before or alongside expensive builds. The maintained CI definition in `.github/workflows/validate.yml` is the reference for the full automated matrix.

Do not interpret a policy check as equivalent to a platform build or device test. Validation claims should identify the exact gate that ran.

## Diagnostics

Use the Rust collector for the current incident format:

```powershell
cargo run -p torca-deploy -- logs --target all
```

Each collection creates a fresh incident directory instead of mixing new evidence with older runs. See [`../docs/OPERATIONS.md`](../docs/OPERATIONS.md) for the current producer and bundle layout.

## Related documentation

- [`../README.md`](../README.md) — product overview and developer entry point.
- [`../CONTRIBUTING.md`](../CONTRIBUTING.md) — contributor and validation rules.
- [`../docs/STATUS.md`](../docs/STATUS.md) — current maturity/validation status.
- [`../docs/TESTING.md`](../docs/TESTING.md) — real-device and automated validation guidance.

## Background Iroh benchmark

For unattended Iroh measurements, use `Start-TorcaBackgroundTest.ps1`. The
default `smoke` mode runs the Iroh transport/conformance checks and one
screen-off emulator sample. `-Mode conversation` additionally runs the
production `torca-soak` Active Messaging scenario with a real lab bot contact,
including pairing, bidirectional messages and delivery checks. `-Mode soak`
repeats a selected profile; `-Mode full` runs `always`, `direct` and `local`
sequentially.

The helper serializes runs, starts a headless emulator with two cores and
1536 MB RAM, uses BelowNormal priority, and aborts when the emulator exceeds
15% of host CPU. Results are written as JSON under
`.torca/measurements/background`; interrupted or incomplete runs are not green.

```powershell
.\scripts\Start-TorcaBackgroundTest.ps1 -Mode soak -Profile always -DurationSeconds 60 -Repetitions 3
.\scripts\Start-TorcaBackgroundTest.ps1 -Mode conversation -Profile always -FakePeers 1 -DurationSeconds 180
```

Headless runs deliberately skip the unreliable API 35/36 uiautomator probe and
record that fact in `startup-ui.xml`; readiness is inferred from resumed
`MainActivity` plus fatal-log scanning. Add `-EnableUiProbe` when running an
AVD with a real window and you want semantic menu labels checked as well.

Emulator CPU is not calibrated battery energy. Relay/discovery isolation,
Wi-Fi/LTE migration and mAh attribution remain hardware-only gates.
