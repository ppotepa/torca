# Known limitations of the 0.1 source baseline

- Windows and Android are the production composition targets. They use SQLCipher-backed repositories plus platform protected secret storage, but fresh target builds and real-device/runtime validation are still required.
- Windows uses DPAPI-backed secret stores in the native composition; Android uses the protected secret-store bridge supplied by the platform overlay. Platform lifecycle and restore behavior remain validation gates.
- The Tor adapter requires a trusted packaged Tor executable and the complete two-client direct Tor journey is not yet owner-validated against the current composition root.
- Windows tray/single-instance behavior and Android foreground/background ownership remain platform lifecycle validation gates.
- Pairing session state is intentionally ephemeral in the current composition; restart behavior must be explicit and polished in 0.2.
- Relay data is intentionally ephemeral and in memory. Production network hosting, deployment hardening, abuse controls and operational monitoring remain service work.
- The accepted Flutter `pubspec.lock` still needs to be generated and committed from an owner toolchain run.
- GitHub Actions did not execute the final 0.1 validation job because the account was blocked by a billing issue; source-complete does not imply green CI.
- Group conversations, calls, multi-device sync, public discovery and cloud backup are outside 0.1.
- No anonymity guarantee is made against compromised devices or global traffic analysis.
