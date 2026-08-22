# Torca soak wizard

All developer soak tests have one entry point. Use the PowerShell bootstrap
when starting from a checkout; it keeps Cargo/build/linker output in a bounded
bootstrap log and opens the Ratatui cockpit only after the binary is ready:

```powershell
.\scripts\soak.ps1 cockpit
```

The Rust command remains useful for CI or an already-built checkout:

```powershell
cargo run -p torca-soak
```

Do not use the removed `torca-battery-soak-tui` binary name. The canonical
binary is `torca-soak`; legacy PowerShell scenario scripts are compatibility
backends, not interactive entry points.

With no arguments it opens a Ratatui wizard, discovers ready ADB devices and
offers five explicit scenarios:

| Scenario | Purpose |
| --- | --- |
| Active messaging battery | Android plus five bots in a star topology; contacts are visible on Android and bots send real messages |
| Idle battery baseline | Physical Android measurement with deliberately no synthetic traffic |
| Connectivity recovery | Repeated Android route loss/recovery |
| Multi-peer runtime lab | Process-isolated peers, attachments, Radio and controlled faults |
| Deterministic code soak | Repeated policy/runtime Rust suites |

The PowerShell files under `scripts/` are implementation backends and CI
compatibility shims. They are not separate developer entry points.

## Fake-peer-only run

Use an endpoint compiled into the lab peer binary:

```powershell
$env:TORCA_RELAY_ENDPOINT = '<v3-onion>.onion:443'
cargo build -p torca-lab-peer
cargo run -p torca-soak -- --scenario runtime-lab --plain --relay external --relay-endpoint $env:TORCA_RELAY_ENDPOINT --duration-seconds 300
```

## Managed relay run

The managed mode starts the Docker relay, waits for its current endpoint, and
rebuilds `torca-lab-peer` with that endpoint before starting peers:

```powershell
.\scripts\soak.ps1 cockpit --scenario runtime-lab --relay managed --duration-seconds 1800
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
cargo run -p torca-soak -- --scenario runtime-lab --plain --relay external --relay-endpoint $env:TORCA_RELAY_ENDPOINT
```

## Non-interactive physical Android battery soak

CI and scripted runs select the same backend explicitly:

```powershell
.\scripts\soak.ps1 plain --scenario idle-battery `
  --android "adb-85Z5AIGU79XSLZMZ-RUuyXh._adb-tls-connect._tcp" `
  --duration-seconds 3600 `
  --require-unplugged `
  --require-screen-off `
  --collect-native-diagnostics
```

Interactive controls never mutate relay or client data directly. `q` stops the
scenario through its normal peer/ADB/relay cleanup path and records
`run_cancelled` in `summary.json`.

## Android participant

Build/install a debug APK containing the debug-only ScenarioBridge, then pass
the authorized ADB serial:

```powershell
cargo run -p torca-soak -- --scenario active-messaging --android 2406APNFAG --fake-peers 5 --relay managed
```

To let the orchestrator install/restart the current debug client (including the
debug-only ScenarioBridge), add `--android-auto-deploy`. The deploy is
restricted to that serial and preserves client data:

```powershell
cargo run -p torca-soak -- --scenario active-messaging --android 2406APNFAG --fake-peers 5 --android-auto-deploy --relay managed
```

The bridge binds only to Android loopback. The runner reads its random token
through `adb run-as`, creates an `adb forward`, and sends the same typed
pairing/message/attachment/radio operations used by fake peers. Release builds
do not start the bridge.

An absent or unauthorized ADB device is a preflight failure, never a passing
soak result.

## Persistent server bots

For repeated Android measurements, run the dev-only bot host beside the relay
so five bot identities and their pairings survive between runs:

```powershell
$env:TORCA_RELAY_ENDPOINT = '<current-onion>.onion:443'
$env:TORCA_SOAK_BOT_TOKEN = '<random-token-with-at-least-16-characters>'
docker compose -f infra/docker/compose.yml -f infra/docker/compose.soak.yml up --build soak-bot-host
```

Then point the cockpit at its loopback control endpoint:

```powershell
.\scripts\soak.ps1 cockpit --scenario active-messaging `
  --android <adb-serial> --android-auto-deploy `
  --bot-host 127.0.0.1:47890 --bot-token $env:TORCA_SOAK_BOT_TOKEN
```

Without `--bot-host`, the runner uses the same production `torca-lab-peer`
processes locally, but still keeps their roots under `.torca/soak/bots/`.
