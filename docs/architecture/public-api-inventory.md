# Public API inventory

This inventory describes intentional cross-crate surfaces after the 0.1 maintainability cleanup. New `pub` items should fit one of these boundaries; tests should prefer crate-local access instead of expanding production API.

## `torca-client-engine`

Intentional:
- `EngineCommand`, `EngineResult`, `EngineError`;
- `ClientSnapshot`, `AvatarGenomeRecord`;
- `ClientEngine`, `EngineRuntime`, `EngineHandle`, `ClientEngineActor`;
- `RelationshipRepository` for durable infrastructure implementations;
- `InMemoryRelationshipRepository` as the supported in-memory engine composition.

Compatibility-only:
- legacy value-namespace `EngineError(String)` constructor; it redacts to `EngineError::Repository` and should disappear after all downstream constructors are typed.

Not API:
- actor requests/mailbox helpers;
- per-domain dispatch functions;
- repository mapping helpers.

## `torca-runtime`

Intentional:
- runtime state/snapshot types (`TorState`, `OnionServiceState`, peer health/activity, `NetworkSnapshot`);
- `RuntimeDriverError`;
- narrow application ports (`PairingDriver`, `PeerSessionPort`, relationship/read/attachment ports, `TorDriver`, `RelayProbe`);
- `RuntimeHandle`, `RuntimeOwner`;
- attachment request/view boundary types.

Not API:
- `RuntimeCommand`, wait/deadline helpers;
- runtime state buckets/counters;
- lease-owner namespaces;
- command dispatcher and maintenance phases.

## `torca-pairing-coordinator`

Intentional:
- protocol-independent pairing coordinator/runtime types and errors;
- pairing crypto/rendezvous/approval/secret-store ports;
- `PairingTransportSnapshot` only as a protected-storage boundary;
- invitation URI facade that delegates grammar ownership to protocol code.

Not API:
- binary persisted-state codec helpers;
- local/remote offer maps and completion sets;
- transport-session internals.

## `torca-peer-link`

Intentional:
- `PeerLink`;
- `PeerConnectionState`, `PeerLinkError`, `PeerLinkReport`;
- `InboundPeerEnvelope`, `LinkAck`, payload-free `PeerActivitySnapshot`.

Not API:
- ACK waiter state machine;
- reconnect entries/backoff helpers;
- handshake/auth outcome helpers;
- telemetry reducers/session map implementation.

## `torca-native`

Intentional:
- exported C ABI/JNI symbols defined by the contract/header;
- process metadata and opaque runtime handle ABI;
- crate-internal native composition functions needed by platform glue (`pub(crate)` only).

Not Rust API:
- native actor messages/state;
- registry internals;
- idempotency ledger;
- bridge decoder helpers;
- `TorcaRuntime` implementation methods not called by the process adapter.

## `torca-foundation`

Intentional:
- opaque IDs, timestamps, command/event metadata;
- error classification vocabulary;
- cancellation vocabulary;
- `SecretBytes<N>` and `WakeSlot` as dependency-light ownership primitives.

Not API:
- wake callback alias or internal mutex representation;
- secret storage representation.

## Rules

1. `pub` means another crate is an intended consumer.
2. `pub(crate)` means platform/composition wiring inside the crate requires it.
3. Tests do not justify a new public symbol by themselves.
4. Compatibility-only items need a named removal condition.
5. Infrastructure types must not leak into application public ports.
6. Platform ABI is defined by the contract/header, not by incidental Rust visibility.
