# Attention-Driven Runtime v2

## Status

Accepted architecture plan for reducing application-controlled CPU wakeups, network probes, FFI polling, reconnect storms, and unnecessary Tor activity.

Implementation status: the dependency-light `torca-runtime-policy` kernel is now
present in `crates/application/torca-runtime-policy` and is used by peer probe
scheduling. Live/recent TX, RX, ACK and handshake evidence suppresses cosmetic
peer probes; stale or unknown evidence remains eligible. The event hub and
policy tests are in place. Flutter and the Android foreground service now wait
on the native revision hub instead of issuing idle snapshot/notification polls.
Pairing now uses a deadline-driven worker, and peer reconnects are demand
driven: an address-book entry no longer creates an outgoing reconnect loop;
send/probe paths create the reconnect entry when work actually requires it.
Foreground pairing commands explicitly wake the relay probe lane, so an idle
runtime does not need to poll just to discover that relay demand has appeared.
Lease ownership for every feature executor (relay, delivery, attachments and
radio) and Arti dormancy remain follow-up work; the policy API is ready for
those integrations.

The central model is:

```text
Attention -> Demand Lease -> Health Evidence -> Deadline Scheduler
```

## Architectural decisions

- `AttentionContext` is advisory only. It must never control delivery correctness, durable queues, pairing completion, attachments, radio correctness, or inbound traffic.
- Shared policy lives in `crates/application/torca-runtime-policy`.
- The policy crate owns decisions, leases, evidence, budgets, deadlines, event waiting, and counters. It does not own sockets, SQLite, Flutter, JNI, Android APIs, or Arti execution.
- Tor uses adaptive Arti soft dormancy instead of destroying the client or clearing its cache/state.
- Relay, peer connections, pairing, attachments, and radio remain separate executors controlled by policy permits.
- Android uses `NetworkCallback`, network capabilities, and Power Saver events. `ConnectivityDiagnosticsManager` is not a baseline dependency because ordinary messaging applications do not receive its callbacks.
- There is no legacy runtime mode and no parallel old scheduler. Each old polling/tick path is removed when its replacement lands.
- Relay continues to store only invitations. It does not become a message mailbox.
- Background delivery without push is best-effort and may wait for a rendezvous window or for the recipient to open Torca.

## Shared crate: `torca-runtime-policy`

The crate is a dependency-light synchronous policy kernel containing:

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

## Attention and smart contact refresh

Flutter sends one ephemeral context through the generated contract:

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

## Connection leases and deterministic dialing

The runtime no longer reconnects every stored contact. Only resources with an active lease may create or retain outgoing work.

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

Deterministic dial ownership is part of the baseline:

- identity ordering selects the preferred initiator for speculative connect, rendezvous, and recovery;
- concrete outbound work may dial immediately regardless of preferred ownership;
- simultaneous incoming/outgoing connections deterministically retain the same single stream on both sides;
- a non-preferred side with real demand takes over recovery after 20 seconds.

## Relay and network changes

Relay receives a lease only for active pairing, pending relay work, or explicit diagnostics.

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

## Adaptive Tor and rendezvous windows

`torca-tor` exposes:

```rust
set_activity_mode(TorActivityMode::Active | SoftDormant)
```

The implementation uses Arti `TorClient::set_dormant()` while retaining directory cache, persistent state, and onion identity.

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

## Deadline scheduler and wake windows

The fixed `recv_timeout(100 ms)` loop is replaced with:

```text
deadline exists -> recv_timeout(deadline - now)
no deadline     -> recv()
```

Every participating subsystem exposes its next deadline. Low-priority work is coalesced:

- foreground slack: up to 3 seconds;
- background slack: up to 15 seconds;
- user actions, protocol ACKs, pairing expiry, and audio deadlines: no slack.

## Radio and adaptive keepalive

Radio retains a dedicated media worker because real-time audio has 20-ms deadlines. The current 5-ms unconditional loop is removed.

- No armed session: block indefinitely.
- Pending connection: block until command, accept event, or reconnect deadline.
- Active session: block until audio/playback/keepalive deadline.
- The listener exposes cancellable blocking accept and wakes the media worker through a channel.
- Shutdown cancels both listener and media waiter.

Keepalive policy:

- TX/RX resets the keepalive deadline.
- First keepalive occurs only after 30 seconds of silence.
- Successful idle periods expand the interval from 30 to 60 to 120 seconds.
- A half-open failure lowers the interval for the current network generation.
- No active session means no keepalive work.

## Event-driven Flutter and Android

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
- Dispose calls `cancelWaiter` before terminating the isolate.
- The 250-ms/1-second polling timers and idle JSON comparison disappear.

Android:

- The notification `HandlerThread` blocks on the same event hub.
- New revisions wake it to fetch notifications once.
- `onDestroy()` cancels the waiter before quitting the thread.
- The 250-ms and 1500-ms pollers disappear.
- The manifest explicitly includes `ACCESS_NETWORK_STATE`.
- `NetworkCallback` reports availability, validation, meteredness, blocked state, and generation.
- `ACTION_POWER_SAVE_MODE_CHANGED` updates policy without polling.

## Observability and Energy Ledger

Counters are incremented only on existing execution paths:

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
ffi_wakes
tx_frames
rx_frames
active_peer_ms
wake_lock_ms
```

Energy Ledger assigns stable abstract costs for regression testing. It is not presented as mWh. Android UID TX/RX totals are sampled only on lifecycle changes, Diagnostics entry, and scenario completion.

## Implementation sequence

1. Add `torca-runtime-policy`, fake clock, policy reducer, lease book, evidence ledger, budgets, scheduler, event hub, and tests.
2. Add counters to existing execution paths without introducing timers.
3. Replace Flutter and Android polling with cancellable event waiters.
4. Replace the central Rust tick with deadline scheduling.
5. Add AttentionContext, viewport, freshness, and policy state to the generated contract.
6. Replace all-contact maintenance with leased-peer maintenance and deterministic stream resolution.
7. Move relay health to demand/evidence policy.
8. Add Arti soft dormancy and background rendezvous windows.
9. Replace the radio polling worker with blocking/deadline execution.
10. Delete every replaced polling constant, reconnect-all path, and obsolete scheduler.

## Acceptance criteria

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
