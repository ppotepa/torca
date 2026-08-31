# Development

Torca is a Rust workspace plus one shared Flutter client. Use the Rust deployment tool as the canonical build/run/deploy entry point and keep implementation aligned with the architecture boundaries in [`../ARCHITECTURE.md`](../ARCHITECTURE.md).

## Toolchain sources of truth

Do not maintain a second prose copy of exact tool versions. Use:

- `rust-toolchain.toml` and root `Cargo.toml` for Rust/toolchain requirements;
- `Cargo.lock` for resolved Rust dependencies;
- `apps/client/flutter/pubspec.yaml` and `pubspec.lock` for Flutter/Dart/package constraints;
- Android Gradle files for Java/Android build requirements; and
- `.github/workflows/validate.yml` for the currently automated CI environment.

When these files change, update prose only if the workflow/requirement meaning changes.

## Canonical entry point

From the repository root:

```powershell
cargo run -p torca-deploy
```

No subcommand opens the interactive deployment UI. CLI subcommands use the same planner/executor. Representative commands are:

```powershell
cargo run -p torca-deploy -- status
cargo run -p torca-deploy -- plan --target all --configuration debug
cargo run -p torca-deploy -- build --target windows --configuration debug
cargo run -p torca-deploy -- build --target android --configuration debug
cargo run -p torca-deploy -- deploy --target android --device <adb-serial>
cargo run -p torca-deploy -- logs --target all
cargo run -p torca-deploy -- resume
```

Use command help for exact current flags and `--dry-run` where supported to inspect a normalized plan without changing host/devices.

## Flutter client

The shared client lives under `apps/client/flutter`.

For presentation-only work:

```powershell
cd apps/client/flutter
flutter pub get
flutter analyze
flutter test
```

Flutter owns responsive presentation/navigation/transient interaction. Product workflows, durable state, provider lifecycle, storage and security-sensitive decisions stay in Rust.

## Rust workspace

The workspace layers are:

- `foundation` — dependency-light primitives;
- `protocol` — bounded external/wire contracts;
- `domains` — product vocabulary/invariants;
- `application` — use cases, ports, read models and runtime policy;
- `infrastructure` — concrete storage/crypto/files/provider adapters; and
- `platform` — generated/native boundary plus Windows/Android composition.

Preserve inward dependency direction. Architecture/source policy scripts under `scripts/modules` are executable guardrails, not optional conventions.

## Provider composition

Iroh is the sole production communication provider. Memory is test-only. Production provider selection is static composition; do not add provider branching to Flutter.

Iroh profile/configuration can be part of build/deploy metadata. Artifact reuse must respect that metadata so an artifact built for another profile/configuration is not silently reused.

See [`TRANSPORT.md`](TRANSPORT.md).

## Generated contract

The Flutter/native boundary is generated from the canonical Torca contract. Change the canonical schema/input and regenerate/check the outputs; do not hand-maintain a divergent Dart protocol model.

A normal check is:

```powershell
cargo run -p torca-contract-gen -- --check apps/client/flutter/lib/generated/torca_contract.dart
```

Keep private keys, database keys, relationship secrets and other non-presentation state out of contract DTOs/logs.

## Feature placement

A normal feature should move in this direction:

```text
product invariant / domain concept
  -> application command/query/port
  -> infrastructure/platform implementation when required
  -> generated presentation contract
  -> Flutter rendering/interaction
```

Do not start a durable/security workflow as a Flutter state machine and then mirror it in Rust. Do not put product policy into SQL repositories, Iroh adapters or OS hosts simply because they execute the final side effect.

## Data and SQL

Operational/business SQL is owned by storage infrastructure and should remain parameterized and bounded. Use repository/read-model APIs from upper layers. Versioned schema compatibility and storage epoch rules are described in [`VERSIONING-AND-RELEASES.md`](VERSIONING-AND-RELEASES.md).

## Deploy state and local artifacts

`torca-deploy` owns build/deploy checkpoints under `.torca/deploy/`; soak/measurement tools write their own ignored local artifacts under `.torca/`. These are local run/evidence outputs and are not sources of product truth.

Destructive data reset must be an explicit choice. Normal iteration should preserve identity/history unless a clean-profile scenario is the thing being tested.

## Documentation/changelog discipline

When behavior changes, update the existing canonical page listed in [`README.md`](README.md). Notable user/developer/compatibility/security changes also update [`../CHANGELOG.md`](../CHANGELOG.md) under `Unreleased`.

Use [`VERSIONING-AND-RELEASES.md`](VERSIONING-AND-RELEASES.md) for product/build/contract/storage/wire/ABI changes. Temporary implementation plans should not become long-lived competing documentation.

## Validation

Run the narrowest deterministic checks that prove the change, then expand according to [`TESTING.md`](TESTING.md). Source checks, platform builds, device runs and soak evidence are different claims.
