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
without changing `runtimeId`; lifecycle events and notification cursors are process-wide. Long network
operations run outside the actor and return typed events. Flutter never owns SQL, retries, private keys,
Tor state, or message history filtering.

`torca-tor` is the only crate allowed to import Arti types. Windows and Android adapters implement only
`PlatformServices` (paths, protected secrets, device descriptor and lifecycle capabilities); composition
and domain behavior are shared.

The canonical contract is `crates/platform/torca-contract/schema/torca_contract.json`. The generator
produces the checked-in Dart projection and validates drift before builds. The native ABI exports only
the metadata, handle, generic invoke, response, shutdown and allocator symbols documented in
`crates/platform/torca-native/include/torca_native.h`.

The root snapshot contains summaries and health, not complete conversation history. Paginated queries
serve message history and search. All logs are redacted and grouped under one runtime run.
