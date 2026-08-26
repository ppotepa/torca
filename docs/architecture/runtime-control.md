# Runtime control

This document is the canonical runtime-control contract. It describes the
current direction; it does not claim that a platform scenario has been device
validated unless that scenario is recorded in validation evidence.

## Ownership

`torca-runtime-policy` is the policy kernel consumed directly by
`RuntimeOwner`. `torca-diagnostics` owns bounded telemetry and redacted host
energy samples. There is no second battery scheduler or executor owner.

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
remains, the selected provider enters its provider-owned dormant state. Tor
keeps its cached identity while reducing network activity. Iroh's relay-backed
`always` profile closes the endpoint and later rebinds it from the same
protected endpoint secret; its `direct`/`local` profiles already have relay and
discovery disabled, so they keep the cheap bound listener to preserve the
opaque endpoint route while suppressing reachability probes. A user action,
real durable work or a relevant platform event can wake the required lane
again. The application must not assume that every provider has an onion
service or that dormant means the same thing for every transport.

Foreground is likewise a host fact rather than an implicit
`AlwaysAvailable` profile. It permits visible UI demand and prevents dormant
Tor while the app is present, but it does not promote unrelated peer, relay or
cosmetic probe work.

The relay is an untrusted pairing rendezvous service, not a mailbox. It receives
a lease only for active pairing, pending relay work or explicit diagnostics.

## Iroh energy modes

Iroh is not automatically cheaper merely because it uses QUIC. Its energy
profile is an explicit deployment choice and must be measured on the target
device. The provider exposes three modes:

| mode | relay/discovery | inbound reachability | intended use |
| --- | --- | --- | --- |
| `always` | configured relay and optional address lookup | monitored while awake | internet reliability |
| `direct` | disabled | no public reachability probe | paired devices on a reachable direct path |
| `local` | disabled | no public reachability probe, loopback-friendly | lab/soak runs |

`direct` and `local` are the lowest application-controlled idle cost because
they do not create relay or discovery workers. They trade that cost for
availability: a direct endpoint address can become stale after a Wi-Fi/LTE
change and may require a route refresh or re-pairing. The `always` mode is the
reliable default for production-style mobile tests, but its relay/discovery
traffic must be compared with Tor using the same workload.

The Iroh `online` check is demand-driven and bounded to three attempts per
network generation. Construction of an endpoint does not start a network
report. A foreground/AlwaysAvailable/durable-reachability demand starts one
single-flight probe; after three failures it stops creating timers and waits
for a platform network event or a new demand edge. This is an important
battery invariant: an offline or captive network must not leave a permanent
exponential retry loop running.
The selected QUIC path is observed dynamically (`direct` or `relay`) rather
than inferred from the deployment profile, so diagnostics can attribute work
to the route that was actually used.

The same diagnostics record also exposes endpoint, route and network
generations,
`reachabilityDemanded` and bounded `onlineProbeAttempts`/
`onlineProbeFailures` counters. These counters are workload evidence, not a
scheduler: collecting them never creates a new wake-up.

An in-flight online report is cancellable. Withdrawing the reachability lease,
entering dormancy, or receiving a network-generation event wakes the probe
task immediately; it does not wait for the 30-second report timeout. This
keeps a foreground-to-background transition cheap even when the network is
offline or captive.

`routeGeneration` is deliberately separate from `endpointGeneration`. Iroh
can retain the same endpoint identity while its advertised address gains or
loses direct/relay candidates after Wi-Fi, LTE or relay migration. A provider
consumer must treat a route-generation change as stale until the updated
opaque route is exchanged with the peer; it must not silently keep dialing a
captured address forever. `routeFresh` is false while an asynchronous Iroh
network migration is in progress, so no pre-migration endpoint can be
advertised. The provider also increments the route generation when a completed
online report observes a changed address, so diagnostics can distinguish an
explicit platform transition from asynchronous Iroh discovery.

Provider service configuration is embedded into the selected artifact and
fingerprinted by the deployer. `TORCA_IROH_RELAY_URLS`,
`TORCA_IROH_PKARR_URL`, `TORCA_IROH_DISABLE_RELAY`,
`TORCA_IROH_DISABLE_DISCOVERY`, `TORCA_IROH_LOCAL_ONLY` and
`TORCA_IROH_RUNTIME_THREADS` cannot be changed silently by a host shell after
artifact verification. The direct/local profiles ignore relay and lookup
values by design. `TORCA_IROH_RUNTIME_THREADS` is an experimental bounded
knob (`1..=8`) intended for device measurements; Android defaults to two
workers and desktop to four.

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
