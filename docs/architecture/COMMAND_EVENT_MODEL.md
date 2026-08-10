# Command and event model

## Commands

A command requests a state change. Every externally retryable mutation carries:

- `requestId`: stable transport idempotency key;
- `issuedAt`: diagnostic timestamp, not ordering authority;
- command-specific payload; and
- optional correlation and causation identifiers.

The runtime records completed successful command identifiers at the transaction boundary required to
prevent duplicate effects. The ledger is bounded and expires entries; it is not a permanent cache.

Commands use imperative names such as `message.send`, `pairing.approve` and `contact.block`.

## Domain events

A domain event is a fact produced after an invariant-preserving state transition, for example
`MessageQueued` or `PairingCompleted`.

Domain events are immutable values, use past-tense names, carry identifiers and facts needed by
immediate handlers, and never contain database connections or service handles. They are not
automatically public network events.

## Application events and projections

Application handlers translate domain events into cross-domain work, durable jobs and projection updates.
Flutter receives typed snapshots and cursor-addressed projections, not internal domain event streams.

## Transaction boundaries

Events requiring durable follow-up are stored with their state change in the same transaction, normally
through an outbox or durable work table. In-memory publication may improve latency but cannot be the
sole reliability mechanism.

## Idempotency

Idempotency is required at several layers:

- command deduplication by `requestId`;
- message deduplication by stable message or envelope identifier;
- receipt monotonicity;
- pairing completion uniqueness;
- outbox attempt retry; and
- protocol acknowledgement replay.

A retry returns the original successful result when practical rather than inventing a second resource.
Queries, lifecycle notifications and parsers are never served from the command idempotency ledger.

## Ordering

The process runtime serializes local mutations. Durable local sequence numbers establish projection order.
Remote wall-clock timestamps are metadata and cannot override local integrity rules. Snapshot revisions
advance only for observable state transitions, never merely because a query was read.
