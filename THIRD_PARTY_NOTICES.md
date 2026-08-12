# Third-party notices

Torca incorporates open-source software. Each dependency remains copyrighted by
its respective authors and is distributed under its own license. The dependency
manifests and lock files are the authoritative version inventory for a build.

Principal components include:

| Component | Purpose | License family |
| --- | --- | --- |
| Flutter and Dart | Cross-platform client UI and runtime | BSD-3-Clause |
| Arti and the Tor crates | Embedded Tor client and onion services | MIT OR Apache-2.0 |
| SQLCipher | Encrypted SQLite storage | BSD-style |
| rusqlite | Rust SQLite integration | MIT |
| serde / serde_json | Rust serialization | MIT OR Apache-2.0 |
| ring and RustCrypto crates | Cryptographic primitives | ISC/MIT/OpenSSL-style and MIT OR Apache-2.0 |
| mobile_scanner | QR-code capture | MIT |
| qr_flutter | QR-code rendering | BSD-3-Clause |
| shared_preferences | Device preference integration | BSD-3-Clause |
| window_manager and tray_manager | Desktop window and tray integration | MIT |

Transitive dependencies may carry additional compatible notices. Release
packaging must preserve the license material supplied by resolved Cargo and
Flutter packages and must regenerate its complete attribution set whenever
`Cargo.lock` or `apps/client/flutter/pubspec.lock` changes.

Torca is not affiliated with or endorsed by the authors of these components.
Names and trademarks belong to their respective owners.
