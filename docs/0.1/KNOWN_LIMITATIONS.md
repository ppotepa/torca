# Known limitations of the 0.1 source baseline

- Production cryptography is not yet wired; the deterministic provider is test-only.
- SQLCipher schemas and ports exist, but the concrete embedded SQLCipher driver is not wired.
- Windows and Android host contracts exist, but generated runner projects and native bridge artifacts require owner builds.
- Tor adapter requires a separately supplied trusted Tor executable.
- Relay semantics are implemented in memory; production HTTP/WebSocket hosting and deployment hardening remain composition work.
- Group conversations, calls, multi-device sync, public discovery and cloud backup are outside 0.1.
- No anonymity guarantee is made against compromised devices or global traffic analysis.
