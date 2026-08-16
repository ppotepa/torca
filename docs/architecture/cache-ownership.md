# Snapshot and cache ownership

Caches in Torca are projection/telemetry accelerators only. Durable state remains owned by repositories, protocol state machines and protected secret stores.

| Cache / projection | Owner | Refresh / invalidation | Failure fallback |
|---|---|---|---|
| native `snapshot_json` | `TorcaRuntime` | successful `refresh_snapshot`; command/background reconciliation | keep the previous valid snapshot when refresh fails |
| native `query_json` | `TorcaRuntime` | each explicit query | query-scoped error response; never treated as durable state |
| notification event window | `TorcaRuntime` | read-model reconciliation; bounded to 256 events | cursor remains monotonic for the process lifetime |
| attachment snapshot cache | `TorcaCommunicationDriver` | after successful inbound/outgoing attachment processing | last completed projection while attachment executor is busy |
| peer transport activity | peer-link/runtime owner | every completed/failed transport observation | monotonic payload-free counters only |
| relay health snapshot | `RelayHealthWorker` | one completed relay probe | stale-while-revalidate; no UI flicker to `Checking` after a usable sample |
| bootstrap progress projection | native host | Tor bootstrap observer / attached runtime | attached runtime becomes authoritative after startup |

## Invariants

1. A cache may not be used to acknowledge durable peer data.
2. Cache invalidation must be driven by the owner that mutates the underlying state; callers must not guess expiration from UI polling.
3. Projection reads must not block a process actor on network/file I/O. If an executor is busy, use the last complete cache or a non-blocking read.
4. Cache entries never contain long-lived secrets, plaintext key material, relay tickets or SQLCipher keys.
5. Monotonic counters must not be reset by a temporary busy/poisoned executor read; cache the last successful sample instead.
6. `snapshot_json` refresh failure must preserve the previous valid snapshot; background reconciliation moves the old string rather than cloning it and restores it on failure.
7. Any new cache must document owner, refresh trigger, invalidation trigger and fallback in this file.

## Attachment follow-up rule

The attachment executor is allowed to own its runtime mutex while doing one bounded transfer pass. Callers that only need presentation or telemetry data must use the attachment projection/counter cache rather than wait for that mutex. This keeps file/network I/O off the runtime actor's lock path.
