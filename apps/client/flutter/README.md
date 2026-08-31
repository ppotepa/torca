# Shared Flutter client

This directory contains Torca's single presentation client for the supported Windows and Android hosts.

Flutter owns responsive layout, navigation, transient interaction state, localization/theme preferences and translation of user interactions into typed `EngineGateway` requests. Production startup opens the native Rust runtime; it must not silently substitute an in-memory business implementation when native startup fails.

Durable state, identifiers, pairing completion, retry/outbox policy, cryptography, peer secrets, provider lifecycle/routing and security policy stay in Rust.

## Development

Use the repository-level deployment workflow for normal build/run/deploy work:

```powershell
cargo run -p torca-deploy
```

For Flutter source checks:

```powershell
flutter pub get
flutter analyze
flutter test
```

Supported platform artifact builds should use the repository deployment path so the native composition/packaging is exercised consistently.

## Generated contract

`lib/generated` is generated from the canonical Torca contract. Change the canonical schema/generator input and regenerate/check it; do not hand-maintain a divergent Dart business/protocol model.

Flutter should render typed projections and submit user intent. It must not infer durable success from unrelated snapshot changes or reconstruct versioned invitation/wire formats itself.

## Platform behavior

Windows and Android may differ for genuine OS capabilities such as lifecycle, protected secret stores, notifications, deep links, secure-window behavior, microphone permission and installation/device integration. Product workflows remain shared.

Android screen capture is blocked by default. The deployment tool can explicitly allow capture for a local test run; this changes the OS window flag only and does not weaken Torca message encryption or Iroh transport rules.

## Communication provider

Iroh is the sole production provider and is composed below the Flutter/native boundary. Flutter can render presentation-safe provider/network status and capabilities, but it does not select/implement providers or own route/reconnect policy.

Memory is test-only. Tor/WebRTC are not active product providers.

## Further reading

- [`../../../ARCHITECTURE.md`](../../../ARCHITECTURE.md) — ownership and dependency rules.
- [`../../../docs/APP-FLOWS.md`](../../../docs/APP-FLOWS.md) — current application journeys.
- [`../../../docs/TRANSPORT.md`](../../../docs/TRANSPORT.md) — provider/Iroh boundary.
- [`../../../docs/DEVELOPMENT.md`](../../../docs/DEVELOPMENT.md) — repository workflow.
- [`../../../docs/TESTING.md`](../../../docs/TESTING.md) — validation/evidence rules.
- [`../../../SECURITY.md`](../../../SECURITY.md) — security guarantees and limits.
