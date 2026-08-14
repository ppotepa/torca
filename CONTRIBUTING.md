# Contributing to Torca

Torca is a security-sensitive application. Keep changes small enough to review, preserve the enforced architecture boundaries, and state validation evidence precisely.

## Before changing code

Read:

1. [`README.md`](README.md) for the product shape and canonical workflow;
2. [`docs/STATUS.md`](docs/STATUS.md) for current maturity and outstanding validation;
3. [`ARCHITECTURE.md`](ARCHITECTURE.md) for ownership/dependency rules;
4. [`SECURITY.md`](SECURITY.md) and [`docs/security/threat-model.md`](docs/security/threat-model.md) for security-sensitive work; and
5. [`0.3_PROGRESS.md`](0.3_PROGRESS.md) when you need the detailed current engineering handoff.

Use [`docs/README.md`](docs/README.md) to decide whether a documentation change belongs in an evergreen document or a working ledger.

## Canonical development workflow

Use the Rust deployment tool for normal build/run/deploy/device/log work:

```powershell
cargo run -p torca-deploy
```

With no subcommand it opens the Ratatui wizard. The same planner/executor is available through CLI subcommands such as `status`, `plan`, `run`, `deploy`, `rebuild`, `full-redeploy`, `relay`, `resume`, `logs` and `build`.

The remaining PowerShell scripts are compatibility, validation and maintenance helpers. Do not create a second deployment pipeline or document a lower-level helper as the primary developer workflow.

## Architectural ownership

Torca has one responsive Flutter client. Windows and Android are platform hosts of the same product, not separate business implementations.

Flutter owns:

- presentation and responsive layout;
- navigation and transient interaction state;
- local presentation preferences; and
- submission of typed user intent through `EngineGateway`.

Rust owns:

- identities, contact relationships and security-sensitive identifiers/timestamps;
- durable domain state and transactions;
- pairing, peer authentication and cryptography;
- SQLCipher persistence and protected-secret usage;
- Tor/onion lifecycle, peer connectivity and delivery/retry;
- attachments, receipts, Radio Mode policy and background coordination; and
- presentation-safe read models, diagnostics and notification intent.

Do not move durable workflow or security policy into Dart, serialization code, JNI/C++/Kotlin hosts or storage/network adapters.

## Data and SQL

Business SQL belongs in parameterized `.sql` files owned by storage infrastructure. Application/domain code consumes repositories and storage-owned projections rather than raw database connections.

Keep message history paged/searchable. Do not reintroduce complete-history loading/filtering into Flutter for normal conversation views.

## Native and generated contract

`torca-native` is the narrow process/native boundary. The generated contract accepts user intent and returns presentation-safe DTOs/read models.

- Flutter must not generate durable domain IDs or security-sensitive timestamps.
- Secret bytes and private keys must not cross into presentation contracts or logs.
- Production native failure must remain explicit; do not silently fall back to an in-memory business implementation.
- Long-running Tor/network work must not block access to local encrypted state.
- Contract changes must update the canonical schema and generated projections together.

## Security-sensitive changes

Do not invent a custom ratchet or cryptographic protocol to obtain a feature label. The current pairwise message-key design does not claim forward secrecy or post-compromise security. Any change to that guarantee needs a reviewed design, focused negative tests and updates to both `SECURITY.md` and the threat model.

Treat changes to pairing, contact verification, peer authentication, encryption, protected-secret storage, notifications, Radio Mode media, diagnostics and platform capture/privacy behavior as security-sensitive.

A previously verified contact identity changing is a security event. Do not weaken the send block or allow resets/migrations to silently inherit the previous trust decision.

## Validation

Run the narrowest deterministic tests that cover your change, then expand to the affected repository gates. The CI definition is the best reference for the full automated matrix.

Common source checks are:

```powershell
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D clippy::correctness -D clippy::suspicious -D clippy::perf
cargo test --workspace --all-targets --all-features --locked
```

For Flutter changes, run `flutter analyze` and `flutter test` from `apps/client/flutter`. For contract changes, run the generator in check mode. For platform behavior, build and test the affected platform rather than treating a host-independent check as equivalent evidence.

Never write “validated”, “release-ready”, “platform-complete” or similar wording without naming the evidence actually executed. A configured CI workflow is not a passing CI result.

## Documentation changes

Update evergreen documentation when a stable ownership, security, privacy or product rule changes. Update the detailed progress ledger when recording implementation/validation handoff.

Avoid copying rapidly changing constants, schema versions, migration counts, class inventories or test totals into evergreen documents. If a working plan becomes obsolete, preserve its historical value in Git history and move any still-valid principle into the maintained documentation.

## Work units and reviews

A coherent work unit contains the smallest complete implementation, focused tests/contracts where appropriate, documentation/status updates when behavior or guarantees change, and exact validation evidence.

Use a branch and pull request when review is useful or required by the working context. If work lands directly on `main`, each commit must still leave the repository internally coherent and must not present unexecuted validation as completed evidence.