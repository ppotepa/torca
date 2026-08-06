# Torca 0.1 status

This file is the live implementation checklist. Update it in the same commit that completes or invalidates an item.

Status legend: `[ ]` not started, `[-]` in progress, `[x]` complete, `[!]` blocked.

## M0 — Foundation

- [x] Root project README.
- [x] Canonical architecture entrypoint.
- [x] Versioned 0.1 documentation structure.
- [x] 0.1 scope and roadmap.
- [x] Domain map and dependency rules.
- [x] Initial architecture decision records.
- [x] Planned component README structure.
- [x] Main-only contribution policy.

## M1 — Workspace and contracts

- [ ] Rust workspace.
- [ ] Flutter workspace.
- [ ] Toolchain pinning.
- [ ] Foundation identifiers and timestamps.
- [ ] Command metadata and idempotency contract.
- [ ] Domain event envelope.
- [ ] Wire envelope primitives.
- [ ] Generated bridge contract pipeline.
- [ ] Repository validation entrypoint.
- [ ] CI boundary and build checks.

## M2 — Identity, storage and crypto

- [ ] Identity domain implementation.
- [ ] Storage port contracts.
- [ ] SQLite/SQLCipher adapter.
- [ ] SQL directory and loader conventions.
- [ ] Initial migrations.
- [ ] Key provider abstraction.
- [ ] Local profile persistence.
- [ ] Restart and migration tests.

## M3 — Pairing and contacts

- [ ] Pairing domain implementation.
- [ ] Contact domain implementation.
- [ ] Conversation domain baseline.
- [ ] Relay protocol.
- [ ] In-memory relay service.
- [ ] Two-engine pairing integration tests.

## M4 — Messaging and receipts

- [ ] Messaging domain implementation.
- [ ] Receipt domain implementation.
- [ ] Message persistence.
- [ ] Transactional outbox.
- [ ] Inbound deduplication.
- [ ] Retry scheduler.
- [ ] Conversation projections.
- [ ] Transport-independent messaging tests.

## M5 — Peer and Tor

- [ ] Peer session implementation.
- [ ] Capability handshake.
- [ ] Peer protocol codec.
- [ ] Tor process adapter.
- [ ] Onion service lifecycle.
- [ ] Reconnect and resend tests.

## M6 — Flutter and platforms

- [ ] Shared Flutter package.
- [ ] Generated Rust/Flutter bridge.
- [ ] Pairing screens.
- [ ] Contact and conversation screens.
- [ ] Windows host integration.
- [ ] Android host integration.
- [ ] Lifecycle and background recovery.

## M7 — Attachments and diagnostics

- [ ] Attachment domain and ports.
- [ ] Encrypted image flow.
- [ ] Structured redacted logs.
- [ ] Health projection.
- [ ] Diagnostic export.

## M8 — Stabilization

- [ ] End-to-end test matrix.
- [ ] Threat-model review.
- [ ] Packaging and release scripts.
- [ ] Upgrade and recovery tests.
- [ ] Release notes and known limitations.
- [ ] 0.1 test artifacts.
