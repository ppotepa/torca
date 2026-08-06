# Torca

Torca is a privacy-focused, peer-to-peer messenger built around local identities, Tor onion services, encrypted local storage, and explicit contact pairing.

This repository is a clean implementation of the product. The previous [`ppotepa/tOrca`](https://github.com/ppotepa/tOrca) repository remains a source of requirements, protocol lessons, test cases, and selected proven implementations, but it is not the structural template for this codebase.

## Start here

Development currently targets **Torca 0.1**. Before making any change, read [`0.1_PROGRESS.md`](0.1_PROGRESS.md). It is the single canonical record of implementation state, validation evidence, blockers and the exact next action. Every coherent implementation change must update that file in the same commit.

## Current target

Version `0.1` is the first coherent engineering baseline: stable module boundaries, typed contracts, local persistence, pairing, direct messaging, durable retry, and shared client behavior across supported platforms. Version `1.0` is outside the current planning horizon.

## Product principles

- Privacy is the default.
- Client devices own identities, keys, contacts, conversations and history.
- The relay is an untrusted, ephemeral rendezvous service used only for pairing.
- Contact communication is direct peer-to-peer through Tor onion services.
- Delivery is durable, retryable, idempotent and observable.
- Windows and Android share the same Rust engine and Flutter presentation where practical.
- Domain rules live in focused libraries, not UI, storage, transport or platform glue.

## Architectural direction

```text
Flutter client
    |
Generated bridge and typed client contract
    |
ClientEngine actor — single writer and workflow coordinator
    |
Application services and projections
    |
Mini-domain libraries
    |
Ports implemented by storage, crypto, peer, Tor and rendezvous adapters
```

Infrastructure and wire protocols are separate from domain models. See [`ARCHITECTURE.md`](ARCHITECTURE.md).

## Repository layout

```text
apps/client/flutter/    shared Flutter client
crates/foundation/      dependency-light shared primitives
crates/domains/         independent mini-domain libraries
crates/application/     engine, workflows and projections
crates/infrastructure/  storage, crypto, peer and Tor adapters
crates/protocol/        versioned wire contracts
crates/platform/        bridge and generated contracts
services/relay/         ephemeral pairing rendezvous
docs/0.1/               active scope, roadmap and acceptance criteria
```

## Toolchain and validation

The initial workspace pins Rust `1.97.1`, Flutter `3.44.7` and Dart `3.12.0`. See [`docs/0.1/TOOLCHAIN.md`](docs/0.1/TOOLCHAIN.md).

Run from the repository root:

```powershell
./scripts/validate.ps1
```

GitHub Actions invokes the same command.

## Canonical documents

- [`0.1_PROGRESS.md`](0.1_PROGRESS.md) — live progress and handoff.
- [`ARCHITECTURE.md`](ARCHITECTURE.md) — system structure.
- [`ROADMAP.md`](ROADMAP.md) — active delivery plan.
- [`docs/0.1/SCOPE.md`](docs/0.1/SCOPE.md) — exact 0.1 scope.
- [`docs/0.1/IMPLEMENTATION_ORDER.md`](docs/0.1/IMPLEMENTATION_ORDER.md) — ordered batches.
- [`docs/0.1/DEFINITION_OF_DONE.md`](docs/0.1/DEFINITION_OF_DONE.md) — completion gate.
- [`docs/architecture/DOMAIN_MAP.md`](docs/architecture/DOMAIN_MAP.md) — mini-domain ownership.
- [`docs/decisions`](docs/decisions/README.md) — accepted ADRs.

## Development workflow

All work currently lands directly on `main`. Every commit must leave the repository internally consistent and update [`0.1_PROGRESS.md`](0.1_PROGRESS.md). See [`CONTRIBUTING.md`](CONTRIBUTING.md).

## Status

M0 is complete. Batch 01 has introduced the initial Rust and Flutter workspaces, pinned toolchains, repository validation script and CI workflow. The authoritative validation state and next action are recorded in [`0.1_PROGRESS.md`](0.1_PROGRESS.md).
