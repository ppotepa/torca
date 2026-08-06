# Torca 0.1 implementation order

This sequence is the default order for implementation batches. It is more granular than the milestone roadmap and is intended to prevent circular dependencies and premature platform work.

The current active batch and exact handoff are always recorded in [`../../0.1_PROGRESS.md`](../../0.1_PROGRESS.md).

## Batch 01 — Repository toolchain

Create the Rust workspace, Flutter workspace, toolchain pins, formatting configuration, validation scripts and CI skeleton. The repository must build before domain code is added.

## Batch 02 — Foundation contracts

Implement opaque identifiers, timestamps, command metadata, cancellation primitives, error conventions and event envelopes. Avoid generic utility dumping; every foundation type must have multiple legitimate consumers.

## Batch 03 — Protocol base

Implement versioned wire envelope headers, codec errors, size limits and compatibility tests. Do not add messaging payloads yet.

## Batch 04 — Identity domain

Implement installation identity, public identity, profile, lifecycle rules and required key-provider and repository ports using in-memory test doubles.

## Batch 05 — Storage kernel

Implement database bootstrap, SQL file loader, migration runner, transaction abstraction and identity repository. Establish the final SQL directory convention before adding more tables.

## Batch 06 — Crypto adapters

Implement key generation, protected local key storage, signing, verification and payload sealing APIs needed by identity and pairing. Keep algorithm choices behind narrow interfaces.

## Batch 07 — Pairing and contacts domains

Implement invitation, pairing session, approval rules, contact aggregate and direct conversation creation contracts. All workflows must run against in-memory ports first.

## Batch 08 — Rendezvous protocol and relay

Implement opaque relay messages, in-memory slot lifecycle, expiry and two-client integration tests. The relay must remain independent from client domain crates.

## Batch 09 — ClientEngine baseline

Introduce the single-writer actor, command dispatch, event dispatch, scheduler abstraction and snapshot publication. Wire identity and pairing workflows before messaging.

## Batch 10 — Messaging and receipts domains

Implement message state transitions, reply references, receipts and invalid-transition tests. Transport remains an abstract port.

## Batch 11 — Durable delivery storage

Add message tables, outbox, deduplication, receipt work and projections using atomic transactions. Add crash-boundary tests around commit points.

## Batch 12 — Peer protocol

Define authenticated handshake and versioned messaging payloads. Add codec test vectors and strict size limits.

## Batch 13 — Peer sessions

Implement transport-independent session lifecycle, acknowledgements, timeouts, reconnect state and delivery worker coordination using simulated duplex streams.

## Batch 14 — Tor adapter

Implement Tor process management, onion service publication, SOCKS connection, health checks and shutdown. Prove two clients exchange data through Tor before UI integration.

## Batch 15 — Generated bridge

Define stable command, result, event and snapshot contracts. Generate Rust and Dart bindings and prevent handwritten duplicates.

## Batch 16 — Shared Flutter shell

Implement startup, identity setup, pairing, contact list and conversation views against engine snapshots. Keep presentation state local and workflow state in the engine.

## Batch 17 — Windows host

Implement process composition, single-instance behavior, tray lifecycle, notifications, diagnostics and clean shutdown.

## Batch 18 — Android host

Implement runtime loading, permissions, background constraints, notifications, lifecycle recovery and safe navigation.

## Batch 19 — Attachments

Add bounded encrypted image attachments only after text delivery is stable under restart and reconnect tests.

## Batch 20 — Stabilization

Complete failure injection, diagnostic export, packaging, threat-model review, migration tests, release notes and the full end-to-end matrix.

## Batch completion rule

A batch is complete only when its public contracts, implementation, tests, documentation and root [`0.1_PROGRESS.md`](../../0.1_PROGRESS.md) update land together on `main`.

The progress update must include exact validation commands and results, remaining work, blockers and one exact next action.
