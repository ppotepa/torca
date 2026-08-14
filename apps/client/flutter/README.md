# Shared Flutter client

This directory contains Torca's single presentation client for the supported Windows and Android hosts.

Flutter owns responsive layout, navigation, transient interaction state and presentation preferences. It communicates with the Rust application through `EngineGateway` and the generated Torca contract. Production startup opens the native runtime; it must not silently substitute an in-memory business implementation when native startup fails.

Keep platform detection/integration under the intended platform boundary and native dynamic-library handling in the FFI gateway. Do not move persistence, identifiers, durable retry/outbox policy, pairing cryptography, peer secrets, Tor lifecycle or security policy into Dart.

## Development

Use the repository-level Rust workflow rather than maintaining per-platform build recipes here:

```powershell
cargo run -p torca-deploy
```

For source-level Flutter checks:

```powershell
flutter pub get
flutter analyze
flutter test
```

The repository CI also checks Dart formatting and generated contract drift. Platform artifact builds should use `torca-deploy` so they exercise the same native composition/packaging path used elsewhere in the project.

## Generated contract

The Dart DTO/state surface under `lib/generated` is generated from the canonical Torca contract. Change the canonical schema/generator input and regenerate; do not hand-maintain a divergent Dart protocol model.

Flutter should render typed application state and submit user intent. It must not infer durable use-case success by diffing unrelated snapshots or reconstruct versioned wire formats such as pairing URIs.

## Platform behavior

Windows and Android may differ for genuine OS capabilities such as lifecycle, protected secret stores, notifications, deep links, screen-capture controls and microphone permission. Product workflows remain shared.

Android screen capture is blocked by default by the platform host. The deployment tool can explicitly allow capture for a local test run; that option changes the OS window flag only and does not weaken Torca's message encryption or transport rules.

## Further reading

- [`../../../ARCHITECTURE.md`](../../../ARCHITECTURE.md) — ownership and dependency rules.
- [`../../../CONTRIBUTING.md`](../../../CONTRIBUTING.md) — contributor workflow and validation policy.
- [`../../../docs/STATUS.md`](../../../docs/STATUS.md) — current maturity and validation state.
- [`../../../SECURITY.md`](../../../SECURITY.md) — security guarantees and non-guarantees.