# Platform libraries

Platform libraries expose stable application contracts to Flutter and native hosts.

Planned component:

- [`torca-bridge`](torca-bridge/README.md) — generated Rust/Dart commands, results and snapshots.

Platform code is an adapter. It must not reimplement pairing, messaging, retries or persistence.
