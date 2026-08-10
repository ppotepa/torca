# Torca architecture

Torca is one Flutter presentation shared by Windows and Android and one process-owned Rust runtime.

```text
Flutter UI
  -> NativeRuntimeWorker (one Dart isolate)
  -> torca_native generic invoke ABI
  -> ProcessRuntimeRegistry
  -> TorcaRuntime single-writer actor (bounded mailbox)
       -> storage, identity/profile, embedded torca-tor, onion service,
          relay/pairing, messaging, probing, notifications and logging
```

The runtime publishes immutable revisioned snapshots. Presentation workers may detach and reattach
without changing `runtimeId`; lifecycle events and notification cursors are process-wide. Command
idempotency is bounded and applies only to successful commands; queries always observe current state.
`revision` changes only for observable runtime state transitions, not for ordinary reads. Flutter never
owns SQL, retries, private keys, Tor state, or message history filtering.

The actor serializes durable state and scheduling decisions. Network isolation is an active hardening
boundary: blocking peer/Tor operations must report typed completion results back to the actor rather
than hold an actor turn. Until that work is complete, callers must retain their defined request timeouts.

`torca-tor` is the only crate allowed to import Arti types. Windows and Android adapters implement only
`PlatformServices` (paths, protected secrets, device descriptor and lifecycle capabilities); composition
and domain behavior are shared.

The canonical operation metadata is `crates/platform/torca-contract/schema/torca_contract.json`. The
current generator derives the Rust operation allow-list and verifies the checked-in Dart projection;
full payload/type generation from the schema remains planned work. The native ABI exports only
the metadata, handle, generic invoke, response, shutdown and allocator symbols documented in
`crates/platform/torca-native/include/torca_native.h`.

The root snapshot contains summaries and health, not complete conversation history. Paginated queries
serve message history and search. The current presentation worker polls bounded snapshots and a
notification cursor; an event journal/long-poll channel is planned to replace frequent full polling.
All logs are redacted and grouped under one runtime run.
