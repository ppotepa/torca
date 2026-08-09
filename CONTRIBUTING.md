# Contributing to Torca

## Start of work

Read [`0.2_PROGRESS.md`](0.2_PROGRESS.md) before changing the repository. It is the live implementation/validation handoff. Historical 0.1 documents are reference material, not the current status source.

Work currently lands directly on `main`; each commit must leave the repository internally coherent. Never present source-complete behavior as platform/E2E validated without actual evidence.

## Developer workflow

Use only the three public root workflows:

```powershell
./scripts/build.ps1
./scripts/run.ps1
./scripts/deploy.ps1
```

- `build` owns source policy, formatting/codegen, architecture/release checks, Rust/Flutter validation and optional platform compilation.
- `run` prepares the shared native/runtime composition and launches the selected client target.
- `deploy` performs release builds, packaging and checksums.

Do not create public one-off scripts for formatting, codegen, packaging or platform bootstrap. Private helpers belong under `scripts/modules/`; `tools/build/overlays/` contains only platform templates.

The lightweight `scripts/modules/Torca.SourcePolicy.ps1` runs before expensive build work and protects the canonical Rust source roots, generic native invoke contract and absence of frontend-owned mutation ABI.

## One-client rule

Torca has one responsive Flutter client. Windows and Android are platform hosts, not separate product implementations. OS-specific Kotlin/C++ is allowed only for genuine platform capabilities such as protected key storage, notifications, lifecycle/tray, deep links and capture protection.

Flutter owns presentation/navigation/local ephemeral UI state. Rust owns identifiers, timestamps, domain workflows, persistence, Tor/pairing/peer state, delivery/retry and security policy.

## Data and SQL

Business SQL lives in `.sql` files under storage infrastructure. Commands, queries and migrations are separated and parameterized. Do not move message paging, summaries or search back into Flutter-side full-history filtering.

Runtime/application code must consume storage-owned projections rather than raw database connections.

## Native boundary

- `torca-native` is the narrowly reviewed C ABI boundary.
- The canonical contract accepts user intent; Flutter must not generate domain IDs or command timestamps.
- ABI functions expose primitive arguments/JSON projections, never Rust domain layouts or secret bytes.
- production native failure is explicit; there is no silent memory fallback.
- long-running Tor/network work must not block access to local encrypted state or Flutter UI progress.

## Security changes

Do not invent custom ratchets or cryptographic protocols. 0.2 uses authenticated encryption with protected pairwise secrets but does not claim forward secrecy/post-compromise security. Any change to this guarantee requires a reviewed standard design plus updates to `SECURITY.md` and the threat model.

A previously verified contact identity changing is a security event. Do not weaken the send block or allow reset actions to silently erase the mismatch.

## Work unit

A coherent work unit contains the smallest complete implementation, focused tests/contracts where appropriate, documentation/status updates, and exact validation evidence when it exists. One file existing is not completion; a release gate closes only after behavior is composed and validated at the relevant layer.
