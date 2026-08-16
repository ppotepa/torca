# Lock scope policy

Lock ownership was audited across the process registry, runtime event hub, communication driver and Tor lifecycle.

## Verified short locks

- native process `REGISTRY`: used only to read/publish an `Arc<RuntimeHandleInner>`; production composition and runtime startup happen after the registry guard is dropped;
- `RuntimeEventHub`: mutex protects only revision/cursor/cancellation counters and is released by `Condvar::wait` while sleeping;
- Tor `WakeSlot`: callback is cloned under the mutex and invoked after the guard is released;
- peer-link session maps are actor-owned and do not use cross-thread mutexes.

## Attachment owner

`TorcaCommunicationDriver` intentionally serializes access to the single `AttachmentRuntime`. The outgoing attachment pass can perform file/network work while it owns that runtime. Therefore the process actor must not acquire the attachment mutex merely to build projections or collect counters.

The required boundary is:

- inbound path: `try_lock`, defer when the attachment executor owns the runtime;
- snapshots: serve the last completed cached projection;
- counters: serve cached monotonic counters rather than blocking on the executor;
- only the attachment job itself may block while owning `AttachmentRuntime`.

The cache/counter conversion is tracked as the snapshot/cache cleanup rather than hidden inside this audit commit.

## Rules

1. Never hold the native process registry mutex while constructing storage, Tor, or application runtime objects.
2. Never invoke a callback while holding its callback-slot mutex.
3. Never perform network or file I/O while holding a lock whose other waiter is a process/UI actor.
4. A mutex that protects an executor may be held by that executor during its operation only when every competing process path uses non-blocking access or a cache.
5. Do not hold two independent runtime mutexes while calling into another component.
6. Condvar waits must release their state mutex and use a predicate loop.

Any exception must document its owner, waiters, maximum blocking operation, and why a mailbox/actor boundary would be worse.
