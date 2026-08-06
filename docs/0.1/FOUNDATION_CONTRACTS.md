# Torca 0.1 foundation contracts

This document records the shared contracts introduced in Batch 02. The implementation lives in `crates/foundation/torca-foundation`.

## Identifier policy

The foundation stores identifiers as opaque 128-bit values with a canonical 32-character hexadecimal representation. It intentionally does not choose a random or time-based generation algorithm.

The foundation owns only process-level identifiers:

- `CommandId` for command idempotency;
- `CorrelationId` for one logical workflow;
- `CausationId` for the direct command or event cause;
- `EventId` for one immutable event occurrence.

Each mini-domain owns its own identifiers, for example `MessageId`, and wraps `OpaqueId` rather than using untyped strings.

The all-zero value is representable for decoding and diagnostics but must not be generated for persisted production entities.

## Time policy

`Timestamp` is an integer number of UTC milliseconds from Unix epoch, bounded through `9999-12-31T23:59:59.999Z`.

Timestamps are diagnostic facts. They do not establish authoritative ordering between remote devices. ClientEngine serialization and durable local sequence numbers will establish local ordering in later batches.

## Command contract

Every externally retryable state mutation uses `CommandMetadata`:

- stable `command_id`;
- diagnostic `issued_at` timestamp;
- workflow `correlation_id`;
- optional direct `causation_id`.

A root command derives its correlation identifier from its command identifier. Follow-up commands retain the same correlation identifier and identify their direct cause.

`CommandEnvelope<C>` is generic and does not define serialization, dispatch or persistence.

## Event contract

`EventMetadata` identifies an immutable event occurrence, its diagnostic timestamp, workflow correlation and direct cause.

`DomainEventEnvelope<E>` is internal application data. It is not automatically a network message, database record or Flutter event. Protocol and persistence adapters map it explicitly when required.

## Error contract

Domain errors remain domain-specific Rust error types. Errors that cross an application boundary expose a non-sensitive `ErrorDescriptor` containing:

- a stable lowercase machine code;
- a broad category;
- retry advice.

Error descriptors do not contain plaintext messages, keys, capabilities, SQL details or filesystem paths.

## Cancellation contract

`CancellationProbe` is read-only and independent from Tokio or another async runtime. Concrete engine and platform adapters may implement it using their chosen cancellation primitive without leaking that runtime into domain crates.

## Dependency rule

The package has no third-party dependencies. Adding one requires evidence that at least several mini-domains need it and that the dependency does not introduce storage, transport, serialization, cryptographic or platform policy into foundation.

## Validation ownership

The implementation includes unit tests for parsing, bounds, metadata propagation, error classification and cancellation behavior. Full Rust and repository validation is executed locally by the project owner and recorded in `0.1_PROGRESS.md` when results are available.
