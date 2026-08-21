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

| Batch | Status | Scope |
| --- | --- | --- |
| 1 | Implemented | Typed wake diagnostics and observation sessions. |
| 2 | Implemented core | RuntimeOwner receives atomic host-policy inputs; legacy values normalize safely. |
| 3 | Implemented core | One deadline registry with source-selective maintenance. |
| 4 | Implemented core | One-shot background grace and soft dormancy; no recurring rendezvous. |
| 5 | In progress | Demand/dirty-peer maintenance and unified platform visibility. |
| 6 | In progress | Debug-only Battery/Runtime/Logs/Incident console and bounded support bundle. |
| 7 | In progress | Canonical docs, lab peer and deterministic/device validation. |

The remaining implementation work is:

1. Migrate user preferences to Automatic, Always
   reachable and Battery saver while accepting legacy persisted values.
2. Finish dirty-peer maintenance so delivery routes only active contacts rather
   than using the current transitional contact projection.
3. Finish the Debug-only Battery, Runtime, Logs and Incident console; bounded sanitized
   incident bundles and optional dev-only ingest.
4. Add the lab peer and deterministic/real
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
