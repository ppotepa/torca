# Android connectivity soak

This is a physical-device validation. Use USB ADB so toggling Wi-Fi does not disconnect the test controller.

Run:

```powershell
./scripts/Run-TorcaConnectivitySoak.ps1 -Iterations 10 -SettleSeconds 15
```

When multiple devices are connected, select one explicitly with the exact
serial from `adb devices`:

```powershell
./scripts/Run-TorcaConnectivitySoak.ps1 -Iterations 10 -SettleSeconds 15 `
  -DeviceId 'adb-<device-id>-<transport>._adb-tls-connect._tcp'
```

The harness fails before changing network state if the selected device is not
ready, and verifies that the Torca process started on that same device.

Optionally include mobile-data transitions:

```powershell
./scripts/Run-TorcaConnectivitySoak.ps1 -Iterations 10 -ToggleMobileData
```

The harness records a timestamped route-change timeline plus `dumpsys connectivity`, Torca service state, process state and logcat evidence under `artifacts/soak/`.

Review criteria: no crash/ANR, no unbounded worker/thread growth, no repeated concurrent Tor bootstrap workers, no permanent `Starting` state after the final stable network, no endless onion re-publication storm, and no background retry loop materially faster than the configured backoff. After the final recovery, manually verify one pairing/relay operation and one peer message operation on the same build.

Do not classify this as passed from source inspection. Record device model, Android version, app build, transport used for ADB, network topology and whether mobile data was toggled.
