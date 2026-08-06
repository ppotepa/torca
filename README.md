# Torca

Torca is a privacy-focused, peer-to-peer messenger built around local identities, Tor onion services, encrypted local storage, and explicit contact pairing.

This repository is a clean implementation of the product. The previous [`ppotepa/tOrca`](https://github.com/ppotepa/tOrca) repository remains a source of requirements, protocol lessons, test cases, and selected proven implementations, but it is not the structural template for this codebase.

## Current target

Development currently targets **Torca 0.1**.

Version `0.1` is the first coherent engineering baseline: a minimal but complete architecture, stable module boundaries, typed contracts, local persistence, pairing, direct messaging, durable retry, and shared client behavior across supported platforms.

Version `1.0` is the eventual production release, but it is intentionally outside the current planning horizon. All active planning and acceptance criteria live under [`docs/0.1`](docs/0.1/README.md).

## Product principles

- Privacy is the default, not an optional mode.
- Client devices own identities, keys, contact state, conversations, and message history.
- The relay is an untrusted and ephemeral rendezvous service used only for pairing.
- Contact communication is direct peer-to-peer through Tor onion services.
- Message delivery is durable, retryable, idempotent, and observable.
- Windows and Android share the same Rust application engine and the same Flutter presentation layer wherever practical.
- Domain rules live in focused libraries, not in UI, storage, transport, or platform glue.
- Architecture should remain simple enough to understand from the repository layout.

## Architectural direction

Torca is designed as a modular monolith composed from small libraries. Each meaningful mini-domain owns its vocabulary, invariants, commands, events, errors, and ports.

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

The initial mini-domains are:

- identity;
- contacts;
- pairing;
- conversations;
- messaging;
- receipts;
- attachments;
- presence;
- notifications.

Infrastructure and wire protocols are separate from those domains. A domain model must not depend on SQLite, Flutter, FFI, sockets, Tor process management, or wire serialization.

See [`ARCHITECTURE.md`](ARCHITECTURE.md) for the canonical architecture overview.

## Repository plan

```text
apps/                   deployable client compositions
crates/
  foundation/           shared value types and low-level utilities
  domains/              independent mini-domain libraries
  application/          workflow coordination, engine and projections
  infrastructure/       storage, cryptography, peer and Tor adapters
  protocol/             versioned wire contracts
  platform/             Flutter/native bridge and generated contracts
services/                deployable server-side components
  relay/                 ephemeral pairing rendezvous service
docs/
  0.1/                  active version scope, roadmap and acceptance criteria
  architecture/         long-lived architecture rules
  decisions/            architecture decision records
```

The directories are introduced through documentation first. Source crates are added only when their milestone begins, so the workspace remains buildable at every step.

## Canonical documents

- [`ARCHITECTURE.md`](ARCHITECTURE.md) — system structure and dependency direction.
- [`ROADMAP.md`](ROADMAP.md) — entrypoint to the active delivery plan.
- [`docs/0.1/SCOPE.md`](docs/0.1/SCOPE.md) — exact product scope for version 0.1.
- [`docs/0.1/ROADMAP.md`](docs/0.1/ROADMAP.md) — ordered milestones and exit criteria.
- [`docs/0.1/STATUS.md`](docs/0.1/STATUS.md) — live implementation checklist.
- [`docs/0.1/DEFINITION_OF_DONE.md`](docs/0.1/DEFINITION_OF_DONE.md) — completion rules.
- [`docs/architecture/DOMAIN_MAP.md`](docs/architecture/DOMAIN_MAP.md) — ownership of every mini-domain.
- [`docs/decisions`](docs/decisions/README.md) — accepted architectural decisions.

When documents conflict, the more specific versioned document wins for the active release. Accepted architecture decision records override prose descriptions until superseded.

## Development workflow

All work currently lands directly on the `main` branch. The branch must remain buildable and internally consistent after every commit.

Changes should be small, milestone-oriented, and accompanied by updates to the relevant documentation and status checklist. No parallel legacy architecture will be maintained in this repository.

See [`CONTRIBUTING.md`](CONTRIBUTING.md) for repository rules.

## Status

The repository is in the **0.1 foundation phase**. Documentation and component contracts are being established before production code is introduced.
