# Torca connectivity hardening

## Target architecture

The application core never performs blocking network I/O. It persists local intent and communicates
with four process-owned supervisors through bounded command/event queues:

```text
Flutter / ABI
     |
ClientApplication (local commands, projections, pending-operation ledger)
     |
Runtime coordinator (non-blocking routing only)
     +-- TorSupervisor
     +-- RelayPairingSupervisor
     +-- PeerSessionSupervisor
     `-- DeliverySupervisor
```

Each supervisor owns exactly one state machine and publishes payload-free observations to the shared
connectivity ledger. Network failures degrade only their transport; they cannot stop snapshots,
history, settings or locally durable commands.

## Connection state machine

```text
Disconnected -> Connecting -> Healthy -> Suspect -> Backoff
      ^              |            ^          |          |
      `--------------+------------+----------+----------'
            network generation / timer / explicit wake
```

- Only one connect attempt may run for one transport generation.
- Backoff uses exponential full jitter and is reset only after a stable healthy window.
- A default-network change increments a generation, closes stale streams and wakes supervisors.
- One failure produces `Suspect`; repeated failures inside a rolling window produce `Degraded`.
- UI commands enqueue durable intent and never wait for a network round trip.

## Relay delivery invariants

- `Open`, `Join` and `Push` have stable operation IDs and are idempotent.
- Every queued relay message has a stable message ID and monotonically increasing side sequence.
- `Poll(after_sequence)` is non-destructive.
- Only `Ack(up_to_sequence)` removes delivered messages.
- Retrying after an unknown outcome returns the original response or deduplicates the mutation.
- Slots remain ephemeral and expire on the relay clock; no contact graph or message history is stored.

## Scheduling

- Relay health probes are independent of pairing work and never block the runtime actor.
- Pairing uses immediate wake after a local transition, then adaptive polling with jitter; never a
  fixed 100 ms loop.
- Completed sessions have a bounded final-ACK grace period and cannot poll forever.
- Peer keepalive is suppressed by recent application traffic and runs less frequently in background.

## Observability

All layers emit correlation ID, operation kind, phase, attempt, network generation, latency and a
typed redacted error. Relay metrics include active connections/slots, request outcomes, deduplicated
operations and queue depth. Diagnostic collection must capture both the durable logs and the live
runtime connectivity ring without invitation codes, tokens, onion identities or payloads.

## Release gates

- Both clients must use the same source fingerprint, wire version and relay endpoint.
- Chaos tests cut connections before write, after write, after relay commit and during response.
- Tests prove no lost delivery, bounded duplicates, actor responsiveness and eventual reconnect.

## Implemented in 0.3

- Relay wire V4 provides idempotent mutations, stable message IDs, side-local sequences and
  non-destructive `Poll` followed by explicit `Ack`.
- Pairing relay I/O is owned by a bounded `torca-pairing-supervisor`; its adaptive schedule uses
  immediate wake, a one/two-second healthy cadence and exponential backoff capped at 30 seconds.
- Android default-network changes are debounced, invalidate the pairing socket and wake active
  sessions. Ordinary transport errors use the same reconnect path on Windows.
- Relay health uses one long-lived, single-flight worker. A failed bounded
  probe is surfaced immediately with a stable error code and recovery uses
  5/15/30/60-second backoff.
- Client onion publication is independent from Tor bootstrap and runtime-actor
  maintenance; a bootstrapped client is attached while its endpoint keeps
  publishing in the background.
- Onion publication has independent publishing and degraded deadlines. A timed-out
  attempt drops the real Arti service handle before relaunching it with the preserved
  HSS identity; repeated failure escalates to one background Tor bootstrap recovery.
- Relay and peer adapters keep a swappable `TorServiceHandle`. During recovery it is
  temporarily unavailable and then points at the replacement Arti client, preventing
  long-lived adapters from reconnecting through a stale runtime.
- Peer keepalive/ACK waits run through one durable worker outside the runtime
  actor and feed their result back without blocking history, snapshots or
  local commands.
- Relay emits periodic redacted counters for connections, slots, requests, failures and expiry.
- `scripts/torca.ps1 collect` forwards named parameters correctly, so incident bundles can be
  captured through the main CLI.

The process-owned `OwnedTorDriver` remains the Tor lifecycle authority. Its
normal maintenance is non-blocking; expensive bootstrap is limited to
startup/recovery and public onion publication is not coupled to relay or peer
polling.
