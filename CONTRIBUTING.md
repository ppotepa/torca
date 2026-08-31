# Contributing to Torca

Torca is a security-sensitive Rust/Flutter application. Keep changes reviewable, preserve enforced architecture boundaries, and report validation evidence precisely.

## Read first

1. [`README.md`](README.md) — product shape and canonical entry points.
2. [`ARCHITECTURE.md`](ARCHITECTURE.md) — ownership, layer and provider boundaries.
3. [`docs/STATUS.md`](docs/STATUS.md) — current maturity and missing release evidence.
4. [`docs/app-flows.md`](docs/app-flows.md) — current product/runtime journeys.
5. [`docs/transport.md`](docs/transport.md) — communication-provider model.
6. [`SECURITY.md`](SECURITY.md), [`docs/security/threat-model.md`](docs/security/threat-model.md) and [`PRIVACY.md`](PRIVACY.md) for security/privacy-sensitive work.
7. [`docs/development.md`](docs/development.md) and [`docs/testing.md`](docs/testing.md) for workflow/evidence.
8. [`docs/versioning-and-releases.md`](docs/versioning-and-releases.md) when changing public compatibility or release metadata.

## Canonical development workflow

Use the Rust deployment tool for normal build/run/deploy/device/log work:

```powershell
cargo run -p torca-deploy
```

No subcommand opens the interactive deployment UI. CLI commands use the same planner/executor for automation. PowerShell scripts are compatibility, policy, validation and measurement helpers rather than a second deployment architecture.

## Architectural ownership

Flutter owns presentation, responsive layout, navigation, transient interaction state and presentation preferences. It submits typed user intent through `EngineGateway`.

Rust owns identities, durable workflow state, security-sensitive identifiers/timestamps, pairing, peer authentication, cryptography, persistence, provider composition, delivery/retry, attachments, receipts, Radio Mode policy, diagnostics and presentation-safe projections.

Platform code owns genuine OS integration: lifecycle, paths, protected secret stores, notifications, deep links, capture/window/permission behavior and final host composition.

Do not move durable/security state machines into Dart, serialization code, Kotlin/C++ hosts, SQL repositories or transport adapters.

## Communication providers

The application/runtime is provider-neutral above the transport/provider APIs. Iroh is the sole production provider; Memory is reserved for deterministic tests.

Provider work must not:

- branch product workflows in Flutter based on provider names;
- leak Iroh/QUIC/relay endpoint types into application or domain layers;
- silently substitute another provider when the production composition expects Iroh;
- fabricate legacy Tor/onion state for Iroh; or
- claim anonymity for direct or relay Iroh paths.

A future provider requires neutral-API implementation, conformance/integration coverage, production composition changes and security/privacy documentation of its metadata exposure.

## Data, SQL and history

Business SQL belongs in storage infrastructure and should remain parameterized and bounded. Application/domain code consumes repositories/read models rather than raw database connections.

Conversation history/search is paged/query-driven from Rust. Do not reintroduce complete-history loading/filtering in Flutter as a normal correctness path.

If a change intentionally makes existing installed data incompatible, update storage compatibility/epoch behavior and release metadata together. Do not silently reinterpret old provider/storage state.

## Native/generated contract

`torca-native` is the narrow process composition/native boundary. The generated contract maps user intent and presentation-safe projections.

- Flutter must not generate durable domain IDs/security-sensitive timestamps.
- Private keys, database keys and relationship secrets must not cross into presentation DTOs/logs.
- Production native failure must remain explicit; do not silently fall back to an in-memory business implementation.
- Provider/network startup must not unnecessarily block access to usable local encrypted state.
- Contract changes must update the canonical schema and generated projections together.

## Security-sensitive changes

The current relationship-key design does not claim Signal-style forward secrecy or post-compromise security. Do not invent a custom cryptographic ratchet merely to obtain a feature label.

Treat changes to pairing, provider bootstrap, peer authentication, cryptography, protected-secret storage, contact verification, notifications, Radio media, diagnostics and platform capture/privacy as security-sensitive.

For these changes, review/update security/privacy/threat-model boundaries and add negative/failure tests as well as happy paths.

## Versioning and changelog

Follow [`docs/versioning-and-releases.md`](docs/versioning-and-releases.md).

When a change is notable to users/operators/developers or changes compatibility/security behavior, update [`CHANGELOG.md`](CHANGELOG.md) under `Unreleased` in the same change.

Do not bump the product SemVer just to distinguish two builds. Use the artifact build number for repackaging when product compatibility/identity has not changed. Conversely, do not hide a breaking contract/storage/wire/ABI change behind an unchanged compatibility marker.

## Validation

Run focused deterministic tests first, then the affected repository gates. The CI workflow is the executable reference for the automated matrix; [`docs/testing.md`](docs/testing.md) defines evidence levels and minimum expectations.

Common Rust checks include:

```powershell
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D clippy::correctness -D clippy::suspicious -D clippy::perf
cargo test --workspace --all-targets --all-features --locked
```

For Flutter changes, run `flutter analyze` and `flutter test` from `apps/client/flutter`. For contract changes, run the contract generator in check mode. For platform/provider behavior, build/test the affected target and add device/network evidence when the behavior depends on real OS/network conditions.

Never write “CI green”, “device validated”, “release ready”, “signed” or “audited” without the corresponding executed evidence.

## Documentation policy

Use [`docs/README.md`](docs/README.md) to find the canonical page. Update an existing current-state page instead of adding a competing dated plan/status ledger.

- architecture/ownership -> `ARCHITECTURE.md`
- maturity -> `docs/STATUS.md`
- product/runtime flows -> `docs/app-flows.md`
- provider behavior -> `docs/transport.md`
- development -> `docs/development.md`
- deploy/runtime diagnostics -> `docs/operations.md`
- validation/soak -> `docs/testing.md`
- version/release compatibility -> `docs/versioning-and-releases.md`
- security/privacy -> `SECURITY.md`, threat model, `PRIVACY.md`

Durable design rationale belongs in an ADR. Dated validation reports belong in `docs/validation/` and must identify the commit/environment/run they describe. Temporary implementation plans belong in issues/PRs/work tracking and Git history.

## Change reporting

A coherent change should contain the smallest complete implementation, focused tests/contracts, documentation/changelog updates when behavior or guarantees change, and an exact validation section distinguishing what was run from what was not run.
