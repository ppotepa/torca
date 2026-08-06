# Production adapters

## Cryptography

`torca-crypto::RustCryptoProvider` provides:

- Ed25519 signing and strict verification;
- XChaCha20-Poly1305 authenticated encryption with 24-byte nonces;
- operating-system CSPRNG through `rand_core::OsRng`;
- RFC 8032 Ed25519 known-answer test;
- XChaCha20-Poly1305 known-answer and tamper tests;
- redacted secret diagnostics and best-effort zeroing.

`ManagedIdentityKeys` composes those algorithms with a `ProtectedSecretStore` and exposes only opaque `KeyId` handles to the identity domain.

Platform source now includes:

- `torca-platform-windows::DpapiFileSecretStore`, using current-user DPAPI with UI forbidden, atomic files, buffer clearing and `LocalFree`;
- `AndroidKeystoreSecretStore`, using a non-exportable Android Keystore AES-256-GCM key, random IV and `KeyId` as AAD.

The deterministic provider and in-memory secret store remain test-only.

Still required before release:

- wire the Windows DPAPI store into the Windows production composition;
- wire the Android Keystore store through the native Rust adapter;
- use the same protected stores to provision the SQLCipher database key;
- execute DPAPI, Keystore and known-answer tests on target devices.

## SQLCipher

`torca-storage-sqlite` uses `rusqlite` with `bundled-sqlcipher-vendored-openssl` and provides:

- raw 256-bit database key handling with redacted diagnostics;
- SQLCipher version and key verification during open;
- real transaction-backed migration backend;
- ordered embedded migrations;
- concrete identity row mapping;
- transactional message plus outbox insertion;
- atomic claim through `UPDATE ... RETURNING`;
- reschedule, complete, dead-letter and stale-claim recovery;
- inbound envelope deduplication;
- encrypted in-memory integration tests;
- file-backed identity restart test;
- wrong-key rejection test;
- claimed-outbox restart/recovery test.

Still required before release:

- platform-protected database-key provisioning;
- process-kill tests at transaction boundaries;
- owner-generated and committed `Cargo.lock`;
- full local validation on Windows and Android build hosts.

## Native Flutter transport

Flutter now defaults to `MethodChannelEngineGateway` on `torca.engine.v1`. The in-memory gateway is available only through an explicit development `dart-define`.

Implemented host source:

- Android `TorcaEngineChannel` method handler;
- Windows `TorcaEngineChannel` C++ method handler;
- identical version and map contract on both platforms;
- architecture checks prohibiting the memory gateway in release manifests.

Still required before release:

- a concrete native `NativeEngine` adapter that owns the Rust `EngineBridge`;
- production ClientEngine composition using SQLCipher, RustCrypto and protected keys instead of in-memory defaults;
- generated Flutter runners and platform lifecycle tests.

See [`NATIVE_CHANNEL.md`](NATIVE_CHANNEL.md).

## Dependency lock workflow

The connector environment cannot execute Cargo. After pulling these changes, run:

```powershell
./scripts/refresh-lock.ps1
./scripts/format.ps1
./scripts/validate.ps1 -SkipFlutter
```

Review and commit the generated `Cargo.lock`. Release validation should return to locked dependency mode after the dependency graph passes locally.
