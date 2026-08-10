# TorcaRuntime architecture

`TorcaRuntime` is the process-owned coordination boundary for the Windows and Android client. It is
exposed through the `torca-native` generic ABI and is presented to Flutter by one
`NativeRuntimeWorker` isolate.

## Responsibilities

- serialize state-changing requests through a bounded mailbox;
- coordinate identity, storage, bootstrap, Tor, relay, pairing and communication workflows;
- publish immutable root snapshots with a monotonic state revision;
- own command idempotency, lifecycle handling, notification cursors and controlled shutdown; and
- schedule durable work and accept typed worker completion results.

## Non-responsibilities

The runtime must not contain domain invariants owned by mini-domain crates, raw SQL or row mapping,
cryptographic algorithm implementations, wire byte codecs, Flutter navigation, or direct system
notification rendering.

## Actor model

```text
Flutter commands / lifecycle / timers / worker results
                         |
                         v
                 bounded runtime mailbox
                         |
                         v
              TorcaRuntime single-writer actor
                 |                 |
                 v                 v
       storage/domain commits   network work scheduling
                                   |
                                   v
                         typed completion event
                                   |
                                   +----> mailbox
```

The actor owns state transitions and durable commits. Network code may wait for Tor, a peer ACK or an
attachment transfer only outside the actor, then returns a typed result. This isolation is a required
runtime invariant and remains an active hardening area where legacy synchronous adapters exist.

## Snapshots, requests and recovery

Only successful commands are idempotency-cached, and the cache is bounded. Queries always execute
against current runtime state. A snapshot revision advances only when observable state changes, allowing
Flutter to discard stale projections safely.

The root snapshot contains bootstrap, profile, health, contact and conversation summaries. It omits
message history; conversation pages and search are explicit bounded queries. Shutdown stops external
mutations, persists required transitions, stops workers/listeners/Tor, then closes storage.
