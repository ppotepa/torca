# Production crypto and SQLCipher adapters

## Cryptography

`torca-crypto::RustCryptoProvider` is the production algorithm adapter:

- Ed25519 signing and strict verification;
- XChaCha20-Poly1305 authenticated encryption with 24-byte nonces;
- operating-system CSPRNG through `rand_core::OsRng`;
- RFC 8032 Ed25519 known-answer test;
- XChaCha20-Poly1305 draft known-answer test;
- ciphertext modification rejection test;
- redacted secret diagnostics and best-effort zeroing.

The deterministic provider remains test-only.

Still required before release:

- Windows protected-key implementation;
- Android Keystore implementation;
- identity key-handle composition using those stores;
- local execution of all known-answer and negative tests.

## SQLCipher

`torca-storage-sqlite` now uses `rusqlite` with `bundled-sqlcipher-vendored-openssl` and provides:

- raw 256-bit database key handling with redacted diagnostics;
- SQLCipher version verification during open;
- database-key verification before migrations;
- real transaction-backed `StorageBackend`;
- ordered embedded migrations;
- concrete identity row mapping;
- transactional message plus outbox insertion;
- atomic claim through `UPDATE ... RETURNING`;
- reschedule, complete, dead-letter and stale-claim recovery;
- inbound envelope deduplication;
- encrypted in-memory integration tests.

Still required before release:

- platform-protected database-key provisioning;
- file-backed restart and wrong-key tests;
- process-interruption tests at transaction boundaries;
- owner-generated and committed `Cargo.lock`;
- full local validation on Windows and Android build hosts.

## Dependency lock workflow

The connector environment cannot execute Cargo. After pulling these changes, run:

```powershell
./scripts/refresh-lock.ps1
./scripts/format.ps1
./scripts/validate.ps1 -SkipFlutter
```

Review and commit the generated `Cargo.lock`. Release validation should return to locked dependency mode after the lockfile is regenerated and validated.
