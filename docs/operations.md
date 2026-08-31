# Runtime operations and diagnostics

This page covers the current build/deploy/runtime operational model. It is not a release checklist; exact deployment evidence belongs to the run/CI artifacts that produced it.

## Deployment owner

`torca-deploy` is the canonical planner/executor for local build, deploy, run,
logs and resume operations.

```powershell
cargo run -p torca-deploy
```

Representative automation:

```powershell
cargo run -p torca-deploy -- status
cargo run -p torca-deploy -- plan --target all --configuration debug
cargo run -p torca-deploy -- build --target windows --configuration debug
cargo run -p torca-deploy -- deploy --target android --device <adb-serial>
cargo run -p torca-deploy -- logs --target all
cargo run -p torca-deploy -- resume
```

The deployer invokes Docker/Cargo/Flutter/ADB through typed process adapters. PowerShell helpers remain compatibility/validation utilities rather than a second deployment architecture.

## Durable deploy state

Deploy runs/checkpoints live under:

```text
.torca/deploy/current.json
.torca/deploy/runs/<run-id>.json
.torca/deploy/runs/<run-id>.events.jsonl
```

A partially completed deployment should be understood through the checkpoint/events and resumed or deliberately restarted. Do not infer success merely because an artifact was built or a device was discovered.

## Provider-aware deployment

Every deployment plan uses Iroh as its sole production communication provider.
Memory is available only as a deterministic test double. Iroh does not require
the removed managed Tor rendezvous service.

Artifact reuse is provider-aware. A build for a different provider must be rejected instead of silently reused.

## Startup readiness

Runtime startup has separate notions of local/application readiness and communication reachability.

Flutter opens the FFI gateway, initializes the Rust application/runtime and sends `flutter_gateway_ready` after it has successfully decoded the initial application response. This prevents host/deployer logic from treating a native actor that merely exists as proof that the Flutter application can use it.

A network provider can still be degraded after local state becomes usable. Local encrypted history should not disappear behind a provider warm-up/error state.

## Lifecycle

Android attaches a runtime lifecycle observer after gateway readiness; Windows initializes desktop lifecycle/tray behavior. Presentation lifecycle changes must not create a second independent native runtime.

Runtime work is driven by durable demand, communication evidence and deadlines. Idle UI polling must not be a correctness dependency for retry, pairing or incoming work.

## Diagnostics surfaces

The gateway exposes structured diagnostics and log-tail requests; the Flutter diagnostics screen presents runtime-safe information. The deployer can collect target logs/diagnostic artifacts for incident/validation work.

Diagnostics may include operational data such as:

- provider/commissioning state;
- build/source metadata;
- connection/transport health;
- queue/retry counters and timing;
- lifecycle/power observations;
- bounded errors/log tails; and
- platform/device identifiers needed to explain a run.

Diagnostics must not intentionally contain message/attachment plaintext, Radio audio, identity private keys, database keys, relationship secrets or reusable pairing capabilities.

## Build/runtime mismatch

Flutter explicitly recognizes native-library symbol/procedure mismatch failures and reports that the installed DLL/SO is from another build. Fix the deployed artifact set; do not mask this state by retrying against an incompatible native library indefinitely.

## Android capture privacy

Strict capture protection is the default. `--privacy allow-capture` is an explicit development choice that changes the Android window capture flag only. Treat logs/screenshots produced in that mode as potentially sensitive test artifacts.

## Reset and destructive actions

Client data reset is a deliberate operation. It may remove identity, encrypted
history and provider cache/secret state.

Never make destructive reset an incidental prerequisite for ordinary rebuild/deploy unless the change being tested requires a clean identity/storage state.

## Soak/incident artifacts

The soak cockpit writes run artifacts below `.torca/soak` by default and can retain fixture metadata for repeatable scenarios. A manifest states what was requested; verdict/report evidence states what actually happened.

When collecting an incident or soak artifact:

1. record the exact source commit/build/provider/platform/device;
2. preserve the first relevant failure rather than repeatedly resetting the environment;
3. collect structured diagnostics/logs without adding user payloads;
4. distinguish environment/setup failure from product/protocol failure; and
5. state whether the result came from a simulated peer, host process or real device.

## Recovery principle

Failures should be explicit, bounded and recoverable where possible:

- unsent durable work remains Rust-owned;
- provider failure does not hand retry ownership to Flutter;
- stale provider/host bridges are cleared across runtime generations;
- pairing sessions expire/fail instead of pretending unavailable bootstrap is healthy; and
- deploy checkpoints support deliberate resume after interrupted host/device operations.

See [`testing.md`](testing.md) for validation levels and [`transport.md`](transport.md) for provider-specific commissioning behavior.
