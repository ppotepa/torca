# Iroh background test report

Date: 2026-08-27  
Repository: `G:\Git\torca-test`

## Result

The new unattended runner completed successfully while using a headless
Android emulator:

- Iroh transport tests: **20 passed**;
- provider conformance tests: **4 passed**;
- Android profile: Iroh `always`;
- screen state: screen-off background mode;
- emulator: `TorChat_API36`, two virtual cores, 1536 MB RAM;
- runner result: **passed**;
- cleanup: emulator stopped automatically;
- artifact: `.torca/measurements/background/20260827-182434-4c533193/always/always-1.json`.

The first five-second sample reported a median of **2%** and P95 **56%**, which
showed a short startup/wake burst. After the QEMU-aware host guard change, a
second run reported **0% median** and **3.8% P95**. Both runs validate runner
startup/cleanup; they are too short to establish an idle-energy baseline. Use
the planned 30–60 minute soak for comparisons.

## Background safety

A post-fix smart-smoke run (10 s, one repetition) reached `MainActivity`,
passed fatal-log checks and completed screen-off measurement at **8% median /
11.5% P95**; its startup evidence is under
`.torca/measurements/background/smart-smoke-verify7/`.

`scripts/Start-TorcaBackgroundTest.ps1` and
`scripts/run-android-emulator-cpu.ps1` now provide:

- a lock preventing concurrent emulator runs;
- hidden, no-audio, no-window emulator startup;
- BelowNormal emulator priority;
- two-core/1536 MB default resource budget;
- host CPU guard at 15% of the machine, with fail-closed termination;
- Ctrl+C/failure cleanup through `finally`;
- JSON status with `passed`, `failed`, `pending` or cancellation reason;
- sequential `always`/`direct`/`local` execution in `full` mode.

The helper does not claim calibrated battery energy. Emulator battery values
are synthetic and must not be used as mAh evidence.

## Smart runner and startup diagnostics

The Android runner now verifies the application before measuring CPU. It waits
for `sys.boot_completed`, the Android package manager, and the app's resumed
`MainActivity`; it also scans the last 600 logcat lines for fatal startup
markers. Every repetition stores `startup-activity.txt`, `startup-logcat.txt`,
`startup-ui.xml` and `screen-power.txt`. Headless AVDs do not expose a stable
uiautomator hierarchy, so the artifact explicitly records
`UI_PROBE_SKIPPED: headless benchmark (-no-window)` and uses activity focus plus
logcat as the readiness signal. A fatal marker or readiness timeout fails the
run and preserves all diagnostics.

ADB package probes and installs are retryable/bounded. A transient `device
offline`/package-service race is deferred to the authoritative SOAK deploy;
an install cannot block indefinitely. The runner also creates the conversation
artifact directory before invoking the scenario, so a failure always has a
`last-failure.log`.

## Iroh coverage

The Rust conformance layer covers persisted contacts, credentials, handshake,
bidirectional text, attachments, control frames, receipts, restart,
non-preferred durable sending, stale routes and route refresh. The remaining
external scenarios are:

1. three-run Android release CPU matrix for `always`, `direct` and `local`;
2. relay/discovery 2×2 isolation on a real network;
3. Wi-Fi to LTE migration for Iroh `always`;
4. calibrated mAh/current attribution using a phone exposing `current_now` or
   an external power monitor.

## Interpretation of the battery investigation

Desktop measurements show Iroh `always` as the dominant provider-specific CPU
cost because it keeps relay/discovery activity alive. Android's earlier 100%+
CPU result was instead caused by the foreground-service snapshot/audio
enumeration loop; the event-driven waiter and audio readiness cache removed
that loop. Current physical-phone samples are approximately 0–2% P95 CPU with
about 33°C battery temperature, but they are not an energy measurement.

Avatars already render from spritesheets with a shared frame clock. They are
not implicated in the idle drain; a future regression test should only verify
that background and reduce-motion modes do not advance frames.

## Commands

```powershell
.\scripts\Start-TorcaBackgroundTest.ps1 -Mode smoke -DurationSeconds 5
.\scripts\Start-TorcaBackgroundTest.ps1 -Mode soak -Profile always -DurationSeconds 60 -Repetitions 3
.\scripts\Start-TorcaBackgroundTest.ps1 -Mode full -DurationSeconds 60 -Repetitions 3
```

Hardware-only gates remain intentionally fail-closed when required telemetry
or controllable network transitions are unavailable.

The smart conversation path was exercised against the headless AVD. App
startup and readiness passed, but the run remained in the first-time
`torca-lab-peer` build/deploy phase and was stopped after a bounded background
wait; its complete diagnostics are in
`.torca/measurements/background/conversation-verify4/`. This is an incomplete
scenario result, not a passing messaging claim. Re-run it after the lab-peer
artifact is prebuilt (or provide `--lab-peer`) to validate the bot contact,
pairing, bidirectional messages, attachments and receipts end-to-end.
