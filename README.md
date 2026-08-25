# Torca

Torca is an experimental privacy-focused one-to-one messenger for Windows and Android. The client is one responsive Flutter application backed by a Rust application/runtime that owns identities, durable state, pairing, delivery, cryptography, networking and security policy.

> **Status:** security-sensitive alpha software. Torca has not received an independent production security audit and must not be presented as a finished high-risk communications product. Automated tests and successful builds are engineering evidence, not a security certification.

## What exists today

The current source implements:

- a shared Flutter client for Windows and Android;
- local installation identity and SQLCipher-backed structured storage;
- OS-protected secret adapters for identity, database and relationship secrets;
- explicit contact pairing with QR/full-link flows and local approval;
- authenticated peer sessions with application-layer authenticated encryption;
- provider-neutral communication with **Tor** and **Iroh** selectable in normal deployment;
- durable message retry, delivered/read receipts, replies, paged history and search;
- encrypted resumable attachments and explicit export/open flows;
- Safety Number-style contact verification and identity-change protection;
- privacy-aware notifications, diagnostics and Android capture protection by default;
- mutual-consent half-duplex Radio Mode with provider-owned media transport; and
- a typed deployment tool plus multi-process/device soak-test cockpit.

Groups, calls, multi-device synchronization, public discovery, cloud backup and a supported Linux production client are outside the current product baseline.

## Communication providers

Torca has one active communication provider per deployment. The application protocol, encryption, delivery, attachments, Radio Mode coordination and persistence stay provider-neutral.

| Provider | Normal deployment | Pairing bootstrap | Current product capabilities |
| --- | --- | --- | --- |
| Tor | selectable | managed rendezvous | messages, attachments, incoming sessions, Radio, QR/full-link/short-code pairing |
| Iroh | selectable | direct QR/full link | messages, attachments, incoming sessions, Radio |
| WebRTC | hidden | external signaling | adapter/contracts exist; platform session/signaling composition is not deployment-ready |
| Memory | hidden | test-only | deterministic/simulated runtime use |

Tor remains the default provider for backward-compatible deployments. Iroh is also marked deployment-ready by the shared provider profile. See [`docs/transport.md`](docs/transport.md) for the current provider boundary and capability model.

## Architecture

![Torca architecture](docs/diagrams/architecture.svg)

The central ownership rule is:

- **Flutter** renders presentation-safe state, owns navigation/transient interaction state, and submits typed user intent through `EngineGateway`.
- **Rust** owns durable workflow state, identifiers, pairing, cryptography, persistence, provider composition, delivery/retry, diagnostics and security rules.
- **Platform hosts** own genuine OS integration such as protected secret stores, lifecycle, permissions, notifications and window/capture behavior.

The process-owned native runtime selects exactly one communication provider and exposes provider-neutral snapshots/events back to Flutter.

Read [`ARCHITECTURE.md`](ARCHITECTURE.md) for the system model and [`docs/app-flows.md`](docs/app-flows.md) for current user/runtime flows.

## Repository map

```text
apps/client/flutter/      Flutter UI and Windows/Android host integration
packages/                 reusable Flutter packages
crates/foundation/        dependency-light primitives
crates/domains/           product vocabulary and invariants
crates/protocol/          peer/pairing/relay/radio/wire formats
crates/application/       use cases, runtime policy and provider-neutral ports
crates/infrastructure/    storage, crypto, transport and communication adapters
crates/platform/          contract, native composition and OS adapters
services/relay/           Tor pairing rendezvous service
tools/torca-deploy/       canonical build/run/deploy/log workflow
tools/torca-soak/         soak-test cockpit and orchestration
tests/torca-integration/  cross-crate integration journeys
docs/                     maintained project documentation
```

The Rust workspace version is currently `0.2.0-alpha.0`; the Flutter app uses the matching alpha product version. Version numbers do not imply release or audit maturity.

## Development

The canonical interactive entry point is the Rust deployment tool:

```powershell
cargo run -p torca-deploy
```

Useful non-interactive examples:

```powershell
cargo run -p torca-deploy -- status
cargo run -p torca-deploy -- plan --target all --configuration debug
cargo run -p torca-deploy -- build --target windows --configuration debug
cargo run -p torca-deploy -- deploy --target android --device <adb-serial>
cargo run -p torca-deploy -- logs --target all
cargo run -p torca-deploy -- resume
```

See [`docs/development.md`](docs/development.md) for prerequisites and day-to-day workflow, and [`docs/operations.md`](docs/operations.md) for deployment, diagnostics and lifecycle behavior.

## Validation

The checked-in GitHub Actions workflow covers source policy, Rust format/check/clippy/tests, generated contract drift, Flutter analysis/tests, and Windows/Android builds. A configured workflow is not evidence that a particular commit passed; check the actual workflow run before citing CI.

Common local checks:

```powershell
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D clippy::correctness -D clippy::suspicious -D clippy::perf
cargo test --workspace --all-targets --all-features --locked
```

From `apps/client/flutter`:

```powershell
flutter analyze
flutter test
```

See [`docs/testing.md`](docs/testing.md) for source, platform, integration and soak-test gates.

## Security and privacy

The current relationship-secret design does **not** claim Signal-style forward secrecy or post-compromise security. Provider privacy properties also differ: Tor is intended to reduce direct network-location exposure through onion routing, while direct-path providers such as Iroh have a different network-metadata surface. Application-layer peer authentication and encryption remain required regardless of provider.

Read [`SECURITY.md`](SECURITY.md), [`PRIVACY.md`](PRIVACY.md) and [`docs/security/threat-model.md`](docs/security/threat-model.md) before making security/privacy claims or changing pairing, transport, cryptography, storage, notifications, Radio Mode or platform boundaries.

## Documentation

[`docs/README.md`](docs/README.md) is the documentation index. The maintained set is intentionally small; implementation plans and old validation ledgers belong in Git history rather than competing with current-state documentation.

## License

Torca is licensed under AGPL-3.0-or-later. See [`LICENSE`](LICENSE) and [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md).