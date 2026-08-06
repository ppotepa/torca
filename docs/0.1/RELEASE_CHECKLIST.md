# Torca 0.1 release checklist

A test release may be cut only when every item is evidenced:

- [ ] `./scripts/build.ps1 -Target check -CI` passes from a clean checkout.
- [ ] `./scripts/build.ps1 -Target windows -Configuration release` succeeds on Windows.
- [ ] `./scripts/build.ps1 -Target android -Configuration release` succeeds with the Android NDK/toolchain.
- [ ] `torca-native` is composed with production SQLCipher, RustCrypto and protected key stores rather than memory defaults.
- [ ] Windows native runtime uses the current-user protected secret store and restores durable state.
- [ ] Android native runtime uses Android Keystore-backed key handles and restores durable state after process recreation.
- [ ] Platform lifecycle matrix is completed, including Windows tray/single-instance behavior and Android background ownership.
- [ ] Direct Tor exchange succeeds without relay participation after pairing.
- [ ] Interrupted direct-message/outbox delivery recovers without duplicate user-visible messages.
- [ ] Interrupted attachment transfer resumes safely.
- [ ] Diagnostic export is reviewed for secret leakage.
- [ ] Threat model and known limitations are accepted.
- [ ] The committed `Cargo.lock` is the dependency graph used by the release build.
- [ ] `./scripts/deploy.ps1 -Target all` generates release artifacts and `SHA256SUMS.txt`.
- [ ] Android release signing is configured for the intended distribution channel; debug signing is not accepted as a production release.
