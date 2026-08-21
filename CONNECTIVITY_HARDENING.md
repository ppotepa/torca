# Torca connectivity hardening

**Document type:** focused engineering record.  
**Evergreen architecture:** [`ARCHITECTURE.md`](ARCHITECTURE.md).  
**Current status:** [`docs/STATUS.md`](docs/STATUS.md).

This document records the connectivity invariants that motivated the current supervision/runtime-policy work. Exact retry constants and worker names are implementation details; the durable rules below are the part that should survive refactors.

## Target properties

The application core must not perform unbounded blocking network I/O. Local user intent is persisted first where durability is required, while process-owned executors/supervisors own concrete Tor, relay, peer, pairing, delivery and Radio I/O.

```text
Flutter / ABI
     |
ClientApplication (local commands, projections, durable work)
     |
Runtime + runtime policy (routing, demand, evidence, deadlines)
     +-- Tor / onion execution
     +-- Relay / pairing execution
     +-- Peer / delivery execution
     `-- Radio execution
```

Network failures degrade the affected transport. They must not prevent access to local encrypted history/settings or convert cosmetic health state into a correctness dependency.

## Connection invariants

- Only one relevant connect/probe attempt should run for a transport/work lane at a time.
- Backoff is bounded and reset by meaningful recovery evidence, not arbitrary UI polling.
- Default-network/route changes invalidate stale transport state and wake the appropriate recovery path.
- Real TX/RX/ACK/handshake evidence should suppress unnecessary health probing.
- Durable work such as queued messages, attachments, pairing and Radio uses feature-owned demand independent of the currently visible Flutter route.
- Healthy/idle contacts must not create unconditional reconnect loops merely because they exist in storage.
- Expensive Tor bootstrap/publication work must not occupy the application/native actor that serves local commands and projections.

## Relay/pairing invariants

The relay is an ephemeral, untrusted pairing rendezvous service.

- Stable operation IDs are used where relay mutations need idempotent replay after an unknown outcome.
- Polling is non-destructive until explicit acknowledgement where the protocol requires it.
- Pairing work has bounded/adaptive scheduling rather than a fixed hot loop.
- Relay health observation must not veto an authoritative user pairing attempt or make local UI unavailable.
- Relay degradation must not redefine normal established contact traffic as relay-dependent.
- Onion address allocation, publication and externally proven reachability are distinct states.

## Peer/delivery invariants

- Local message/control intent becomes durable before network delivery owns it.
- Peer keepalive/probe decisions use recent authenticated transport evidence.
- ACK/probe waits are bounded and run outside the main application/native actor.
- Retry and inbound deduplication make late/duplicate network outcomes safe.
- Route/network generation changes invalidate stale connections without spawning reconnect storms.

## Radio invariants

- Radio consent/session/floor state is application/domain-owned; audio/network adapters execute concrete work.
- Media/control lanes are bounded and cannot create a second independent general-purpose scheduler.
- Route failure produces one controlled interrupted/recovery path rather than parallel sessions.
- Microphone capture stops on release/session close/permission failure/background conditions defined by the platform/application contract.
- Radio traffic contributes health/work evidence without putting audio payloads into diagnostics.

## Runtime scheduling and battery

The current direction is event/deadline driven rather than fixed idle polling:

```text
Attention -> Demand Lease -> Health Evidence -> Deadline Scheduler
```

UI attention is advisory. It can justify freshness work but cannot control correctness of durable queues, pairing completion, attachments, inbound handling or Radio session rules.

See [`docs/architecture/runtime-control.md`](docs/architecture/runtime-control.md)
for the canonical runtime-policy/energy contract and
[`docs/validation/runtime-power.md`](docs/validation/runtime-power.md) for
measurement-gated dormancy validation.

## Observability

Connectivity diagnostics should contain typed states, bounded counters/timing, attempt/generation context and redacted errors. They must not contain message/attachment plaintext, Radio audio, private identity keys, relationship secrets or pairing capabilities.

Use the current Rust collector rather than legacy PowerShell collection instructions:

```powershell
cargo run -p torca-deploy -- logs --target all
```

See [`docs/diagnostics.md`](docs/diagnostics.md) for the current incident layout.

## Validation expectations

Connectivity confidence requires more than source checks. Useful gates include:

- deterministic lost-response/replay and retry tests;
- route-change/recovery tests with controlled adapters;
- source/architecture policy and workspace tests;
- supported-platform builds; and
- real Windows ↔ Android scenarios covering Tor warm-up, pairing, peer messaging, attachments, relay/network interruption and Radio recovery.

Do not describe a target property in this file as device-validated unless the corresponding real-device scenario was actually run. Record detailed dated evidence in the manual acceptance record rather than adding permanent test counts/constants here.
