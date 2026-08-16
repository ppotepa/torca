# Single-flight worker boundary

Torca intentionally does not provide a generic `SingleFlightJob<T>` today.

## Audit result

The attachment maintenance lane in `torca-communication-driver` is the only current worker with the complete ad-hoc single-flight shape:

- atomic active flag,
- one result slot,
- one detached bounded job,
- wake callback after completion.

The other background workers are not equivalent:

- relay health owns one long-lived command loop and `JoinHandle`;
- pairing owns one long-lived deadline-driven mailbox and `JoinHandle`;
- Tor recovery owns an epoch-tagged one-shot result and may intentionally outlive driver shutdown;
- onion publication owns a long-lived recovery loop with explicit shutdown commands.

Sharing a generic worker type between these lifecycles would hide cancellation, stale-result and ownership differences instead of removing duplication.

## Rule

Introduce a reusable `SingleFlightJob<T>` only when a second production owner has all of the following semantics:

1. one bounded operation may be active at a time;
2. completion is returned through a single replaceable result slot;
3. the owner does not need a persistent command loop;
4. shutdown semantics are the same;
5. stale-result handling is the same;
6. wake notification is the only completion side effect.

Until then, keep the attachment implementation local and explicit. `WakeSlot` is shared separately because its lifecycle semantics are genuinely identical.
