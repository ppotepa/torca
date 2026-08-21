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

## Android participant

Build/install a debug APK containing the debug-only ScenarioBridge, then pass
the authorized ADB serial:

```powershell
cargo run -p torca-soak -- --android 2406APNFAG --relay managed
```

To let the orchestrator install the debug client when the selected device has
no launchable activity, add `--android-auto-deploy`. The deploy is restricted
to that serial and preserves client data:

```powershell
cargo run -p torca-soak -- --android 2406APNFAG --android-auto-deploy --relay managed
```

The bridge binds only to Android loopback. The runner reads its random token
through `adb run-as`, creates an `adb forward`, and sends the same typed
pairing/message/attachment/radio operations used by fake peers. Release builds
do not start the bridge.

An absent or unauthorized ADB device is a preflight failure, never a passing
soak result.
