# Torca implementation order

This repository implements one Windows/Android baseline: one Flutter presentation, one process Rust
runtime, one canonical contract, and thin operating-system adapters.

## Ordered work

1. Establish source policy and the baseline storage epoch.
2. Generate the language-neutral request/response contract and verify drift.
3. Start one bounded single-writer `TorcaRuntime` and process registry.
4. Compose Windows and Android through `PlatformServices`.
5. Keep all Arti integration inside `torca-tor`.
6. Bootstrap storage, identity, embedded Tor, onion service and relay probe.
7. Expose only `profile.set` for profile creation/update.
8. Use one Dart worker and the generic invoke ABI.
9. Add lifecycle, cursor-addressed notifications and Rust deep-link parsing.
10. Verify manifests, artifact hashes, controlled reset and platform E2E journeys.

Source checks and platform validation are separate gates; neither substitutes for the other.
