# Android battery soak

This validation is intentionally device-side. It is not a CI check and must not be marked as passed from source inspection alone.

Run from the repository root with a physical Android device connected through ADB:

```powershell
./scripts/Run-TorcaBatterySoak.ps1 -DurationMinutes 60
```

For an energy-valid run, require the device to be disconnected from all
chargers:

```powershell
./scripts/Run-TorcaBatterySoak.ps1 -DurationMinutes 60 -RequireUnplugged
```

For the strict background-idle gate, also require the display to enter Doze:

```powershell
./scripts/Run-TorcaBatterySoak.ps1 -DurationMinutes 360 `
  -RequireUnplugged -RequireScreenOff
```

If more than one ADB transport is ready, pass the exact serial reported by
`adb devices` (wireless ADB serials include the mDNS suffix):

```powershell
./scripts/Run-TorcaBatterySoak.ps1 -DurationMinutes 60 `
  -DeviceId 'adb-<device-id>-<transport>._adb-tls-connect._tcp'
```

The harness refuses an ambiguous or stale device selection and verifies that
the Torca process is running before starting the measured window. With
`-RequireUnplugged`, it also fails before launching if Android reports AC, USB,
or wireless power.
With `-RequireScreenOff`, it sends `KEYCODE_SLEEP` and fails unless Android
reports `Dozing` or `Asleep` before the measured window begins.

The harness launches Torca once, backgrounds it, resets Android batterystats before the measured idle window, and captures battery/power/device-idle/service/process/logcat evidence under `artifacts/soak/`.

`result.json` also records the selected serial, PID, start/end battery levels,
power source, screen-state requirement and whether the process was still
present at the end of the window.

Acceptance review should compare at least two equivalent runs and inspect: Torca UID battery attribution, partial wakelocks, foreground-service residency, unexpected process restarts, repeated network bootstrap/reconnect loops, scheduler wake frequency, and whether an otherwise idle device is prevented from entering normal idle states.

For a release-oriented result, also run a longer 6–8 hour background window on battery power with the screen off. Keep device model, Android version, Torca build, network type, charging state and battery profile in the recorded test notes. Do not compare absolute battery percentages across materially different devices or radio conditions.
