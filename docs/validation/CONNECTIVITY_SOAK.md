# Android connectivity soak

This is a physical-device validation. Use USB ADB so toggling Wi-Fi does not disconnect the test controller.

Run the unified soak entry point; without arguments it opens the Ratatui
wizard and detects ready Android devices:

```powershell
cargo run -p torca-soak
```

For CI or a scripted run, select the scenario and device explicitly:

```powershell
cargo run -p torca-soak -- --scenario connectivity --plain `
  --android 'adb-<device-id>-<transport>._adb-tls-connect._tcp' `
  --iterations 10
```

The harness fails before changing network state if the selected device is not
ready, and verifies that the Torca process started on that same device.

`Run-TorcaConnectivitySoak.ps1` remains the internal backend and CI
compatibility shim. Mobile-data fault injection stays intentionally internal
until it is represented by a typed option in `SoakPlan`.

The harness records a timestamped route-change timeline plus `dumpsys connectivity`, Torca service state, process state and logcat evidence under `artifacts/soak/`.

Review criteria: no crash/ANR, no unbounded worker/thread growth, no repeated concurrent Tor bootstrap workers, no permanent `Starting` state after the final stable network, no endless onion re-publication storm, and no background retry loop materially faster than the configured backoff. After the final recovery, manually verify one pairing/relay operation and one peer message operation on the same build.

Do not classify this as passed from source inspection. Record device model, Android version, app build, transport used for ADB, network topology and whether mobile data was toggled.
