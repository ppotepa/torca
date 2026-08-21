# Runtime control

This document is the canonical runtime-control contract. It describes the
current direction; it does not claim that a platform scenario has been device
validated unless that scenario is recorded in validation evidence.

## Ownership

`torca-runtime-policy` is the policy kernel consumed directly by
`RuntimeOwner`. `torca-battery` is transitional compatibility/telemetry
surface only; it must not become a second scheduler or executor owner.

`RuntimeOwner` is the sole owner of application-controlled runtime wake-ups and
effective executor policy. The native hosts report facts only: application
visibility, power/network state and persisted user preference. Flutter
attention is an ephemeral hint, not a source of delivery correctness.

```text
Flutter / native host facts
            |
            v
      RuntimeOwner
      /     |      \\
 policy  scheduler  diagnostics
            |
            v
 due, source-specific executor work
```

`RuntimeOwner` has one deadline registry. A wake has a typed source and only
the executor for that source may perform maintenance. An unrelated command or
deadline must not trigger broad Tor, relay, pairing, delivery, peer and radio
maintenance.

## Demand and evidence

A stored contact is passive. It creates no timer, probe, dial or worker merely
by existing. A peer becomes active only for durable work, an incoming session,
a focused conversation, an explicit diagnostic request or radio activity.

Durable work owns a durable lease. UI attention owns an `UntilRelease` lease
and is released when the UI changes attention or the host backgrounds. Real
authenticated TX, RX, ACK and handshake evidence refreshes health and is
preferred over a cosmetic probe. A network change is an event that recovers
only demanded work.

## Background lifecycle

The default lifecycle is:

```text
foreground -> background grace (30 seconds) -> SoftDormant
```

There is no periodic five-minute background rendezvous and no 90-second relay
lease. Background grace is a one-shot deadline. At expiry, when no durable work
remains, the Tor client enters soft dormancy; directory cache and onion identity
are retained. A user action, real durable work or a relevant platform event can
wake the required lane again.

The relay is an untrusted pairing rendezvous service, not a mailbox. It receives
a lease only for active pairing, pending relay work or explicit diagnostics.

## Battery settings

The supported user-facing choices are:

- **Automatic** — runtime chooses the least active policy consistent with
  current work and host facts.
- **Always reachable** — reliability-oriented availability.
- **Battery saver** — suppresses cosmetic work and allows aggressive idle
  dormancy.

Historical `Balanced` and cadence values are accepted during preference
migration, normalised to `Automatic` and `OnOpen`, and are not allowed to create
periodic background network work.

## Idle invariant

For background, screen-off idle with no pairing, radio, transfer, queued
delivery or other durable work, once background grace has expired:

```text
application-controlled next deadline = none
background rendezvous wakes          = 0
peer and relay probes                = 0
polling DB reads/writes               = 0
FFI polling                           = 0
full contact scans                    = 0
peer reconnect attempts               = 0
```

Arti, Android and the operating system may still perform their own internal
maintenance. These criteria apply only to Torca-controlled work.

## Observability and validation

Diagnostics records typed wake sources and supports observation baselines so a
single device can demonstrate what Torca itself did during an idle window. See
[`../validation/runtime-power.md`](../validation/runtime-power.md) and
[`../diagnostics.md`](../diagnostics.md) for the test and collection protocol.
