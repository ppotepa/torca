# Torca

Torca is a privacy-focused, peer-to-peer messenger built around local identities, Tor onion services, encrypted local storage and explicit contact pairing.

This repository is the clean implementation of Torca. The previous [`ppotepa/tOrca`](https://github.com/ppotepa/tOrca) repository remains only a source of requirements, protocol lessons and selected reviewed implementations.

## Start here

Development targets **Torca 0.1**. Read [`0.1_PROGRESS.md`](0.1_PROGRESS.md) before changing the repository. It is the canonical record of implementation, validation, release gaps and the exact next action.

## One client

Torca has one application client:

```text
responsive Flutter UI
        |
shared Dart FFI gateway
        |
torca-native (torca_bridge.dll / libtorca_bridge.so)
        |
EngineBridge
        |
ClientEngine actor
        |
domains and infrastructure adapters
```

Windows and Android are build targets of the same Flutter application. They do not have separate UI or application-workflow implementations. Platform-specific Kotlin/C++ is limited to actual operating-system services such as protected key storage, lifecycle integration and tray/notification behavior.

The UI adapts by available width: compact devices use routed screens, while wide desktop/tablet layouts use split views backed by the same widgets and the same engine state.

## Repository layout

```text
apps/client/flutter/      the single responsive application client
crates/foundation/        dependency-light shared primitives
crates/domains/           independent mini-domain libraries
crates/application/       engine, projections and diagnostics
crates/infrastructure/    storage, crypto, files, peer and Tor adapters
crates/protocol/          versioned wire contracts
crates/platform/          bridge, native ABI and OS adapters
tools/build/              private build/run/deploy implementation
tools/                    deterministic contract generation
services/relay/           ephemeral rendezvous broker
tests/torca-integration/  cross-crate primary-journey tests
docs/0.1/                 release scope and gates
```

## Developer workflow

There are exactly three public workflows:

```powershell
./scripts/build.ps1
./scripts/run.ps1
./scripts/deploy.ps1
```

Typical usage:

```powershell
# Validate and build the default local target.
./scripts/build.ps1

# Fast Windows development loop with Flutter hot reload.
./scripts/run.ps1 -Target windows

# Run on an Android device/emulator.
./scripts/run.ps1 -Target android -Device emulator-5554

# Produce release artifacts and SHA-256 checksums.
./scripts/deploy.ps1 -Target all
```

Formatting, code generation, architecture checks, lockfile refresh, Clippy, tests, Flutter platform bootstrap, Android Rust cross-compilation and packaging are private implementation details of those three commands.

## Architecture rules

- Domains never depend on Flutter, SQLite implementations, sockets, FFI or Tor process APIs.
- Flutter sends commands and renders snapshots; workflow state belongs to Rust.
- All state-changing client operations pass through the single-writer `ClientEngine` actor.
- SQL lives in parameterized `.sql` files owned by storage.
- Pairing may use the untrusted ephemeral relay; normal contact traffic is direct over Tor.
- Memory implementations are explicit development/test choices and are never a silent production fallback.

## Canonical documents

- [`0.1_PROGRESS.md`](0.1_PROGRESS.md) — quantitative status and handoff.
- [`ARCHITECTURE.md`](ARCHITECTURE.md) — system boundaries.
- [`docs/0.1/IMPLEMENTATION_ORDER.md`](docs/0.1/IMPLEMENTATION_ORDER.md) — implementation batches.
- [`docs/0.1/TEST_MATRIX.md`](docs/0.1/TEST_MATRIX.md) — automated and platform test matrix.
- [`docs/0.1/KNOWN_LIMITATIONS.md`](docs/0.1/KNOWN_LIMITATIONS.md) — current limitations.
- [`docs/0.1/RELEASE_CHECKLIST.md`](docs/0.1/RELEASE_CHECKLIST.md) — binary release gate.
- [`docs/security/threat-model.md`](docs/security/threat-model.md) — assets, boundaries and threats.
- [`docs/decisions`](docs/decisions/README.md) — architecture decision records.

All 0.1 work currently lands directly on `main`. See [`CONTRIBUTING.md`](CONTRIBUTING.md).
