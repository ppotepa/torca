# Command and event model

## Commands

A command represents a requested state change. Every externally retryable mutation carries:

- `command_id` — stable idempotency key;
- `issued_at` — diagnostic timestamp, not ordering authority;
- command-specific payload;
- optional correlation and causation identifiers.

The engine records completed command identifiers at the transaction boundary required to prevent duplicate effects.

Commands use imperative names such as `SendMessage`, `ApprovePairing` and `BlockContact`.

## Domain events

A domain event is a fact produced after an invariant-preserving state transition, for example `MessageQueued` or `PairingCompleted`.

Domain events:

- are immutable values;
- use past-tense names;
- contain identifiers and facts needed by immediate handlers;
- do not contain database connections or service handles;
- are not automatically public network events.

## Application events and projections

Application handlers translate domain events into cross-domain work, durable jobs and projection updates. Flutter receives typed snapshots or presentation events, not internal domain event streams.

## Transaction boundaries

Events that require durable follow-up are stored with the state change in the same transaction, normally through an outbox or durable work table. In-memory publication may improve latency but cannot be the sole reliability mechanism.

## Idempotency

Idempotency is required at several layers:

- command deduplication by `command_id`;
- message deduplication by stable message or envelope identifier;
- receipt monotonicity;
- pairing completion uniqueness;
- outbox attempt retry;
- protocol acknowledgement replay.

A retry must return the original successful result when practical rather than inventing a second resource.

## Ordering

The ClientEngine serializes local mutations. Durable local sequence numbers establish projection order. Remote wall-clock timestamps are metadata and cannot override local integrity rules.
