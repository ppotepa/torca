# Torca 0.1 roadmap

Milestones are ordered by dependency. A later milestone may begin experimentally, but it cannot be marked complete before its prerequisites satisfy their exit criteria.

## M0 — Repository and architecture foundation

Deliverables:

- canonical README, architecture and roadmap;
- versioned documentation structure;
- mini-domain map and dependency rules;
- initial ADRs;
- planned crate and deployable-unit READMEs;
- main-only contribution policy.

Exit criteria:

- every planned component has an owner and non-responsibilities;
- no contradictory architecture descriptions remain;
- the 0.1 scope and definition of done are explicit.

## M1 — Workspace, contracts and foundation types

Deliverables:

- buildable Rust workspace and Flutter workspace;
- supported toolchain files;
- common identifiers, timestamps and command metadata;
- domain event envelope and application command envelope;
- versioned wire envelope primitives;
- one repository-wide validation command.

Exit criteria:

- clean checkout builds foundation crates;
- dependency-boundary checks run in CI;
- generated contract output is deterministic.

## M2 — Identity, storage and cryptographic base

Deliverables:

- identity domain;
- encrypted SQLite bootstrap;
- migrations and SQL loader conventions;
- key storage abstraction;
- local profile persistence;
- transaction and repository primitives.

Exit criteria:

- identity survives restart;
- secrets never appear in logs;
- database migration from empty state is repeatable;
- domain tests do not depend on SQLite.

## M3 — Pairing, contacts and rendezvous relay

Deliverables:

- pairing state machine;
- contact domain;
- direct conversation creation;
- relay protocol and in-memory relay service;
- invitation expiry, approval, rejection and cancellation;
- integration test with two independent client engines.

Exit criteria:

- accepted pairing produces matching verified contacts on both sides;
- relay restart loses only active pairing slots;
- relay cannot read private identity material;
- repeated commands do not create duplicate contacts or conversations.

## M4 — Messaging, receipts and durable delivery

Deliverables:

- messaging and receipt domains;
- message repository;
- transactional outbox and inbound deduplication;
- retry scheduler;
- conversation and message projections;
- transport-independent two-engine messaging tests.

Exit criteria:

- no accepted outbound message is lost after process interruption;
- duplicate inbound envelopes create one message;
- state transitions are validated;
- delivered and read receipts are idempotent.

## M5 — Peer protocol and Tor transport

Deliverables:

- peer session abstraction;
- capability-authenticated handshake;
- versioned peer protocol codec;
- local Tor process lifecycle;
- onion service publication and reachability checks;
- reconnect, timeout and cancellation behavior.

Exit criteria:

- two clients exchange messages through Tor without relay participation;
- interrupted transport resumes queued delivery;
- peer identity and capability mismatch are rejected;
- transport tests cover fragmentation and reconnect boundaries.

## M6 — Shared Flutter client and platform lifecycle

Deliverables:

- typed generated bridge;
- shared app shell, contact list and conversation UI;
- pairing UI;
- Windows composition root and tray behavior;
- Android composition root, lifecycle and notification integration;
- snapshot subscription and command submission only through the bridge.

Exit criteria:

- both platforms render equivalent engine state;
- UI restart does not reset engine-owned workflows;
- navigation cannot trap the user in diagnostics or pairing screens;
- platform code contains no duplicate messaging state machine.

## M7 — Attachments and diagnostics

Deliverables:

- bounded encrypted image attachment flow;
- redacted structured diagnostics;
- health snapshot and diagnostic export;
- failure injection for storage, relay and peer transport.

Exit criteria:

- attachment interruption is recoverable;
- diagnostic export contains no message plaintext, private keys or capabilities;
- failures expose actionable states without corrupting domain data.

## M8 — Stabilization and 0.1 test release

Deliverables:

- end-to-end test matrix;
- packaging scripts;
- migration and recovery tests;
- threat-model review;
- release notes and known limitations;
- signed or checksum-verifiable test artifacts where supported.

Exit criteria:

- all items in `DEFINITION_OF_DONE.md` pass;
- no unresolved critical security or data-loss defect;
- fresh installation and upgrade paths are documented and tested;
- Windows and Android complete the same primary user journey.
