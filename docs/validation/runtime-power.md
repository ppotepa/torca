# Runtime power validation

This document validates the runtime-control invariants from
[`../architecture/runtime-control.md`](../architecture/runtime-control.md).
It is intentionally device-oriented: source inspection and CI cannot prove
physical battery behaviour.

## Baseline: one-device idle

Use a debug build on a physical Android device. Ensure there is no active
pairing, Radio session, outgoing message, attachment, or transfer.

1. Start a diagnostics observation session.
2. Background Torca and turn the screen off.
3. Wait 15–30 minutes, exceeding the 30-second background grace.
4. Stop the observation and mark an incident bundle in the Debug console.
5. Confirm that application-controlled background rendezvous, peer/relay
   probes, polling DB work, FFI polling, contact scans and reconnect attempts
   are zero; the scheduler must report no app-controlled next deadline.

The in-app observation/incident marker is the proof for runtime counters and
the grace event. The ADB soak harness independently verifies process, screen,
power state and captures the native JSONL tree; it must not infer a missing
structured event merely because an older build did not mirror every in-memory
diagnostic event to JSONL.

Use the single soak entry point. With no arguments it opens the scenario wizard:

```powershell
cargo run -p torca-soak
```

For a non-interactive idle battery gate:

```powershell
cargo run -p torca-soak -- --scenario idle-battery --plain `
  --android "<adb-serial>" --duration-seconds 3600 `
  --require-unplugged --require-screen-off --collect-native-diagnostics
```

## Active Android messaging measurement

Choose **Active messaging battery** in the wizard to run Android plus five
process-isolated bots. It uses a star topology: every bot pairs with Android,
so the phone must expose at least five contacts and conversations before the
measurement begins. Bots then send real messages through the production
runtime; this is the comparison run for an actively used client.

```powershell
cargo run -p torca-soak -- --scenario active-messaging --plain `
  --android "<adb-serial>" --android-auto-deploy --fake-peers 5 `
  --duration-seconds 3600 --require-unplugged
```

The run writes `plan.json`, `manifest.json`, message timeline and Android
battery start/end evidence under `.torca/soak/<run-id>/`. It fails before the
measurement if Android cannot expose the five expected bot relationships.

### Provisioned active-messaging fixture

Pairing is intentionally a two-step operation: the soak bot joins the
invitation and the inviter explicitly approves it. To avoid repeating that
interactive setup on every measurement, use the fixture lifecycle:

The default `fixture=none` path also performs the complete one-shot setup
(clean SOAK1 profile, deterministic nickname/avatar, pairing approval and
contact naming), but does not persist a reusable manifest. Use `provision` when
the resulting identities should be reused by later measurements.

```powershell
# One-time setup: clean SOAK1 profile, deterministic nicknames, real pairing
cargo run -p torca-soak -- --scenario active-messaging --plain `
  --android "<adb-serial>" --fake-peers 5 --fixture provision `
  --fixture-name android-default --duration-seconds 1 --fault-profile none

# Measurements: preserve the SOAK1 profile and validate contacts before traffic
cargo run -p torca-soak -- --scenario active-messaging --plain `
  --android "<adb-serial>" --fake-peers 5 --fixture reuse `
  --fixture-name android-default --duration-seconds 3600 `
  --require-unplugged
```

`provision` stores the device-bound manifest at
`.torca/soak/fixtures/<name>.json` and also writes a copy into the run
artifact. It contains only identity ids/fingerprints, display names and
relationship counts; it never contains pairing codes, private keys, messages
or attachment bytes. `reuse` refuses a different Android serial, a changed
identity/nickname, or missing contacts/conversations. The SOAK1 package and
profile are isolated from the normal Torca installation; ordinary deploys do
not consume this fixture.

Validation runs by default. The legacy PowerShell harness remains an internal
backend and CI compatibility shim.

```powershell
cargo run -p torca-soak -- --scenario idle-battery --plain `
  --android "<adb-serial>" --duration-seconds 21600 `
  --require-unplugged --require-screen-off --collect-native-diagnostics
```

Validate the resulting evidence before treating it as a release gate:

```powershell
./scripts/Validate-TorcaBatterySoak.ps1 `
  -Path artifacts/soak/battery-YYYYMMDD-HHMMSS `
  -MinimumMinutes 360 -RequireNativeDiagnostics -RequireObservation
```

## Required comparison matrix

Run equivalent conditions for:

| Scenario | Expected Torca-controlled activity |
| --- | --- |
| foreground idle | no cosmetic periodic peer or relay work |
| background idle | no deadline after grace; zero periodic rendezvous |
| pending message | only the delivery peer/lane is active |
| active pairing | relay lane is active, unrelated peers remain cold |
| active Radio | only the radio peer/lane has media deadlines |
| route change | demanded lanes recover; no all-contact reconnect storm |

Record device, Android version, build id, network type, power state, selected
battery mode and exact incident path. Compare repeated runs under the same
conditions; do not compare absolute battery percentages across different radios
or devices.

## Physical profiling

Use Android Studio Power Profiler / Perfetto for a release-profile measurement
when available. Correlate CPU, WLAN/cellular, UFS and wake-lock activity with
the diagnostics observation timeline. The in-app energy score is a regression
indicator, not a physical mWh measurement.
