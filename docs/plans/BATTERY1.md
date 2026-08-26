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

For Iroh, the endpoint profile is part of the artifact identity and must be
chosen before building. `always` keeps Iroh discovery/relay fallback enabled;
`direct` disables relay and address lookup for lower idle overhead but requires
an out-of-band complete endpoint address; `local` is for local/simulator
scenarios only. Iroh can therefore be more battery-efficient than Tor in the
direct/local profiles, but this is a measurable deployment trade-off rather
than an unverified guarantee: background reachability is reduced when relay
and discovery are disabled.

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
- Foreground permits visible demand and prevents dormancy, but is not a global
  `AlwaysAvailable` profile for unrelated runtime lanes.

## Delivery batches

| Batch | Status | Scope |
| --- | --- | --- |
| 1 | Implemented | Typed wake diagnostics and observation sessions. |
| 1a | Implemented | `ContactAvailabilityMode` moved to `torca-runtime-policy`; native, application and SQLite now consume that owner directly. |
| 1b | Implemented | `BatteryProfile` moved to `torca-runtime-policy`; runtime communication and attachment executors consume the policy owner directly. |
| 1c | Implemented | `MeteredTransferPolicy` moved to `torca-runtime-policy`; SQLite and transfer executors consume the policy owner directly. |
| 1d | Implemented | `RequestedBatteryMode` and `VisualActivityPolicy` moved to `torca-runtime-policy`; SQLite uses their canonical wire mapping. |
| 1e | Implemented | Legacy `BackgroundSyncCadence` moved to `torca-runtime-policy`; it remains an `on_open` migration value, never a timer policy. |
| 1f | Implemented | `SystemEnergyState` moved to `torca-runtime-policy`; host facts now share policy ownership with demand and attention. |
| 1g | Implemented | `BatteryPreferences`, `EffectiveBatteryPolicy` and `PolicyOverrideReason` moved to `torca-runtime-policy`; native, application and SQLite reducers now consume that canonical owner directly. |
| 1h | Implemented | Attachment admission (`BatteryPolicy` and `TransferDecision`) moved to `torca-runtime-policy`; transfer and RuntimeOwner no longer treat `torca-battery` as a policy owner. |
| 1i | Implemented | Battery ledger, bounded events and platform energy samples moved to `torca-diagnostics`; `torca-battery` was removed after all production consumers migrated. |
| 2 | Implemented core | RuntimeOwner receives atomic host-policy inputs and consumes `torca-runtime-policy` directly; legacy values normalize safely. Policy leases use explicit `Until`/`UntilRelease` lifetimes rather than a parallel persistent-owner side table. |
| 3 | Implemented core | One deadline registry with source-selective maintenance. |
| 4 | Implemented core | One-shot background grace and soft dormancy; no recurring rendezvous. Legacy persisted cadence values normalize to `on_open` and no runtime policy type exposes a periodic interval. |
| 5 | Implemented core | Demand/dirty-peer maintenance and unified platform visibility. Radio owns a separate deadline lane; peer maintenance derives its set from leases, live sessions, durable control outbox recipients and transport evidence rather than the contact book. Durable delivery and attachment leases are released by their job completion rather than synthetic TTL renewal. |
| 6 | Implemented | Debug-only Battery/Runtime/Logs/Incident console, explicit bounded log tails and local support bundle. |
| 7 | Implemented code | Canonical docs, deterministic validation and a production-crate headless lab peer are in place. Physical device validation remains explicit evidence, not an inferred code result. Iroh service configuration is embedded and its reachability probe is demand-driven, single-flight and bounded per network generation. |

The text delivery lane now claims a small batch (up to eight messages), groups
it by contact, and uses the provider batch boundary. Iroh emits the group with
one stream flush while preserving individual envelope ACKs and durable retry.

The remaining implementation work is:

1. Finish dirty-peer maintenance for any newly introduced delivery route so
   every delivery path continues to route only active contacts. Current
   messages, attachments and durable control-outbox recipients are scoped;
   startup recovery queries only `Queued`/`Sending` outbound message recipients.
2. Complete provider-owned direct-route refresh after a network generation
   change. The provider now exposes a route-generation diagnostic, marks the
   old route stale immediately, keeps migratable Iroh sessions alive and
   exchanges a validated opaque `Route` frame that is persisted atomically.
   The generic peer link now refuses a dial while the local route is stale.
   Pairing route sources now surface a typed `runtime.route_refresh_required`
   error instead of hiding a stale route as a pending invitation. A
   direct/local installation with no authenticated session now exposes an
   explicit provider refresh action through the contract and pairing/
   diagnostics UI; a failed refresh remains a visible retryable state and the
   runtime never retries an old address silently. This is implemented through
   the provider lifecycle port, not a Tor-only fallback.
3. Run the documented physical device validation matrix and retain its incident
   bundles as release evidence. A future Radio-specific experiment may add a
   QUIC datagram lane, but the current reliable stream remains the intentional
   provider-neutral baseline.

The optional dev-only incident ingest is intentionally not implemented in the
pairing relay: it would require a separate authenticated support service with
a bounded compressed payload, explicit token, TTL and no listing capability.
It is not a prerequisite for BATTERY1's local diagnostics or validation.

The native host accepts a process-scoped `TORCA_APP_ROOT` override for that
runner. It isolates the lab peer's identity, database and Tor state while
leaving the normal desktop storage root unchanged.

`tools/torca-soak` follows the same provider boundary. Active Messaging defaults
to Iroh `always` so a physical Android can reach isolated lab/remote bots across
mobile NAT; RuntimeLab uses Iroh `local`, while the idle battery baseline uses
Iroh `direct`. The selected profile is passed to the typed Rust deploy plan and
recorded in `manifest.json`. Set `TORCA_SOAK_IROH_PROFILE` to run an explicit
profile comparison. The legacy `scripts/Run-TorcaBatterySoak.ps1` remains the
minimal idle harness and does not start a relay unless Tor is selected.
Lab-peer compilation uses a provider/profile-specific target directory under
`.torca/soak/build`; this is intentional because the cockpit may itself be
started with `cargo run`, and nested Cargo builds must not contend for the
workspace target lock or reuse an artifact compiled for another provider.

Iroh's provider runtime uses a small bounded Tokio pool (two workers on
Android, four on desktop). `TORCA_IROH_RUNTIME_THREADS=1..=8` is embedded and
fingerprinted by the deployer for controlled experiments; it is not a user
setting and must only be changed as part of a repeatable battery run.

Active Messaging lab peers are not forced onto loopback: their Iroh endpoint
can publish a relay/direct route that the Android device can reach. Only the
RuntimeLab scenario enables `TORCA_IROH_LOCAL_ONLY=1`; this keeps its disposable
mesh deterministic without accidentally making the physical scenario
unreachable.

`cargo run -p torca-lab-peer -- --root <path>` starts one such production
runtime and reads diagnostics through the same contract ABI as Flutter. A
two-peer scenario will orchestrate two independent invocations. The runner
also exposes `--operation create`, `join --code <code>` and
`approve --session-id <id>` as the native contract pairing commands; it prints
the redacted pairing projection after each successful command.
It waits for `bootstrapPhase=ready` before executing an operation, with a
bounded `--startup-timeout-seconds` rather than an implicit sleep.

The deterministic soak now includes both `torca-runtime` and
`torca-diagnostics`, so policy scheduling and observation/export regressions
are exercised together on every requested iteration.

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
