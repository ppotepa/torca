# Torca architecture

This document is the canonical high-level architecture entrypoint. Detailed rules live in [`docs/architecture`](docs/architecture/README.md), while release-specific decisions live under [`docs/0.1`](docs/0.1/README.md).

## Architectural style

Torca is a **modular monolith** assembled from small Rust libraries and a shared Flutter client. A library boundary is introduced for every meaningful mini-domain or infrastructure capability, but deployment remains deliberately simple.

The architecture follows ports and adapters:

```text
UI and platform hosts
        |
        v
Typed bridge contract
        |
        v
ClientEngine actor
        |
        v
Application workflows and projections
        |
        v
Domain libraries and declared ports
        ^
        |
Storage, crypto, peer, Tor and rendezvous adapters
```

## Deployable units

Version 0.1 has two deployable units:

1. **Torca client** — Windows and Android compositions sharing the Rust engine and Flutter UI.
2. **Torca relay** — an untrusted, in-memory rendezvous broker used only during pairing.

The relay is not a message server, account server, presence server, user directory, backup service, or offline mailbox.

## Component groups

### Foundation

Small stable value types and utilities that do not contain product workflows. Foundation code must remain dependency-light.

### Domains

Each mini-domain owns its vocabulary and invariants:

- identity;
- contacts;
- pairing;
- conversations;
- messaging;
- receipts;
- attachments;
- presence;
- notifications.

A domain library may depend on foundation libraries and explicitly approved domain contracts. It must not depend on infrastructure or presentation packages.

### Application

Application libraries coordinate use cases across domains. The `ClientEngine` is a single-writer actor that serializes state-changing commands, schedules work, dispatches domain events, and publishes UI projections.

### Infrastructure

Infrastructure libraries implement ports declared by domains or application libraries. They own SQLite/SQLCipher access, cryptographic provider integration, peer sessions, Tor process integration, file storage, clocks, and operating-system adapters.

### Protocol

Protocol libraries define versioned wire representations. Domain objects are never serialized directly. Mapping between domain and wire types occurs in dedicated codecs.

### Platform

Platform packages expose a generated, typed contract to Flutter and provide the minimal native bootstrap required by each operating system.

## Core rules

1. Domain code contains no SQL, sockets, Flutter types, FFI types, or Tor process calls.
2. UI sends commands and renders snapshots; it does not implement a second state machine.
3. All state-changing client operations pass through the `ClientEngine` actor.
4. Storage owns transactions and raw database connections.
5. SQL lives in parameterized `.sql` files grouped as migrations, commands, and queries.
6. Mutating commands carry stable `command_id` values and must be idempotent.
7. Outbound delivery uses a durable outbox. Inbound delivery uses deduplication.
8. Wire protocols are explicitly versioned and tolerant of unknown optional fields.
9. Cross-domain effects are coordinated through application handlers, not hidden domain-to-domain calls.
10. `main` must remain internally consistent after every commit.

## Primary flows

### Pairing

```text
Create or join pairing session
        -> rendezvous relay exchanges opaque pairing material
        -> both users explicitly approve
        -> identities and capabilities are verified
        -> contact is created
        -> direct conversation is created
        -> peer endpoint is registered
```

### Sending a message

```text
UI command
    -> ClientEngine
    -> messaging domain validates and creates message
    -> storage transaction persists message and outbox item
    -> delivery worker encodes, encrypts and sends peer envelope
    -> acknowledgement updates delivery state
    -> projection update reaches UI
```

### Receiving a message

```text
Tor peer stream
    -> authenticated peer session
    -> decrypt and decode protocol envelope
    -> deduplicate
    -> messaging domain accepts message
    -> storage transaction persists message and receipt work
    -> projection and notification handlers run
```

## Source of truth

- Long-lived architectural rules: [`docs/architecture`](docs/architecture/README.md)
- Version 0.1 scope and sequence: [`docs/0.1`](docs/0.1/README.md)
- Accepted decisions: [`docs/decisions`](docs/decisions/README.md)
- Live delivery state: [`docs/0.1/STATUS.md`](docs/0.1/STATUS.md)
