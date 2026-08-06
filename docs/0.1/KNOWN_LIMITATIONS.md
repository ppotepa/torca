# Known limitations of the 0.1 source baseline

- Production crypto algorithms and SQLCipher adapters exist and are owner-tested, but the new `torca-native` constructor still starts `ClientEngine::default()`; production repositories/key providers must be injected before release.
- Windows DPAPI protected-secret storage exists, but has not yet been selected by the shared native composition.
- Android Keystore protected-secret storage is preserved as the Android platform overlay; the narrow JNI bridge required for Rust key-handle operations is not yet composed.
- Windows and Android are now targets of the single Flutter client and their standard platform scaffolds are bootstrapped by `build/run/deploy`; fresh target builds after this refactor still require owner validation.
- Windows tray/single-instance behavior and Android foreground/background ownership remain platform lifecycle gates.
- The Tor adapter requires a separately supplied trusted Tor executable/package and the complete two-client direct Tor journey is not yet owner-validated against the new composition root.
- Relay domain semantics are implemented in memory; production network hosting and deployment hardening remain composition work.
- `Cargo.lock` must be regenerated/committed from the owner toolchain after the native crate addition so release builds use a reviewed dependency graph.
- Group conversations, calls, multi-device sync, public discovery and cloud backup are outside 0.1.
- No anonymity guarantee is made against compromised devices or global traffic analysis.
