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

Run the Android harness from the repository root for a longer physical soak:

```powershell
./scripts/Run-TorcaBatterySoak.ps1 -DurationMinutes 60 -RequireUnplugged -CollectNativeDiagnostics
```

For an interactive live view of the physical Android run, use the Ratatui
supervisor (it launches the same PowerShell harness and adds `-ValidateAfter`):

```powershell
cargo run -p torca-soak --bin torca-battery-soak-tui -- `
  --device-id "<adb-serial>" --duration-minutes 60 `
  --require-unplugged --require-screen-off --collect-native-diagnostics
```

To fail the command automatically when the captured evidence does not satisfy
the requested duration, power, screen, process or diagnostics gates, append
`-ValidateAfter`:

```powershell
./scripts/Run-TorcaBatterySoak.ps1 -DurationMinutes 60 `
  -RequireUnplugged -RequireScreenOff -CollectNativeDiagnostics -ValidateAfter
```

For a strict screen-off window:

```powershell
./scripts/Run-TorcaBatterySoak.ps1 -DurationMinutes 360 `
  -RequireUnplugged -RequireScreenOff -CollectNativeDiagnostics
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
