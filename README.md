# Torca

Torca is an experimental privacy-focused one-to-one messenger for Windows and Android. It uses one responsive Flutter client backed by one Rust application/runtime. Local identities, encrypted local state, pairing, delivery and security policy live in Rust; paired contact traffic uses authenticated Iroh peer sessions with direct paths or configured relay fallback, rather than a central message server.

> **Project status:** Torca is security-sensitive alpha software. It is under active development, has not received an independent production security audit, and should not be treated as a finished high-risk communications product. Source-level validation, successful local builds and passing tests are useful engineering evidence, not a security certification or a substitute for real-device soak testing.

The previous [`ppotepa/tOrca`](https://github.com/ppotepa/tOrca) repository is a requirements/reference source, not a second active implementation.

## Current capabilities

The current source includes:

- local installation identity and SQLCipher-backed structured storage;
- operating-system protected-secret adapters for identity, storage and peer secrets;
- short-lived pairing codes/QR with explicit local approval;
- authenticated peer sessions and application-layer authenticated encryption;
- direct peer delivery through Iroh after pairing;
- durable message retry, delivered/read receipts and reply-to;
- paged and searchable conversation history;
- encrypted, resumable attachments;
- Safety Number-style contact verification with identity-change protection;
- privacy-aware notifications, diagnostics and Android screen-capture protection by default;
- an experimental mutual-consent, half-duplex Radio Mode over the paired peer channel; and
- one shared responsive Flutter application for Windows and Android.

Calls, groups, multi-device synchronization, public discovery and cloud backup are outside the current product scope. Linux is not currently a supported production client composition.

## Security model at a glance

Torca uses established cryptographic primitives and keeps secret ownership out of Flutter. Pairing establishes a protected pairwise relationship secret; peer payloads use authenticated encryption with fresh nonces and associated context. Radio Mode derives session-specific directional media keys from the protected pairwise relationship secret. Iroh direct connectivity is not an anonymity network: network-location exposure depends on the selected Iroh path/profile.

The current message-key design **does not provide Signal-style forward secrecy or post-compromise security**. Compromise of a long-lived relationship secret can therefore have consequences beyond one message or session. Iroh does not eliminate traffic-analysis, timing-correlation, endpoint-compromise or denial-of-service risk.

Read [`SECURITY.md`](SECURITY.md) and [`docs/security/threat-model.md`](docs/security/threat-model.md) before making security claims or changing pairing, peer authentication, cryptography, storage, notifications, Radio Mode or platform boundaries.

## Architecture

```text
responsive Flutter UI
        |
EngineGateway / generated DTOs
        |
torca-native C ABI
        |
ClientApplicationRuntime + process-owned runtime
        |
SQLCipher / crypto / files / peer link / Iroh transport
```

The main ownership rule is simple: Flutter renders presentation-safe state and submits typed user intent; Rust owns identifiers, durable state, networking, cryptography and product/security rules. The dependency direction is enforced by repository policy checks, not only by documentation.

See [`ARCHITECTURE.md`](ARCHITECTURE.md) for the maintained system model.

## Repository layout

```text
apps/client/flutter/      shared responsive application client
crates/foundation/        dependency-light primitives
crates/domains/           product vocabulary and invariants
crates/protocol/          bounded wire and pairing/peer/radio protocols
crates/application/       use cases, ports, runtime coordination and policy
crates/infrastructure/    SQLCipher, crypto, files, peer and Iroh adapters
crates/platform/          contract, native ABI and OS adapters
pairing service            ephemeral pairing exchange
tests/torca-integration/  cross-crate integration journeys
tools/torca-deploy/       canonical build/run/deploy/log workflow
scripts/                  compatibility and validation helpers
docs/                     maintained documentation and runbooks
```

## Developer workflow

The canonical local entry point is the Rust deployment tool:

```powershell
cargo run -p torca-deploy
```

With no subcommand it opens the Ratatui wizard. The CLI exposes the same planner/executor for automation, for example:

```powershell
cargo run -p torca-deploy -- status
cargo run -p torca-deploy -- plan --target all --configuration debug
cargo run -p torca-deploy -- build --target windows --configuration debug
cargo run -p torca-deploy -- logs --target all
cargo run -p torca-deploy -- resume
```

Use `--dry-run` where supported when you want to inspect a plan without changing devices. Destructive data resets should be deliberate actions, not incidental development steps.

## Validation

The repository CI definition checks the core Rust workspace, generated Rust/Dart contract, Flutter analysis/tests and Windows/Android client builds. Before presenting a change as validated, distinguish exactly what was run: source checks, host builds, device tests and end-to-end/soak evidence are different gates.

Useful local source checks include:

```powershell
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D clippy::correctness -D clippy::suspicious -D clippy::perf
cargo test --workspace --all-targets --all-features --locked
```

Flutter validation is run from `apps/client/flutter` with `flutter analyze` and `flutter test`. Generated contract drift is checked through `torca-contract-gen` and repository source policy.

A configured GitHub Actions workflow is not evidence that a particular commit passed CI unless the jobs actually ran and completed successfully.

## Documentation

Start with [`docs/STATUS.md`](docs/STATUS.md) for the current maturity/validation summary and [`docs/README.md`](docs/README.md) for the documentation map.

The main maintained documents are:

- [`ARCHITECTURE.md`](ARCHITECTURE.md) — stable ownership and dependency boundaries;
- [`SECURITY.md`](SECURITY.md) — security guarantees, limits and reporting guidance;
- [`docs/security/threat-model.md`](docs/security/threat-model.md) — assets, trust boundaries and threats;
- [`PRIVACY.md`](PRIVACY.md) — local/network data behavior and user choices;
- [`CONTRIBUTING.md`](CONTRIBUTING.md) — contributor workflow and documentation rules;
- [`docs/development.md`](docs/development.md) — local development workflow;
- [`docs/testing.md`](docs/testing.md) — automated and device validation; and
- [`docs/STATUS.md`](docs/STATUS.md) — current engineering and validation status.

Operational details live in [`docs/operations.md`](docs/operations.md), while
transport boundaries are described in [`docs/transport.md`](docs/transport.md).

Planning labels such as “0.3” and the Cargo package version serve different purposes and can move at different times. Do not infer release maturity from either label alone.

## License

Torca is licensed under AGPL-3.0-or-later. See [`LICENSE`](LICENSE) and [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md).
