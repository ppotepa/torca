# Torca

Torca is a privacy-focused, peer-to-peer messenger built around local identities, Tor onion services, encrypted local storage and explicit contact pairing.

This repository is a clean implementation of the product. The previous [`ppotepa/tOrca`](https://github.com/ppotepa/tOrca) repository remains a source of requirements, protocol lessons and selected reviewed implementations, but it is not the structural template for this codebase.

## Start here

Development targets **Torca 0.1**. Read [`0.1_PROGRESS.md`](0.1_PROGRESS.md) before changing the repository. It is the canonical record of source coverage, release gaps, validation evidence, bugs and the next action.

## Current source baseline

All 20 roadmap batches now have committed source and documentation covering:

- pinned Rust and Flutter workspaces;
- mini-domain libraries for identity, contacts, pairing, conversations, messaging, receipts and attachments;
- compile-time SQL, migrations, outbox and deduplication contracts;
- cryptographic provider boundaries and redacted key types;
- generic wire, relay and authenticated peer protocols;
- ephemeral rendezvous relay semantics;
- single-writer ClientEngine and projections;
- Tor process/onion/SOCKS integration;
- generated Rust/Dart contract and shared Flutter client shell;
- Windows and Android host composition contracts;
- atomic encrypted attachment storage;
- diagnostics, fault injection, integration-test journeys and release tooling.

Source-roadmap coverage is **20/20 (100%)**. This is not the same as a validated binary release. Production providers, native platform builds and owner-run tests are tracked explicitly in [`0.1_PROGRESS.md`](0.1_PROGRESS.md) and [`docs/0.1/RELEASE_CHECKLIST.md`](docs/0.1/RELEASE_CHECKLIST.md).

## Architectural direction

```text
Flutter client
    |
generated contract and native gateway
    |
ClientEngine actor — single writer
    |
application workflows and projections
    |
focused mini-domain libraries
    |
storage, crypto, relay, peer and Tor adapters
```

Domains do not depend on Flutter, sockets, SQLite implementations or platform glue. The relay remains an untrusted, ephemeral pairing broker. Normal contact traffic is direct through Tor.

## Repository layout

```text
apps/client/             shared client and platform host contracts
crates/foundation/       dependency-light shared primitives
crates/domains/          independent mini-domain libraries
crates/application/      engine, projections and diagnostics
crates/infrastructure/   storage, crypto, files, peer and Tor adapters
crates/protocol/         versioned wire contracts
tools/                   deterministic contract generation
services/relay/          ephemeral rendezvous broker
tests/torca-integration/ cross-crate primary-journey tests
docs/0.1/                release scope, design, progress support and gates
```

## Local workflow

Format generated source first:

```powershell
./scripts/format.ps1
```

Then run the complete owner validation suite:

```powershell
./scripts/validate.ps1
```

Package only after all release gates pass:

```powershell
./scripts/package.ps1 -Target all
```

## Canonical documents

- [`0.1_PROGRESS.md`](0.1_PROGRESS.md) — quantitative status and handoff.
- [`ARCHITECTURE.md`](ARCHITECTURE.md) — system boundaries.
- [`docs/0.1/IMPLEMENTATION_ORDER.md`](docs/0.1/IMPLEMENTATION_ORDER.md) — 20 implementation batches.
- [`docs/0.1/TEST_MATRIX.md`](docs/0.1/TEST_MATRIX.md) — automated and platform test matrix.
- [`docs/0.1/KNOWN_LIMITATIONS.md`](docs/0.1/KNOWN_LIMITATIONS.md) — honest limitations.
- [`docs/0.1/RELEASE_CHECKLIST.md`](docs/0.1/RELEASE_CHECKLIST.md) — binary release gate.
- [`docs/security/threat-model.md`](docs/security/threat-model.md) — assets, boundaries and threats.
- [`docs/decisions`](docs/decisions/README.md) — architecture decision records.

All work currently lands directly on `main`. See [`CONTRIBUTING.md`](CONTRIBUTING.md).
