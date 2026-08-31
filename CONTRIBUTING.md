# Contributing to Torca

Torca is a security-sensitive Rust/Flutter application. Keep changes reviewable, preserve enforced architecture boundaries, and report validation evidence precisely.

## Read first

1. [`README.md`](README.md) — current product and repository shape.
2. [`ARCHITECTURE.md`](ARCHITECTURE.md) — ownership/layer/provider boundaries.
3. [`docs/app-flows.md`](docs/app-flows.md) — current product/runtime journeys.
4. [`docs/transport.md`](docs/transport.md) — communication-provider model.
5. [`SECURITY.md`](SECURITY.md), [`docs/security/threat-model.md`](docs/security/threat-model.md) and [`PRIVACY.md`](PRIVACY.md) for security/privacy-sensitive work.
6. [`docs/development.md`](docs/development.md) and [`docs/testing.md`](docs/testing.md) for workflow/evidence.

## Canonical development workflow

Use the Rust deployment tool for normal build/run/deploy/device/log work:

```powershell
cargo run -p torca-deploy
```

No subcommand opens the Ratatui wizard. CLI commands use the same planner/executor for automation. PowerShell scripts are compatibility/validation helpers, not a second deployment pipeline.

## Architectural ownership

Flutter owns presentation, responsive layout, navigation, transient interaction state and local UI preferences. It submits typed user intent through `EngineGateway`.

Rust owns identities, durable workflow state, security-sensitive identifiers/timestamps, pairing, peer authentication, cryptography, persistence, communication-provider composition, delivery/retry, attachments, receipts, Radio Mode policy, diagnostics and presentation-safe projections.

Platform code owns genuine OS integration: lifecycle, paths, protected secret stores, notifications, capture/window/permission behavior and provider host bridges where necessary.

Do not move durable/security state machines into Dart, serialization code, Kotlin/C++ hosts, SQL repositories or transport adapters.

## Communication providers

The application/runtime is provider-neutral above `torca-transport-api`. Iroh
is the sole production provider; Memory is reserved for deterministic tests.

Provider work must not:

- branch product logic in Flutter based on provider names;
- make Arti/QUIC/DataChannel types leak into application/domain layers;
  - silently substitute another provider when Iroh was selected;
  - fabricate legacy provider/onion state for Iroh; or
  - claim anonymity for a direct provider.

Opening a new provider requires composition/capability/pairing tests plus security/privacy documentation review.

## Data, SQL and history

Business SQL belongs in storage infrastructure and should remain parameterized/bounded. Application/domain code consumes repositories/read models rather than raw database connections.

Conversation history/search is paged/query-driven from Rust. Do not reintroduce complete-history loading/filtering in Flutter as a normal correctness path.

## Native/generated contract

`torca-native` is the narrow process composition/native boundary. The generated contract maps user intent and presentation-safe projections.

- Flutter must not generate durable domain IDs/security-sensitive timestamps.
- Private keys/relationship secrets must not cross into presentation DTOs/logs.
- Production native failure must remain explicit; do not silently fall back to an in-memory business implementation.
- Provider/network startup must not block access to usable local encrypted state unnecessarily.
- Contract changes must update canonical schema + generated projections together.

## Security-sensitive changes

The current relationship-key design does not claim Signal-style forward secrecy/post-compromise security. Do not invent a custom ratchet merely to obtain a feature label.

Treat changes to pairing, provider bootstrap, peer authentication, cryptography, protected-secret storage, contact verification, notifications, Radio media, diagnostics and platform capture/privacy as security-sensitive.

For these changes, update the threat model/privacy/security docs when the boundary or claim changes and add negative/failure tests as well as happy paths.

## Validation

Run focused deterministic tests first, then the affected repository gates. The CI workflow is the reference for the automated matrix; [`docs/testing.md`](docs/testing.md) explains evidence levels.

Common checks:

```powershell
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D clippy::correctness -D clippy::suspicious -D clippy::perf
cargo test --workspace --all-targets --all-features --locked
```

For Flutter changes, run `flutter analyze` and `flutter test` from `apps/client/flutter`. For contract changes, run `torca-contract-gen --check`. For platform/provider behavior, build/test the affected target/provider rather than treating host-independent tests as equivalent evidence.

Never write “CI green”, “device validated”, “release ready” or “audited” without the corresponding executed evidence.

## Documentation policy

The maintained documentation is deliberately consolidated. Update an existing canonical page rather than adding a dated plan/checklist/status ledger.

- architecture/ownership -> `ARCHITECTURE.md`
- product/runtime flows -> `docs/app-flows.md`
- provider behavior -> `docs/transport.md`
- development -> `docs/development.md`
- deploy/runtime diagnostics -> `docs/operations.md`
- validation/soak -> `docs/testing.md`
- maturity -> `docs/STATUS.md`
- security/privacy -> `SECURITY.md`, threat model, `PRIVACY.md`

Temporary plans belong in issues/PRs and Git history. When a plan finishes, move durable conclusions into current-state docs rather than preserving the plan as a competing source of truth.

## Pull requests

A coherent PR should contain the smallest complete implementation, focused tests/contracts, documentation updates when behavior/guarantees change, and an exact validation section that distinguishes what was run from what was not run.
