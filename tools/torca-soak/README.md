# Torca multi-peer soak

`torca-soak` drives real, process-isolated Torca runtimes. It is not a mock
transport test: every fake peer owns a native runtime, identity, SQLCipher
profile, Tor cache and structured log tree.

## Fake-peer-only run

Use an endpoint compiled into the lab peer binary:

```powershell
$env:TORCA_RELAY_ENDPOINT = '<v3-onion>.onion:443'
cargo build -p torca-lab-peer
cargo run -p torca-soak -- --relay external --relay-endpoint $env:TORCA_RELAY_ENDPOINT --duration-seconds 300
```

## Managed relay run

The managed mode starts the Docker relay, waits for its current endpoint, and
rebuilds `torca-lab-peer` with that endpoint before starting peers:

```powershell
cargo run -p torca-soak -- --relay managed --duration-seconds 1800
```

The workspace uses a compact shared developer profile so the headless peer
reuses the same Tor/SQLCipher artifacts as other local developer commands.
Production client profiles are unchanged. Override the binary with
`--lab-peer` when using a separately built executable.

The relay is stopped when the run exits, including when a peer or assertion
fails. Each run is written to `.torca/soak/<run-id>/` with a manifest and
JSONL timeline.

When stdout and stderr are interactive, the runner opens a Ratatui dashboard.
It shows relay/onion state, Android/peer readiness, workload counters and the
bounded event timeline. Controls are deliberately limited to safe operations:

```text
p or Space  pause/resume controlled waits
r           retry a failed Android preflight
m           write an incident marker under <run>/incidents/
l           open the bounded full JSONL event view
q or Esc    request cancellation and run normal cleanup
```

For CI, redirected output or scripts, explicitly use `--plain`; this keeps the
same manifest, timeline and summary artifacts without terminal control codes:

```powershell
cargo run -p torca-soak -- --plain --relay external --relay-endpoint $env:TORCA_RELAY_ENDPOINT
```

## Physical Android battery soak TUI

The physical battery measurement uses the PowerShell harness, while the
following binary supervises it with a Ratatui dashboard. It samples ADB power,
screen, installation and process state every two seconds and streams the
deploy/harness output into the dashboard:

```powershell
cargo run -p torca-soak --bin torca-battery-soak-tui -- `
  --device-id "adb-85Z5AIGU79XSLZMZ-RUuyXh._adb-tls-connect._tcp" `
  --duration-minutes 60 `
  --require-unplugged `
  --require-screen-off `
  --collect-native-diagnostics
```

The wrapper automatically adds `-ValidateAfter` to the PowerShell harness.
Press `q` or `Esc` to stop the harness and wake the device. Use `--plain` to
run the same physical measurement without the dashboard.

Interactive controls never mutate relay or client data directly. `q` stops the
scenario through its normal peer/ADB/relay cleanup path and records
`run_cancelled` in `summary.json`.

## Android participant

Build/install a debug APK containing the debug-only ScenarioBridge, then pass
the authorized ADB serial:

```powershell
cargo run -p torca-soak -- --android 2406APNFAG --relay managed
```

To let the orchestrator install/restart the current debug client (including the
debug-only ScenarioBridge), add `--android-auto-deploy`. The deploy is
restricted to that serial and preserves client data:

```powershell
cargo run -p torca-soak -- --android 2406APNFAG --android-auto-deploy --relay managed
```

The bridge binds only to Android loopback. The runner reads its random token
through `adb run-as`, creates an `adb forward`, and sends the same typed
pairing/message/attachment/radio operations used by fake peers. Release builds
do not start the bridge.

An absent or unauthorized ADB device is a preflight failure, never a passing
soak result.
