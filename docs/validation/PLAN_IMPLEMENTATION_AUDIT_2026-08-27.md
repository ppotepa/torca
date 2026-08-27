# Torca provider/communication plan — implementation audit

Generated: 2026-08-27  
Repository: `G:\Git\torca-test`

This document is an evidence index for the twelve-step restoration and provider
architecture plan. “Implemented” means that the current source contains the
behavior and a targeted test or compile check covers it. Hardware-only checks
are explicitly marked pending rather than inferred from emulator results.

## Requirement matrix

| Step | Requirement | Implementation evidence | Verification | Status |
| ---: | --- | --- | --- | --- |
| 1 | Correct `route_stale` guard and factory-created Iroh route behavior | `crates/infrastructure/torca-transport-iroh/src/lib.rs` (`route_stale`, endpoint slot and factory guards) | Iroh unit tests for fresh/stale/refreshed routes and factory path | Implemented |
| 2 | Persisted-contact Iroh peer handshake reaches Ready and carries ACKs | `torca-transport-iroh` persisted contact/credential path; `torca-peer-link` handshake/authentication | `torca-provider-conformance::iroh_direct_provider_satisfies_peer_contract` | Implemented |
| 3 | Durable reconnect demand survives dialer election | `torca-peer-link/src/owner/reconnect.rs`, `public_methods.rs`, `session_methods.rs` | Reason precedence, non-preferred sender and restart tests | Implemented |
| 4 | Prime relationships only after durable pairing completion | `PairingMaintenanceReport.completed_contacts`, pairing worker report collection, runtime maintenance | Deduplicated completion report and runtime `prime_contact` tests | Implemented |
| 5 | Pipeline diagnostics distinguish factory/connect/handshake and message/ACK stages | `torca-peer-link` telemetry stages, connectivity observer and native diagnostic logs | Targeted peer/runtime tests; diagnostic files under native run logs | Implemented |
| 6 | Separate connection state from user-facing availability | `torca-presence::PeerAvailability`, `PeerHealthSnapshot`, Rust/Flutter contract projections | Presence classification tests and Flutter analyze/tests | Implemented |
| 7 | Memory and Iroh provider conformance | `crates/infrastructure/torca-provider-conformance` | Memory + Iroh DirectOnly bidirectional text, attachment/control payloads and receipts | Implemented |
| 8 | Restart, durable non-preferred send and route migration | Conformance restart test; route stale/refresh tests; `scripts/run-iroh-routing-isolation.ps1` | Rust conformance suite and PowerShell syntax validation | Implemented; physical relay matrix pending |
| 9 | Provider metadata/plugin boundary | `torca-provider-api`, `NativeCommunicationProviderPlugin` composition and compile-time registry | Workspace compile and provider conformance tests | Implemented |
| 10 | One provider routing/bootstrap owner | `ProviderComponents.routing: Arc<dyn ProviderRouting>` and provider-specific routing implementations | Workspace compile; pairing and peer-link use shared routing trait | Implemented |
| 11 | Opaque `ContactRoute` with legacy Tor SQL compatibility | `torca-contacts::ContactRoute` stores `ProviderId` → opaque bytes; SQLite legacy onion read/write adapter | Contact/storage tests; no `.onion` interpretation in contacts, runtime, peer-link or pairing coordinator | Implemented |
| 12 | Acceptance gates and evidence | `scripts/Invoke-TorcaEnergyGate.ps1`, avatar validator, routing isolation script, this report | 470 Rust tests, workspace check, Flutter checks, script parser checks; physical `always` CPU smoke | Implemented; calibrated mAh/network migration pending |

## Automated verification run

The following checks were run against the current worktree:

```text
cargo test --workspace --all-targets --locked       470 passed (77 suites)
cargo check --workspace --all-targets --locked      passed
cargo clippy --workspace --all-targets --locked -D warnings passed
cargo run -p torca-contract-gen --locked -- --check ... passed
Validate-TorcaWorkspace.ps1                         passed
Validate-TorcaAvatarAssets.ps1                      passed
cargo test -p torca-attachment-transfer --lib --locked  5 passed
git diff --check                                  passed
PowerShell parser checks for benchmark scripts       passed
Flutter analyze/test                               passed (92 app tests)
Flutter avatar package tests                       passed (12 tests)
```

CodeGraph was synchronized after the last source changes and reports an
up-to-date index.

The Iroh factory test now covers the complete stale→refresh transition: a
stale route rejects factory creation, the refreshed route creates a transport,
and the transport reconnects through the remote incoming router.

The routing-isolation harness now keeps feature polarity correct: `relay-on`
and `discovery-on` clear the corresponding `TORCA_IROH_DISABLE_*` build flags,
while the `*-off` cases set them. This prevents mislabeled four-way runs.

Telemetry calls in `torca-peer-link` now use a typed `TelemetryEvent` value
instead of a broad lint suppression, so the diagnostics path also passes the
repository's lint-suppression policy.

The Android build/deploy pipeline was also corrected for Flutter flavor output:
new Flutter versions emit `app-normal-<abi>-<configuration>.apk` under
`build/app/outputs/apk/normal/<configuration>`, while the deployment code
expected the older `app-<abi>-<configuration>.apk` layout. The build now
normalizes both layouts, tolerates Flutter's erroneous non-zero wrapper exit
when valid split APKs exist, and validates the normalized artifacts. A full
`scripts/build.ps1 -Target android -Configuration release ...` completed with
both ABI packages verified.

## CPU/battery evidence

The detailed measurements are in
[`ENERGY_AUDIT_2026-08-27.md`](ENERGY_AUDIT_2026-08-27.md). The important causal
finding is an attachment worker retry loop caused by invalid uppercase
`ErrorCode` values. The values are now lowercase `attachment.*` codes and the
post-fix 30-second Release smoke log contains no `invalid error code`, `worker
panic`, or maintenance retry entries.

Avatar rendering was not the dominant measured idle source. Avatars already
use spritesheets and a shared frame clock; desktop lifecycle now pauses frame
invalidation while the window is minimized.

Android profiler symbolization mapped the hot path to
`snapshot_context → SharedRadioCoordinator::projection → CPAL AAudio`.
`microphone_ready()` was re-enumerating the default AAudio input on every
snapshot; a readiness cache now invalidates only after an explicit device
configuration change. On the rebuilt Release x86_64 APK, verified screen-off
and battery-powered, CPU fell from **104%/110% median/P95** before the cache to
**0%/1%** after it. This is the strongest causal A/B evidence in the audit.

The emulator benchmark is reproducible with
`scripts/run-android-emulator-cpu.ps1`. It starts the configured AVD, installs
the normalized release APK, enforces screen-off state, runs the state-aware
process sampler, writes per-run JSON plus a summary, and always shuts the
emulator down. A one-run self-test on 2026-08-27 produced
`medianCpuPercentOfOneLogicalCpu = 0.0` and left no ADB device running.

One automated minimized-window gate run after the fix reported exactly 0.0%
CPU. Because the executable started without a usable main window in that run,
that result is retained as a measurement edge case, not as a replacement for a
normal three-run release benchmark. The measurement script now marks an
all-zero CPU sample set invalid and the energy gate fails closed with a
warning, preventing a false green result. The earlier valid smoke sample was
0.4832% of a 16-logical-CPU machine (median, 0.7678% p95).

The current Windows Release executable was also measured with the windowed
sampler: the normal-window run was valid at median `0%` / P95 `0.9683%` of the
machine (`15.493%` / one logical CPU). The minimized run was correctly rejected
as invalid because Windows suspended the process and produced no CPU samples.
The desktop energy gate can now launch an executable itself with
`-LaunchIfMissing -ExecutablePath <path-to-torca_app.exe>` and only closes a
process it started, enabling unattended benchmark runs.

The Android foreground service now uses the existing cancellable JNI
`nativeWaitForNotification` waiter instead of a 1.5-second polling timer; the
new release APK rebuilt successfully for both ABIs and passed the emulator
screen-off self-test (`0%/0%` median/P95).
The waiter is stored in `tools/build/overlays/android/TorcaForegroundService.kt`,
the build source of truth, so platform-asset preparation cannot overwrite the
optimization with an older polling implementation.

The overlay now restores Android lifecycle telemetry without polling: power
save and charging broadcasts, plus deduplicated metered/validated network
capabilities and a debounced default-route change signal. Destruction removes
all callbacks/receivers, keeping the foreground service lifecycle bounded.
Android API S+ connectivity diagnostics also map data-stall suspicion and
recovery to the native `data_stall_on/off` lifecycle events on a dedicated
executor, which is shut down with the service.
The callbacks are edge-triggered with an atomic state guard, so repeated
connectivity reports do not create repeated native wake-ups.

The Android benchmark additionally captures `current now` and battery
temperature from `dumpsys battery` when available, so the pending handset gate
will produce CPU, charge-counter, current, and thermal evidence in one JSON
artifact.

A physical handset smoke run is now available: 30-second Release
`iroh/always`, screen-off/Dozing, measured 0% median and 1% P95 CPU with
33.3 °C reported battery temperature. The handset exposes no readable
`current_now`, so calibrated mAh/current attribution remains pending and is
correctly rejected by `-RequireBatteryTelemetry`.

The GitHub validation workflow now runs provider conformance explicitly, a
workspace-wide `clippy -- -D warnings` gate, and avatar asset validation on
every PR. Hardware-only energy and route-migration jobs remain opt-in because
they require a connected handset and controllable network transitions.
It also runs Tor transport/runtime unit tests (13 tests in `torca-tor`; the
transport adapter currently has no unit cases) alongside the Iroh checks.

## Remaining external evidence

A physical Android handset is now available. Three valid 30-second
screen-off `iroh/always` repetitions produced 0% median CPU and 1%, 1% and 2%
P95 of one logical CPU, with stable level/charge-counter and approximately
33.3 °C battery temperature. The handset does not expose `current_now`, so
this is physical CPU/thermal evidence but not a calibrated mAh measurement.

The following still cannot be marked complete from this environment:

- calibrated physical Android mAh/current attribution (requires exposed
  current telemetry or an external power monitor);
- three-run Android release matrix for `always`, `direct` and `local` profiles;
- 2×2 Iroh relay/discovery isolation on a real handset/network;
- Wi‑Fi → LTE migration for the `always` profile.

Run the following once a device is available:

```powershell
.\scripts\Invoke-TorcaEnergyGate.ps1 -Platform android -AndroidSerial <serial> -Package com.torca.torca_app -Profile always -Mode background -DurationSeconds 60 -Repetitions 3
.\scripts\Invoke-TorcaEnergyGate.ps1 -Platform android -AndroidSerial <serial> -Package com.torca.torca_app -Profile always -Mode background -DurationSeconds 60 -Repetitions 3 -RequireBatteryTelemetry -FailOnRegression
.\scripts\run-iroh-routing-isolation.ps1 -AndroidSerial <serial> -DurationMinutes 30 -Repetitions 3
```

## Latest background battery work

The background runner is resource-bounded and QEMU-aware. A three-run,
60-second Iroh `always` soak completed with median CPU `0%` and P95 `1%` for
all repetitions; the emulator was cleaned up automatically. A shorter smoke
run after the guard change reported `0%` median and `3.8%` P95.

Desktop lifecycle now emits `backgrounded` on window minimize and
`foregrounded` on restore, allowing the existing runtime dormancy policy to
run instead of only pausing avatar animation. A release Flutter build
succeeded and the normal-window sampler remained valid at `0%` median and
`0.7684%` P95 of the host machine. Windows suspension makes a minimized
all-zero sample invalid for CPU attribution; this remains explicitly marked
as a limitation.

The runner now performs smart startup validation: boot-complete and package
manager readiness, resumed `MainActivity`, fatal-log scanning, and persisted
startup/activity/power/UI-probe artifacts. Headless AVDs explicitly record that
uiautomator is skipped because API 35/36 can block the probe. ADB install and
package races are bounded/retryable. The Iroh-only `conversation` mode invokes
the production SOAK bot path (pairing, persisted contact, messages, receipts)
and always creates `last-failure.log`; a first-time lab-peer build can be
prebuilt or supplied with `--lab-peer` before claiming an end-to-end pass.
