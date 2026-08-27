# Torca top-down alpha code review

Date: 2026-08-27  
Scope: Rust workspace, Flutter desktop/Android clients, Iroh-only runtime path, provider boundaries, release automation, and battery test tooling.

## Executive result

The codebase is in a substantially improved internal-alpha state: the Rust workspace has 470 passing tests, Iroh and provider-conformance suites pass, lint/format/policy checks pass, and the smart runner produces reproducible CPU/log artifacts. It is not yet a distributable alpha release. The blocking gaps are release signing, red/invalid remote CI evidence, an unresolved Arti/RSA advisory, and the missing successful Android persisted-contact conversation run.

## Version inventory

| Component | Current value | Review |
|---|---:|---|
| Product/Cargo/Flutter | `0.2.0-alpha.0` | Consistent |
| Flutter build / release build | `1` | Increment per artifact |
| Contract schema | `23` | Keep stable for this alpha |
| Storage epoch | `2` | Separate from migration count |
| SQLite migrations | `0016` latest | Document relationship to `schemaVersion=1` |
| Native ABI | `1` | Stable for current host |
| Generic release wire version | `1` | Clarify against relay V4, radio v2, pairing v5, peer 1.0 |

Recommended alpha label: `0.2.0-alpha.0+1`; bump only the build component for rebuilt artifacts until a user-visible release changes the prerelease version.

## Findings by priority

### P0 — release blockers

- Android release configuration still selects the debug signing config. Do not distribute an APK until a protected release keystore and CI signing path exist.
- The latest GitHub Actions run for the current base commit failed immediately with zero executed steps. Rerun and repair workflow infrastructure before claiming a green build.
- `cargo audit` reports `RUSTSEC-2023-0071` (RSA Marvin timing attack) through Arti/Tor; no fixed upgrade is available. Keep Tor non-default for alpha, document the accepted risk, and monitor upstream.
- Full Android conversation soak is incomplete: app readiness passed, but first-time `torca-lab-peer` build/deploy exceeded the safe window. There is no passing persisted-contact message/receipt evidence yet.

### P1 — must close before external alpha

- The Windows security validator aborts on ordinary Cargo compiler stderr under PowerShell `ErrorActionPreference=Stop`; repair stream/process handling and run it in CI.
- CI builds Windows and Android in debug mode and does not publish signed artifacts, provenance, SBOM, or release metadata.
- No root changelog/release procedure or release tag was found; `release/version.json` still contains development fields (`buildId=dev`, `builtAt=not-built`, working-tree source).
- Provider plugin/routing and opaque `ContactRoute` migrations remain transitional in application/native adapters. Add direct boundary tests and remove remaining provider-specific matches where possible.
- Invalid/missing compile-time provider selection can fall back to `Memory` in the application read-model path; production selection should fail closed or make the test-only fallback explicit.
- `docs/STATUS.md` is stale (403 Rust / 89 Flutter versus 470 / 92 locally).

### P2 — quality and maintenance debt

- Dependency tree contains several duplicate major/minor versions; this increases build size and audit surface.
- `bincode` and `paste` are reported unmaintained by audit policy; plan replacement or document ownership.
- Windows resource metadata has a `1.0.0` fallback if Flutter version macros are absent; assert packaged artifact metadata.
- Internal Flutter package versions (`0.1.0`) differ from the app prerelease version; document whether this is intentional.

## Positive evidence

- `cargo test --workspace --all-targets --locked`: 470 passed.
- Iroh transport: 20 passed; provider conformance: 4 passed; soak: 25 passed; targeted security-sensitive crates: 49 passed.
- Clippy with `-D warnings`, rustfmt check, Flutter analyze/tests, architecture/policy validation, and Iroh/WebRTC/Tor provider isolation all pass locally.
- Android hardening is present: backup disabled, cleartext disabled, non-exported foreground service, screenshot protection, and protected secret storage.
- Iroh idle `always` soak measured median CPU 0%, P95 1% on the tested emulator; startup smoke is burstier (median 8%, P95 11.5%) and should not be interpreted as steady-state battery use.
- Security documentation correctly states that no independent audit, forward secrecy, or post-compromise security is claimed.

## Acceptance gate

Before tagging the alpha: obtain a green remote workflow, configure release signing, repair the security gate, prebuild the lab peer, complete Android pairing/persistence/restart/receipt/attachment and route-stale/refresh scenarios, run at least one two-device Iroh test, update status/release notes, and publish exact artifact hashes with the chosen version metadata.

