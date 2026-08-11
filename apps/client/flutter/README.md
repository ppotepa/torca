# Shared Flutter client

This directory contains the single Torca presentation client used by the supported Windows and Android hosts.

Flutter owns responsive layout, navigation, interaction state and presentation preferences. It communicates with the Rust application through `EngineGateway` and the generated Torca contract. Production startup opens the native runtime; it does not silently replace it with an in-memory business implementation.

Keep platform detection/integration under `lib/platform` and native dynamic-library handling in the FFI gateway boundary. Do not move persistence, Tor, retry/outbox, pairing cryptography or secret ownership into Dart.

Use the root workflows rather than manual per-platform build recipes:

```powershell
./scripts/run.ps1 -Target windows
./scripts/run.ps1 -Target android
./scripts/build.ps1 -Target check
```

See [`../../../ARCHITECTURE.md`](../../../ARCHITECTURE.md) for ownership rules and [`../../../CONTRIBUTING.md`](../../../CONTRIBUTING.md) before changing cross-layer behavior.