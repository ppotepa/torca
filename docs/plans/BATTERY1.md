# BATTERY1 — event-driven runtime control

## Decision

Torca must not use periodic background rendezvous as its default availability
mechanism.  A runtime wake is justified only by a command, a platform event,
an explicit deadline, or durable work.  The runtime owner is the single
authority that evaluates those reasons.

The default lifecycle is:

```text
foreground -> background grace (30 s) -> soft dormant
```

There is no recurring five-minute wake and no 90-second background relay
lease.  A pending delivery, attachment, pairing operation, radio session or
incoming work can still hold a durable lease independently of the UI.

## Invariants

- A known contact creates no worker, timer, probe or dial by itself.
- Attention is ephemeral and released immediately when the UI releases it or
  the host backgrounds.  It never substitutes durable delivery ownership.
- TX, RX, ACK and handshake evidence refresh health before a cosmetic probe is
  considered.
- Network changes are events and recover only demanded work.
- The actor has one deadline registry and executes only work whose source is
  due; it must not run broad maintenance after an unrelated wake.
- Background idle without durable work has no application-controlled deadline
  after grace expiry.

## Delivery batches

1. Typed wake diagnostics and observation sessions.
2. One runtime policy owner; migrate user preferences to Automatic, Always
   reachable and Battery saver while accepting legacy persisted values.
3. One scheduler with explicit wake sources and source-selective executors.
4. Grace-to-dormant background lifecycle; remove recurring rendezvous.
5. Demand/dirty-peer maintenance and one Android visibility owner.
6. Debug-only Battery, Runtime, Logs and Incident console; bounded sanitized
   incident bundles and optional dev-only ingest.
7. Canonical runtime/power documentation, lab peer and deterministic/real
   device validation.

## Acceptance checks

For 15--30 minutes background + screen-off idle with no durable work:

```text
background rendezvous wakes = 0
peer/relay probes           = 0
DB polling reads/writes     = 0
FFI polling                 = 0
contact scans               = 0
peer reconnect attempts     = 0
next app-controlled wake    = none
```

Diagnostics record wake sources and observation deltas so these invariants can
be verified on one device before peer-to-peer soak testing.
