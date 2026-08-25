# Development

Torca is a Rust workspace plus a Flutter client. Use the typed Rust deployment tool as the canonical build/run/deploy entry point instead of building a parallel script workflow.

## Toolchain baseline

The workspace currently pins/declares:

- Rust `1.97.1`, edition 2024;
- Flutter `3.44.x` in CI (`3.44.7` in the workflow);
- Dart SDK compatible with the Flutter app's `>=3.12.0 <4.0.0` constraint;
- Java 17 for Android CI builds; and
- platform build dependencies required by Flutter/Rust for Windows or Android.

Use `Cargo.lock` and Flutter package resolution committed/current for reproducible project work.

## Canonical entry point

From the repository root:

```powershell
cargo run -p torca-deploy
```

No subcommand opens the Ratatui wizard. The CLI uses the same planner/executor for repeatable automation.

Common commands:

```powershell
cargo run -p torca-deploy -- status
cargo run -p torca-deploy -- plan --target all --configuration debug
cargo run -p torca-deploy -- build --target windows --configuration debug
cargo run -p torca-deploy -- build --target android --configuration debug
cargo run -p torca-deploy -- deploy --target android --device <adb-serial>
cargo run -p torca-deploy -- logs --target all
cargo run -p torca-deploy -- resume
```

Use `--dry-run` where supported to inspect a plan without invoking Docker, Flutter, Cargo or ADB actions.

## Provider selection

Tor is the default communication provider. Normal deployment also exposes Iroh. Provider selection is carried through the deployment plan/artifact metadata and validated by native startup.

Do not add provider branching in Flutter. Provider capabilities are exposed through runtime/build metadata. See [`transport.md`](transport.md).

## Flutter client

The Flutter application lives at `apps/client/flutter` and is shared by Windows and Android.

Typical UI-only workflow:

```powershell
cd apps/client/flutter
flutter pub get
flutter analyze
flutter test
```

Generated contract code under `lib/generated` is not an independent source of product rules. Contract changes should be made at the canonical Rust/schema boundary and regenerated/checked through `torca-contract-gen`.

## Rust workspace

The root workspace groups code by architectural responsibility rather than by platform feature:

- `foundation` — primitives;
- `domains` — product state/invariants;
- `protocol` — bounded wire formats;
- `application` — use cases/runtime/ports;
- `infrastructure` — concrete storage/crypto/network adapters;
- `platform` — native/OS composition.

Before adding a dependency, preserve inward dependency direction. Source-policy scripts in `scripts/modules` enforce important restrictions and run in CI.

## Build artifacts and deploy checkpoints

`torca-deploy` owns build/deploy orchestration. Deployment checkpoints are stored below `.torca/deploy/` and are intended to make resume/recovery explicit rather than hiding partial device state.

Artifact/provider metadata is checked before reuse/install so a binary built for another provider is not silently deployed into the wrong plan.

## Android notes

Use `--device <adb-serial>` when a workflow must target exactly one Android device. The deployer keeps reset/install/launch actions scoped to that serial.

Android capture protection is strict by default. The explicit development option:

```powershell
cargo run -p torca-deploy -- run --target android --privacy allow-capture
```

changes the Android secure-window/capture behavior only. It does not weaken message encryption or change the selected communication provider. Never describe an allow-capture test build as capture-protected.

## Windows notes

The Windows host participates in the same Flutter/Rust application. Desktop lifecycle/tray integration is initialized after the native gateway is ready; business logic must not migrate into the Windows runner merely because a behavior is desktop-specific.

## Generated/public contract

The Flutter/native boundary is typed. `EngineGateway` exposes snapshots, runtime events, commands, diagnostics and optional capability interfaces. Keep private keys, relationship secrets and durable security state out of DTOs.

Run generated contract drift checks whenever the schema changes:

```powershell
cargo run -p torca-contract-gen -- --check apps/client/flutter/lib/generated/torca_contract.dart
```

## Adding a feature

A normal feature should follow this direction:

```text
product invariant/domain model
  -> application command/query/port
  -> infrastructure/platform implementation if needed
  -> generated presentation contract
  -> Flutter rendering/interaction
```

Do not begin by creating a Flutter state machine for a durable/security-sensitive workflow and then mirror it in Rust.

## Documentation changes

Current-state docs are intentionally consolidated. Update:

- `ARCHITECTURE.md` for ownership/dependency/process changes;
- `docs/transport.md` for provider composition/capability changes;
- `docs/app-flows.md` for product/runtime flow changes;
- `SECURITY.md` / threat model / `PRIVACY.md` for security/privacy boundary changes;
- this file for canonical developer workflow changes; and
- `docs/testing.md` when evidence gates change.

Do not add a new long-lived plan/checklist file when GitHub issues/PRs/history can carry temporary implementation work.