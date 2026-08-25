# SOAK1

SOAK1 is the developer-only, multi-participant communication and notification
soak. It runs the production Torca runtime in isolated bot profiles and the
Android `soak` flavor; the normal application entrypoint does not start or
import the scenario control plane.

Run the wizard from the repository root:

```powershell
.\scripts\soak.ps1 cockpit
```

The active messaging plan uses one Android device and five bot participants.
`Auto` fixture mode provisions and pairs the profiles on the first run, then
reuses and validates them on subsequent runs. A run writes its manifest,
timeline, notification observations, battery capture and summary below
`.torca/soak/<run-id>/`.
The shared fixture and bot roots are protected by a stale-process-aware lock at
`.torca/soak-state/active.lock`, so two runs cannot mutate the same lab setup.
Managed relay output is preserved as `relay-compose.log` before cleanup, which
allows a `RELAY_UNREACHABLE` failure to be diagnosed after the container exits.

The soak-only notification listener records system notification metadata for
end-of-run assertions. It never records message bodies, audio, keys or pairing
capabilities. Normal `normalDebug` and all production artifacts use
`main.dart`; SOAK uses `main_soak.dart` and the separate `.soak` application id.

Current correctness gates include participant readiness, fixture contacts and
conversations, message delivery, and private-message notification count. The
first battery run establishes a measurement artifact; later runs can compare
matching device/build/workload baselines.

The Active Messaging baseline does not inject faults unless a fault profile is
selected explicitly; fault recovery belongs to the RuntimeLab scenario.

If Android blocks installation (`INSTALL_FAILED_USER_RESTRICTED`), the cockpit
stays in an action-required state. Unlock the device, approve the install or
enable USB installation, then press `r` to retry without rebuilding the relay.
The cockpit shortcut `o` opens Android Developer options directly on the
selected device.
Every aborted setup also writes `failure.json` and a `run_failed` timeline event
so the failure is auditable even when no workload started. The failure includes
ADB transport, manufacturer, SDK and install-verification settings.
