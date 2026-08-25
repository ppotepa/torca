# `torca-deploy`

`torca-deploy` is the canonical typed build/run/deploy/log workflow for Torca. It owns one planner, durable deployment checkpoints and both interactive (Ratatui) and CLI interfaces over the same executor.

```powershell
cargo run -p torca-deploy
cargo run -p torca-deploy -- status
cargo run -p torca-deploy -- plan --dry-run
cargo run -p torca-deploy -- rebuild --target all
cargo run -p torca-deploy -- resume
cargo run -p torca-deploy -- logs --target all --dry-run
cargo run -p torca-deploy -- build --target windows --configuration debug
cargo run -p torca-deploy -- deploy --target android --device <adb-serial>
cargo run -p torca-deploy -- run --target android --privacy allow-capture
```

No command arguments opens the interactive wizard. CLI commands are intended for CI/repeatable automation. `--dry-run` validates/prints supported plans without performing the external build/device actions.

## Provider-aware plans

Every client plan selects one communication provider. **Tor** is the default and **Iroh** is also exposed by the normal deployment selector. Provider requirements come from the shared `torca-transport-api` deployment profile rather than being inferred by the deployer.

- Tor can require managed rendezvous service build/maintenance/endpoint configuration.
- Iroh is direct and does not inherit Tor relay/onion configuration.
- WebRTC and memory remain hidden from normal deploy selection.

Provider selection and provider endpoint requirements are recorded in deployment/artifact metadata. Reusing an artifact built for another provider is rejected.

Compatibility names/flags related to relay/onion deployment may remain readable while old checkpoints/automation migrate, but new documentation/code should use provider-neutral plan terminology.

## Execution/checkpoints

Rust owns orchestration and invokes Docker, Cargo, Flutter and ADB through typed process adapters; the canonical deploy path does not invoke PowerShell as its executor.

Each run is saved below:

```text
.torca/deploy/current.json
.torca/deploy/runs/<run-id>.json
.torca/deploy/runs/<run-id>.events.jsonl
```

Checkpoints make interruption/resume explicit and let diagnostics explain which stage actually completed.

## Android targeting/privacy

`--device <adb-serial>` restricts discovery, ABI selection, reset, install and launch to one exact Android device.

Strict capture privacy is the default and keeps Android `FLAG_SECURE` enabled. `--privacy allow-capture` is an explicit local-development override for screenshots/screen recording; it does not alter application-layer encryption or provider transport security.

## Destructive operations

Client data reset and provider-service maintenance/rotation are explicit plan choices. A Tor rendezvous/onion rotation can require coordinated service/client artifact changes; the planner guards unsafe combinations. Direct providers must not be forced through Tor service maintenance solely because old deploy flows used a relay.

See [`../../docs/development.md`](../../docs/development.md), [`../../docs/operations.md`](../../docs/operations.md) and [`../../docs/transport.md`](../../docs/transport.md).