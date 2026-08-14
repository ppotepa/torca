# Attention-Driven Runtime v2

## Status

Architecture target and migration plan for reducing application-controlled CPU
wakeups, network probes, FFI polling, reconnect storms, and unnecessary Tor
activity. This document distinguishes the checked-in implementation from the
target design; target-state sections are not claims that the behavior already
exists.

### Audited implementation status

Last reconciled with the source tree on 2026-08-14.

| Capability | Status | Current implementation |
| --- | --- | --- |
| Policy kernel and fake-clock tests | Implemented | `crates/application/torca-runtime-policy` |
| Native revision event hub | Implemented | `torca-runtime-policy::RuntimeEventHub`, exported through `torca-native`; native actor now blocks until a command or the earliest durable pending-operation deadline |
| Flutter revision waiter | Implemented | event isolate waits indefinitely for native revisions through `torca_runtime_wait_for_revision` with `timeoutMs=0`; disposal calls the explicit cancellation ABI |
| Android notification waiter | Implemented | foreground service waits indefinitely for native runtime changes on a dedicated `HandlerThread`; `onDestroy()` cancels the native waiter before quitting |
| Pairing deadline worker | Implemented | pairing worker calculates its own deadlines |
| Demand-created peer reconnect entries | Implemented | stored contacts alone do not create an outgoing reconnect loop |
| Health freshness suppressing some peer probes | Implemented core | `PeerProbeSupervisor` consumes freshness; peer TX/RX/ACK/handshake/failure counters now feed policy evidence, while attachment-specific chunk attribution remains |
| Attention/viewport contract from Flutter | Partial | `runtime.attention.set` crosses the generated contract; top-level navigation reports up to seven visible contacts, focused conversation routes report on entry and on mobile route return, while focused-contact generation race tests remain |
| Feature-owned leases | Partial | queued/retried attachments, message delivery and pairing operations create runtime leases independent of UI attention; relay health is now gated by the active relay lease and radio enable/transmit commands create separate radio leases; full radio executor cleanup and route-generation tests remain |
| Central deadline scheduler | Partial | fixed 100-ms tick, native 1-s pending reconcile fallback and idle fallback were removed when no executor owns work; delivery/control outboxes, radio control/state-sync retry, Tor lifecycle, peer/radio listener events, onion-publisher callbacks, durable pending deadlines and lease expiry expose wake sources |
| Demand/evidence-driven relay health | Implemented core | production runtime uses a demand-driven supervisor: no initial/healthy probe exists without a relay lease; pairing/network events wake one lane and retry backoff is bounded |
| Fully evidence-driven peer health | Partial | fresh Live/Recent evidence removes peer deadlines entirely; handshake transitions, successful probe ACKs and ready-to-disconnected transitions now feed policy evidence; stale/failed peers use 30-second healthy and 10-second retry intervals through one single-flight lane |
| Blocking/deadline radio worker | Partial | idle command waiting and radio-media listener acceptance are blocking; radio control/state-sync queues now expose retry deadlines to the central scheduler; active media uses a dynamically bounded I/O timeout with audio/ACK/burst/keepalive deadlines covered by deterministic tests |
| Arti soft dormancy | Partial | `torca-tor` now exposes `TorActivityMode::{Active, SoftDormant}` over the stable service handle; runtime policy integration remains deliberately measurement-gated |
| Runtime Energy Ledger | Partial | diagnostics count scheduler/native/FFI/radio wakeups, snapshots/DB reads, command and worker writes, inbound text, attachment metadata/blob writes, peer and relay probes; transport evidence is policy-visible, while receipt/reaction projection writes and physical TX/RX totals remain |

The current implementation is therefore a transitional state, but its idle
path is already event/deadline driven. Event waiting, pairing scheduling,
demand-created peer reconnects, adaptive runtime waiting, idle radio blocking,
bounded kernel-blocked peer/radio listeners, onion publisher completion
wakeups with runtime propagation and demand-woken relay health have landed.
The revision waiters now block on the native condition variable with
`timeoutMs=0`. Cancellation is one-shot, so a stopped Flutter isolate or
Android service can be started again on the same runtime without inheriting a
permanent cancelled waiter. A finite timeout remains available to diagnostic
callers, but it is not used by the idle platform workers.
The remaining work is integration hardening: executor-owned relay/radio
leases, complete attention-generation tests, richer TX/RX/chunk evidence and
the remaining projection counters. Arti dormancy remains deliberately
measurement-gated and is not required for the no-idle-polling guarantee.

The central model is:

```text
Attention -> Demand Lease -> Health Evidence -> Deadline Scheduler
```

## Architecture review and optimization verdict

Reviewed against the checked-in source tree on 2026-08-14. The core direction
is appropriate for Torca: event-driven waiting, demand leases, evidence-based
health and a single deadline owner reduce the two largest current costs (idle
polling and reconnect storms) without changing the wire protocol.

The plan is intentionally narrowed into three confidence tiers:

1. **Reliability-critical and low-risk:** finish feature-owned leases,
   evidence recording, executor deadlines, event-wait cancellation and the
   Energy Ledger. These changes preserve the current always-available Tor
   behavior and must land first.
2. **Measurement-gated optimization:** apply Arti soft dormancy only after
   Android onion resume, pairing, delivery, attachments and radio pass soak
   tests. Dormancy is not a prerequisite for event-driven runtime savings.
3. **Optional experiments:** deterministic dial ownership, viewport-driven
   contact probing and rendezvous windows. They are not part of the minimum
   battery architecture and must not add latency or reduce delivery
   reliability. Implement them only when traces demonstrate a concrete
   problem that the first tier does not solve.

This avoids over-engineering the current system. In particular, a stored
contact is not a reason to keep a stream open, but the application must remain
available to inbound traffic in the default `AlwaysAvailable` policy. The
first release target is therefore **no periodic application-controlled idle
work** (waiter safety wakeups are allowed, but must not query or mutate state),
not a dormant Tor process at all costs.

### Revised decision rules

- Preserve one policy kernel and one scheduler; do not add per-feature timer
  frameworks.
- Treat real TX/RX/ACK/handshake/chunk activity as health evidence before
  scheduling any probe.
- Keep reliable work (pending messages, attachments, pairing and radio) on
  explicit leases independent of the current Flutter route.
- Use cached/stale state for cosmetic lists; refresh only on attention or an
  explicit user diagnostic action.
- Keep relay health demand-driven, but never let a missing cosmetic probe block
  local UI, inbound handling or durable delivery.
- Do not introduce a relay mailbox, push dependency or protocol change as part
  of the battery migration.

### Optimization verdict

This is the best low-risk architecture for the current Torca design. The
largest avoidable costs were application polling, duplicate reconnect loops
and health probes that ignored real traffic; those paths are now replaced by
one policy crate, executor deadlines and event wakeups. A second general
purpose scheduler or a single crate owning Tor, relay, peers, radio and UI
would make ownership less clear and would increase failure coupling.

The next optimization should therefore be incremental and measurable:

1. finish executor-owned leases and evidence counters;
2. add idle/active traces for Android and Windows;
3. only then evaluate Arti soft dormancy and rendezvous windows;
4. keep deterministic dial ownership and viewport probing as opt-in
   experiments until traces prove they reduce work without hurting delivery.

The release default remains `AlwaysAvailable`: local UI, inbound listener,
durable delivery and pairing must not depend on cosmetic health refreshes or
on an experimental dormant/resume cycle.

### What is deliberately deferred

The following remain documented options, not required architecture:

- `TorClient::set_dormant()` and background rendezvous windows;
- deterministic dial ownership and takeover rules;
- viewport-level contact IDs and affinity scoring;
- Android `ConnectivityDiagnosticsManager` integration;
- abstract energy costs that pretend to be physical battery measurements.

Each deferred item needs a trace, a measurable acceptance test and a rollback
path before implementation. This keeps the runtime simpler and prevents a
power optimization from reintroducing the long Android warm-up and onion
resume failures already seen in production testing.

## Architectural decisions

- `AttentionContext` is advisory only. It must never control delivery correctness, durable queues, pairing completion, attachments, radio correctness, or inbound traffic.
- Shared policy lives in `crates/application/torca-runtime-policy`.
- The policy crate owns decisions, leases, evidence, budgets, deadlines, event waiting, and counters. It does not own sockets, SQLite, Flutter, JNI, Android APIs, or Arti execution.
- The target Tor policy uses adaptive Arti soft dormancy instead of destroying
  the client or clearing its cache/state. The low-level API exists; automatic
  runtime policy integration is intentionally not enabled yet.
- Relay, peer connections, pairing, attachments, and radio remain separate executors controlled by policy permits.
- Android uses `NetworkCallback` and a fingerprint of route capabilities
  (`validated`, `internet`, `metered`, Wi-Fi/cellular); changes to that
  fingerprint wake recovery even when the `Network` object is unchanged.
  Power Saver propagation and `ConnectivityDiagnosticsManager` remain deferred
  because they are not required for the no-idle-polling baseline. Do not claim
  either signal in diagnostics until a platform callback and a policy mapping
  are implemented and tested.
- There is no legacy runtime mode and no parallel old scheduler. Each old polling/tick path is removed when its replacement lands.
- Relay continues to store only invitations. It does not become a message mailbox.
- Background delivery without push is best-effort and may wait for a rendezvous window or for the recipient to open Torca.

## Shared crate: `torca-runtime-policy`

The checked-in crate is a dependency-light synchronous policy kernel. It
already contains the core attention, demand, lease, evidence, policy and event
hub types. Budget enforcement, scheduler ownership, executor integration and
production counters remain incomplete.

The target crate responsibilities are:

- `attention` — foreground/background state, visible surface, focused resource, and contact viewport.
- `demand` — requirements derived from UI intent, durable queues, and active operations.
- `lease` — temporary permission to keep Tor, relay, or peer resources active.
- `evidence` — TX, RX, ACK, handshake, probe, failure, and network-generation evidence.
- `budget` — token buckets and single-flight limits.
- `scheduler` — deadline queue and wake-window coalescing.
- `policy` — pure event-to-decision reducer.
- `event_hub` — revision/cursor waiting built on `Mutex` and `Condvar`.
- `stats` — counters and the regression-oriented Energy Ledger.

Core API shape:

```rust
pub struct RuntimeGovernor;
pub struct RuntimePolicyConfig;
pub struct AttentionContext;
pub struct NetworkEnvironment;
pub struct HealthEvidence;
pub struct ConnectionLease;
pub struct WorkDemand;
pub struct WorkPermit;
pub struct RuntimeEventHub;

pub enum AttentionSurface {
    Background,
    Home,
    Chats,
    Contacts,
    Conversation(OpaqueId),
    ConnectionDetails(OpaqueId),
    Pairing(OpaqueId),
    Radio(OpaqueId),
    Diagnostics,
}

pub enum DemandReason {
    VisibleConversation,
    VisibleContact,
    PendingMessage,
    AttachmentTransfer,
    ActivePairing,
    RadioSession,
    IncomingSession,
    RecentConversation,
    ExplicitDiagnostic,
    BackgroundRendezvous,
}

pub enum EvidenceKind {
    Tx,
    Rx,
    Ack,
    Handshake,
    Probe,
}

pub enum Freshness {
    Live,
    Recent,
    Stale,
    Unknown,
}

pub enum WorkClass {
    TorWake,
    OnionPublish,
    RelayConnect,
    RelayProbe,
    PeerDial,
    PeerProbe,
    PairingPoll,
    Delivery,
    Attachment,
    Radio,
    Projection,
    Cleanup,
}
```

Primary operations:

```rust
governor.apply(event, now) -> PolicyDelta
governor.acquire_lease(scope, reason, owner, expiry)
governor.release_lease(owner)
governor.record_evidence(scope, evidence)
governor.next_deadline() -> Option<Instant>
governor.take_due(now) -> Vec<WorkPermit>
governor.snapshot() -> RuntimePolicySnapshot
event_hub.wait(waiter, after_revision, after_cursor)
event_hub.cancel(waiter)
```

The crate uses `OpaqueId` and generic resource scopes. Feature crates map their typed IDs at the boundary, preventing policy dependencies on every domain crate.

## Attention and smart contact refresh (target)

Flutter will send one ephemeral context through the generated contract:

```text
surface
focusedResourceId?
visibleContactIds[]
generation
```

- One navigation observer owns the context; individual widgets do not emit competing state independently.
- `generation` prevents an old route or modal from clearing newer attention.
- Pairing, radio, attachments, pending messages, and incoming sessions acquire Rust-owned leases independently of Flutter.
- The UI receives cached state immediately and revalidates in the background.

Contact priority order:

1. Focused contact.
2. Contacts with active delivery, attachment, pairing, or radio demand.
3. Visible frequent/recent contacts.
4. Other visible contacts.
5. A bounded frequent-contact pool using otherwise unused budget.

Affinity is deterministic and local: recent interaction time plus interaction count from the last 30 days. It is not ML and is never transmitted.

Initial probe budget:

- Contacts screen: one probe every 3 seconds, burst capacity 2.
- Active conversation: probe only after 120 seconds without real evidence.
- Connection Details: one immediate probe if the existing sample is older than 10 seconds.
- Home, Chats, and background idle: no independent cosmetic probes.
- Frequent contacts may be checked in the background only inside an already-open rendezvous window, at most three per window.
- At most one expensive peer probe may be in flight globally.

## Health evidence and freshness

Every real operation is health evidence:

- frame transmitted or received;
- protocol ACK;
- completed handshake;
- successful message or attachment chunk;
- radio control/media frame;
- successful relay operation;
- explicit probe only as a last resort.

Initial freshness mapping:

```text
LIVE     real TX/RX/ACK/handshake in the last 15 seconds
RECENT   reliable evidence not older than 2 minutes
STALE    a previous reliable result exists but is old
UNKNOWN  no evidence exists for the current network generation
```

Presence and transport health remain separate concepts. A technical probe never updates human `last seen`. A network change makes previous evidence stale instead of declaring every peer offline.

## Connection leases and deterministic dialing (target)

The checked-in peer link no longer reconnects every stored contact merely
because it exists. The remaining target is stricter: only resources with an
active lease may create or retain outgoing work.

Default leases:

- Visible conversation: until closed plus 30-second grace.
- Pending message: until ACK or durable retry scheduling.
- Attachment transfer: until complete, cancelled, or durably paused.
- Radio: exact armed/session lifetime.
- Incoming session: until its idle timeout.
- Pairing: until terminal state or five-minute expiry.
- Diagnostics: explicit, at most 60 seconds.

After the final lease expires:

- outgoing peer stream closes after 30 seconds;
- an existing inbound stream can remain for up to 120 seconds without traffic;
- relay stream closes after its own grace period;
- no contact exists in a default always-connected set.

Deterministic dial ownership is an optional experiment, not part of the
reliability baseline:

- identity ordering selects the preferred initiator for speculative connect, rendezvous, and recovery;
- concrete outbound work may dial immediately regardless of preferred ownership;
- simultaneous incoming/outgoing connections deterministically retain the same single stream on both sides;
- a non-preferred side with real demand takes over recovery after 20 seconds.

## Relay and network changes (target)

Relay will receive a lease only for active pairing, pending relay work, or explicit diagnostics.

- Successful relay traffic replaces health probes.
- Real traffic moves the next possible probe deadline.
- Without relay demand there is no 15-second heartbeat.
- Relay state is stale-while-revalidate and does not flicker through `Checking` for every validation.

Network changes increment `network_generation` and perform staged recovery:

1. Invalidate old streams and mark evidence stale.
2. Wake Tor only when demand exists.
3. Recover relay only when a relay lease exists.
4. Recover focused, pending, attachment, and radio peers first.
5. Do not reconnect unrelated contacts.
6. Allow cosmetic work only from the remaining budget.

## Adaptive Tor and rendezvous windows (planned, measurement-gated)

The exposed `torca-tor` API is:

```rust
set_activity_mode(TorActivityMode::Active | SoftDormant)
```

It delegates to Arti `TorClient::set_dormant()` while retaining directory
cache, persistent state, and onion identity. The runtime does not call it
automatically yet; the policy integration remains gated on Android onion,
pairing, delivery, attachment and radio soak tests.

Dormancy must land behind an explicit runtime policy (`AlwaysAvailable`,
`Balanced`, `BatterySaver`) and soak tests. `AlwaysAvailable` remains the
reliability-first default until Android onion reachability, pairing, pending
delivery, attachments and radio are proven under dormant/resume cycles.

Tor stays active while:

- the app is foregrounded;
- a user-visible or delivery-critical lease exists;
- pairing, attachment, or radio is active;
- an inbound session exists;
- a rendezvous window is open.

When backgrounded and without demand:

```text
90-second grace
-> close relay and outgoing peer streams
-> suspend active onion publication
-> enter Arti SoftDormant
```

Initial rendezvous policy:

- normal battery/network: one shared window every 30 minutes;
- Power Saver or metered network: one window every 60 minutes;
- maximum window duration: 120 seconds;
- deterministic jitter: at most 15 seconds while preserving overlap;
- remain active until onion is ready plus 45 seconds, or until the window ends;
- pending outbound retry: 1, 2, 5, then 15 minutes, and always join the shared window schedule;
- frequent-contact probes can piggyback on the window but never create it.

Without push or a relay mailbox, background delivery is durable but best-effort. It may wait until a shared window, application foregrounding, or another real demand event.

## Deadline scheduler and wake windows (implemented core)

The original fixed `recv_timeout(100 ms)` loop and idle fallback timer have
been removed. The runtime now takes the earliest deadline exposed by Tor,
pairing, communication/delivery and lease executors:

```text
deadline exists -> recv_timeout(deadline - now)
no deadline     -> recv()
```

Every participating subsystem exposes its next deadline. Listener, Tor and
publisher events wake the runtime immediately; low-priority work is coalesced:

- foreground slack: up to 3 seconds;
- background slack: up to 15 seconds;
- user actions, protocol ACKs, pairing expiry, and audio deadlines: no slack.

## Radio and adaptive keepalive (implemented core; hardening ongoing)

Radio retains a dedicated media worker because real-time audio has 20-ms
deadlines. The unconditional 10-ms tick/sleep path has been removed. The
worker now blocks when idle and computes the next active deadline:

- No armed session: block indefinitely.
- Pending connection: block until command, accept event, or reconnect deadline.
- Active session: block until audio/playback/keepalive deadline.
- `PeerListener` uses one cancellable blocking accept worker, a bounded queue
  and runtime wake callback; radio-media now uses the same wake path.
- Shutdown cancels both listener and media waiter.

Keepalive policy:

- TX/RX resets the keepalive deadline.
- First keepalive occurs only after 30 seconds of silence.
- Successful idle periods expand the interval from 30 to 60 to 120 seconds.
- A half-open failure lowers the interval for the current network generation.
- No active session means no keepalive work.

## Event-driven Flutter and Android (implemented core, hardening ongoing)

`RuntimeEventHub` owns monotonic:

```text
runtime_generation
revision
notification_cursor
change_mask
```

Flutter:

- The command isolate remains responsive and never blocks in event waiting.
- A separate event isolate waits in `waitForRuntimeChange`.
- A change triggers exactly one snapshot fetch.
- Dispose calls `torca_runtime_cancel_revision_wait` before terminating the
  isolate.
- The 250-ms/1-second polling timers and idle JSON comparison disappear.

Android:

- The notification `HandlerThread` blocks on the same event hub.
- New revisions wake it to fetch notifications once.
- `onDestroy()` cancels the waiter before quitting the thread.
- The 250-ms and 1500-ms pollers disappear.
- The manifest explicitly includes `ACCESS_NETWORK_STATE`.
- `NetworkCallback` reports availability, validation, meteredness, blocked state, and generation.
- Power Saver and `ConnectivityDiagnosticsManager` are intentionally not wired
  yet; their future integration must be event-driven and must not add a polling
  loop to the foreground service.

## Observability and Energy Ledger (implemented core; counters still expanding)

Counters are incremented only on existing execution paths. Scheduler wakeups,
native event/FFI wakeups, radio listener wakeups, snapshot builds, DB reads,
command-induced DB writes, text/control worker job writes, attachment metadata
writes, inbound text inserts, attachment metadata writes, attachment blob
writes, peer probes and relay probes are wired today. TX/RX frame counts,
active-peer duration, wake-lock duration and receipt/reaction projection writes
are not emitted yet and must not be populated with estimates:

```text
runtime_wakeups_by_reason
tor_wakes
tor_dials
onion_publications
relay_requests
relay_probes
peer_dials
peer_probes
radio_wakeups
snapshot_builds
db_reads
db_writes
blob_writes
ffi_wakes
```

Energy Ledger assigns stable abstract costs for regression testing. It is not presented as mWh. Android UID TX/RX totals are sampled only on lifecycle changes, Diagnostics entry, and scenario completion.

## Implementation sequence and current progress

1. **Partial:** keep `torca-runtime-policy`, fake clock, reducer, lease book,
   evidence ledger and event hub small and dependency-light. Finish budgets
   and production scheduler ownership before adding new policy concepts.
2. **Partial:** scheduler, native event/FFI-wake, radio listener wake,
   snapshot/DB-read, command-induced DB-write, text/control worker-write,
   attachment-metadata-write, blob-write, inbound-text-write, peer-probe and
   relay-probe counters are wired, with a dedicated CI idle scheduler gate.
   Add receipt/reaction projection counters before attempting Arti dormancy.
3. **Implemented core:** Flutter and Android use cancellable native event
   waiters, including bootstrap/pending states. The native actor also blocks
   until a command or the earliest durable pending-operation deadline. The
   platform ABI uses the unbounded condvar path for idle workers and exposes
   explicit cancellation for shutdown. Keep shutdown, missed-revision,
   one-shot-cancellation and waiter-generation regression tests current.
4. **Partial:** delivery/control outboxes, Tor lifecycle and lease expiry now
   expose deadlines; the runtime blocks when no deadline exists and wakes on
   peer/radio listener and onion-publisher events. Finish deterministic tests
   for Tor worker completion and delete any remaining fallback only after the
   corresponding event source is covered. Do not create independent timers.
5. **Partial / next:** attachment, pending-message and active pairing leases
   now survive route changes; pairing leases expire with the five-minute
   invitation window and release on reject/cancel. Radio enable and
   transmission commands now own separate `ResourceScope::Radio` leases, and
   peer health work accepts an active radio lease as demand. Successful
   approval, rejection and cancellation release the pairing lease. Finish
   radio executor consumption and stale-lease cleanup; attention leases
   remain advisory and must never be the only owner of durable work.
6. **Partial:** evidence suppresses cosmetic peer probes and removes their
   deadlines while Live/Recent; handshake transitions, successful probe ACKs
   and ready-to-disconnected transitions now update the policy ledger.
   Integrate more TX/RX/chunk evidence and add a trace proving no wakeup
   occurs while all focused peers remain fresh.
7. **Implemented core:** relay healthy heartbeats and startup probes without
   demand are removed. The production supervisor sleeps until a relay lease,
   pairing operation or demanded network change; revalidation remains
   operation-scoped and never blocks local readiness.
8. **Partial:** idle radio waits on its command channel and peer/radio
   `PeerListener` accepts on dedicated blocking workers with bounded queues,
   cancellable shutdown and wake propagation. Radio control/state-sync retry
   deadlines now flow through `RadioInboundRuntime` into
   `TorcaCommunicationDriver::next_maintenance_delay`, so a failed
   `SessionOpen` is not stranded after one attempt. Active media selects the
   next audio, ACK, burst or keepalive deadline with deterministic tests; keep
   the default idle path at zero worker iterations.
9. **Planned / measurement-gated:** evaluate Arti soft dormancy only after
   reliability soak tests pass. It is optional and not a release blocker.
10. **Optional experiments:** deterministic dial ownership, viewport contact
    probing and rendezvous windows, each behind its own trace-based gate.
11. **Final cleanup:** delete every replaced polling constant, reconnect-all
    path and obsolete scheduler only after the corresponding executor exposes
    a tested deadline and cancellation path.

## Target acceptance criteria

These are release gates for the completed migration, not claims about the
current build. “Zero wakeups” means zero application-controlled Torca wakeups;
Arti, the operating system and the kernel may still perform their own required
maintenance.

- Background with no work between rendezvous windows: zero Torca-controlled wakeups, probes, snapshots, DB reads, and FFI calls for 10 minutes.
- Active conversation with real traffic: zero peer health probes.
- With 250 contacts and seven visible, only visible contacts and the bounded affinity pool can consume the probe budget.
- A frequent background contact can be checked only during a scheduled rendezvous window.
- Pending messages survive restart and deliver when both peers next overlap online.
- A network change reconnects only leased peers and never creates an all-contact storm.
- Simultaneous dial retains exactly one consistently selected stream.
- Relay without pairing or relay work performs zero heartbeats.
- Idle radio performs zero worker iterations.
- Active radio preserves 20-ms media deadlines and supports deterministic cancellation.
- Event waiters cannot miss revisions and can always be cancelled during shutdown.
- Power Saver and metered modes perform no cosmetic wakeups outside rendezvous windows.
- CI fails when an idle simulation produces a non-zero application-controlled Energy Ledger score.

## External references

- Arti dormant mode: <https://docs.rs/arti-client/latest/arti_client/enum.DormantMode.html>
- Android network callbacks: <https://developer.android.com/develop/connectivity/network-ops/reading-network-state>
- Android network batching: <https://developer.android.com/develop/connectivity/network-ops/network-access-optimization>
- Android PowerManager: <https://developer.android.com/reference/android/os/PowerManager>
- Android TrafficStats: <https://developer.android.com/reference/android/net/TrafficStats>
- ConnectivityDiagnosticsManager restrictions: <https://developer.android.com/reference/android/net/ConnectivityDiagnosticsManager>
