# Torca 0.2 final source audit

Date: **2026-08-08**  
Scope: source architecture, product behavior, security boundaries and developer ergonomics.  
Validation mode: **source-only** — no expensive build, platform or E2E execution was performed as part of batches 02-14 through 02-19.

## Result

Torca 0.2 is **SOURCE COMPLETE / VALIDATION OPEN**.

The final source architecture remains one responsive Flutter client over a process-owned Rust runtime. The 0.2 work did not introduce a second mobile/desktop business implementation or a global frontend state framework.

## Final properties

### Runtime and reliability

- Local SQLCipher identity/history can initialize without waiting for Tor bootstrap.
- Tor/network host startup occurs in the background and exposes starting/degraded state rather than owning application startup.
- Blocking native mutations are serialized outside the Flutter UI isolate.
- Durable message delivery/retry remains Rust-owned.
- Read state is marked only for an active/resumed conversation near the latest messages; sending remote Read receipts is a separate user preference.

### History and performance

- Normal application overview snapshots do not load message bodies from the complete history.
- Conversation history uses bounded SQLCipher cursor paging and literal search.
- Conversation summaries/unread/activity are storage-backed projections.
- Production attachment UI projection is storage-owned and does not require the full message list.
- Pairing/runtime identity/session lookups use overview snapshots where full message history is irrelevant.

### Security

- Pairing uses ephemeral key agreement/transcript-bound approval and protected peer secrets.
- SQLCipher data keys and identity/peer secrets remain behind platform-protected storage.
- Safety Number verification is local and tied to the current remote identity.
- A previously verified remote identity change becomes explicit `changed` state and blocks new message/attachment sends until current identity verification.
- Android requests `FLAG_SECURE`; Windows requests `WDA_EXCLUDEFROMCAPTURE` with `WDA_MONITOR` fallback.
- Controlled plaintext attachment exports use a short cleanup namespace/age; explicit Save As destinations stay user-owned.
- Notifications/diagnostics remain designed to avoid message-content disclosure.

Important non-guarantee: 0.2 uses authenticated encryption with a protected pairwise secret but does not implement MLS or Double Ratchet-style message-key evolution. Forward secrecy and post-compromise security are therefore not claimed for message history.

### Developer ergonomics

- Bridge v11 Dart commands expose intent only; compatibility constructor parameters for Flutter-owned IDs/timestamps are removed.
- The public native header no longer exposes historical frontend-owned mutation entrypoints.
- Canonical Rust roots use `runtime.rs` / `migration.rs`; refactor names such as `final_runtime.rs`, `migration_v2.rs`, `migration_v3.rs` and `retry_ffi.rs` are removed.
- Windows application data is version-neutral with a guarded one-time legacy `0.1` migration. Android runtime state uses stable `torca/runtime` with legacy migration.
- `runtime_composition.rs` is stage-oriented/readable instead of minified composition code.
- `tools/build/Torca.SourcePolicy.ps1` prevents the settled source/ABI ownership debt from returning.
- CI is separated into Rust core, Flutter/contract, Windows and Android jobs.

## Source-audit corrections made in 02-19

- Corrected Flutter CI's schema formatting path from `../../crates/...` to `../../../crates/...` when running under `apps/client/flutter`.
- Removed the final Dart Bridge v11 compatibility parameters (`identityIdHex`, optional pairing/message/attachment IDs and `atMs`).
- Updated README, roadmap and contribution guidance from historical 0.1 instructions to the 0.2 track.
- Added a cheap local/CI source-policy gate before expensive validation.

## Validation gates still open

Before calling 0.2 release-validated, execute and record at minimum:

1. CI core + Flutter/contract + Windows + Android jobs from the final `main` commit.
2. Fresh Windows build/run using the packaged Tor binary and relay endpoint.
3. Fresh Android build/install with JNI library, protected secret store and foreground service.
4. Two real clients: create identities, pair via code and QR, explicit approval and reconnect.
5. Bidirectional text/reply, delivery/read receipts, disabled read receipts and duplicate-send protection.
6. Offline queue, process restart, Tor interruption/recovery and peer reconnect.
7. Large history paging/search without loading the complete message history into the overview path.
8. Attachment send/resume/retry/cancel/open/save and controlled temp cleanup.
9. Notification/background/tray/deep-link lifecycle on both supported platforms as applicable.
10. Safety Number QR/manual verification, remote identity-change warning and send block.
11. Upgrade an existing 0.1 Windows/Android data layout and verify history/secrets remain usable.
12. Confirm schema/generated contract equality and native ABI symbols in produced binaries.

Until those gates run, the correct release statement is **source complete, validation open**.
