# Torca alpha handoff

Date: 2026-08-27  
Branch: `main`  
Target version: `0.2.0-alpha.0+1` (product/core/release metadata)

## What is included

- Iroh route-staleness guards and factory-created peer transport coverage.
- Iroh peer-link handshake, authentication, ACK/replay-waker handling, receipts, and persisted-contact paths.
- Durable reconnect demand with explicit reasons (`PreferredDialer`, `Recovery`, `DurableDemand`) and correct dialer-election behavior.
- Pairing maintenance reporting and one-shot `prime_contact` after durable persistence.
- Peer pipeline diagnostics and connection-to-availability projection.
- Provider API/plugin composition, provider routing ownership, provider isolation checks, and memory/Iroh conformance tests.
- Opaque provider contact routes with legacy Tor/SQLite compatibility handling.
- Android foreground-service lifecycle hardening, event-driven waits, secret-store changes, and desktop lifecycle fixes.
- Avatar spritesheet animation support and avatar asset validation.
- Smart Android/Iroh background runner with screen-flow readiness, bounded ADB operations, screen-off verification, log capture, and realistic soak bot support.
- Battery/CPU measurement scripts and validation reports under `docs/validation/`.
- CI gates for provider conformance, Iroh/Tor transport tests, clippy, and avatar assets.

## Validation completed

- `cargo test --workspace --all-targets --locked`: 470 passed.
- `torca-transport-iroh`: 20 passed.
- `torca-provider-conformance`: 4 passed.
- `torca-soak`: 25 passed.
- Security-sensitive targeted crates: 49 passed.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed.
- `cargo fmt --all -- --check`: passed.
- Flutter analyze: passed; Flutter tests: 92 passed.
- Architecture/policy validation: passed.
- Provider isolation (`iroh`, `webrtc`, `tor`): passed.
- Iroh `always` idle soak: 3 x 60 seconds passed, median CPU 0%, P95 1% on the tested emulator.
- Smart startup smoke: passed readiness and screen-off checks; median CPU 8%, P95 11.5% for the short startup window.

## Not complete / known blockers

1. Full Android conversation soak did not reach a passing end-to-end result. App readiness passed, but the first-time `torca-lab-peer` build/deploy exceeded the safe run window and was stopped. No successful persisted-contact message/receipt run is claimed.
2. No two-physical-device Iroh migration/restart run has been completed. Emulator results are not a substitute for Wi-Fi/LTE or radio measurements.
3. Android release signing still uses the debug signing configuration. A production keystore and CI secret flow are required before distributing an alpha APK.
4. The latest GitHub Actions run at the pushed base commit failed immediately with zero executed job steps; remote CI is therefore not evidence of a green release.
5. `cargo audit` reports `rsa` Marvin timing vulnerability (`RUSTSEC-2023-0071`) through Arti/Tor, with no available fixed upgrade. This needs an explicit alpha risk decision and upstream monitoring.
6. `scripts/Validate-TorcaSecurity.ps1` is not Windows-safe: normal Cargo compiler stderr is treated as a terminating `NativeCommandError`. Fix the process/error-stream handling before making it a release gate.
7. Provider metadata and `ContactRoute` migration remain transitional in a few application/native adapters; direct tests for the final plugin/routing boundary should be added before calling the architecture complete.
8. `docs/STATUS.md` test counts/date are stale (it reports 403 Rust and 89 Flutter tests).

## Release decision

The repository is technically testable and the Iroh core paths are substantially implemented, but this is not ready for a public alpha release until items 1–6 are either fixed or explicitly accepted by the release owner. The safest current label is an internal development alpha candidate, not a signed distributable release.

## Next actions

1. Prebuild/cache `torca-lab-peer`, then rerun the smart runner with pairing, persisted contact, message A→B/B→A, receipt, attachment, restart, and route-stale/refresh scenarios.
2. Configure release signing for Android and add signed artifact/provenance checks to CI; add Windows release artifact version assertions.
3. Repair the Windows security validator and run dependency/security checks in CI.
4. Decide and document the version contract: keep `0.2.0-alpha.0+1`, define build-number increments, and clarify `contractSchema=23`, `storageEpoch=2`, `schemaVersion`, generic `wireVersion`, and protocol-specific versions.
5. Update `docs/STATUS.md`, add release notes/changelog, and record the exact release commit and artifacts.

Detailed evidence is in:

- `docs/validation/ALPHA_TOP_DOWN_CODE_REVIEW_2026-08-27.md`
- `docs/validation/IROH_SMART_RUNNER_ANALYSIS_2026-08-27.md`
- `docs/validation/IROH_BACKGROUND_TEST_REPORT_2026-08-27.md`
- `docs/validation/PLAN_IMPLEMENTATION_AUDIT_2026-08-27.md`
- `docs/validation/ENERGY_AUDIT_2026-08-27.md`

