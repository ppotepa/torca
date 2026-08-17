# Android battery soak

This validation is intentionally device-side. It is not a CI check and must not be marked as passed from source inspection alone.

Run from the repository root with a physical Android device connected through ADB:

```powershell
./scripts/Run-TorcaBatterySoak.ps1 -DurationMinutes 60
```

The harness launches Torca once, backgrounds it, resets Android batterystats before the measured idle window, and captures battery/power/device-idle/service/process/logcat evidence under `artifacts/soak/`.

Acceptance review should compare at least two equivalent runs and inspect: Torca UID battery attribution, partial wakelocks, foreground-service residency, unexpected process restarts, repeated network bootstrap/reconnect loops, scheduler wake frequency, and whether an otherwise idle device is prevented from entering normal idle states.

For a release-oriented result, also run a longer 6–8 hour background window on battery power with the screen off. Keep device model, Android version, Torca build, network type, charging state and battery profile in the recorded test notes. Do not compare absolute battery percentages across materially different devices or radio conditions.
