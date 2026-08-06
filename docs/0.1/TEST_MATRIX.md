# Torca 0.1 test matrix

## Automated workspace tests

- foundation ID/time/command/event/error/cancellation contracts;
- wire framing partial, concatenated, oversized and malformed cases;
- identity create-once, profile update and rotation;
- migration ordering and rollback behavior;
- pairing expiry and approval transitions;
- message/receipt monotonic transitions;
- outbox claim/reschedule/dedup semantics;
- SQLCipher restart, wrong-key rejection and stale-outbox recovery;
- Windows DPAPI secret round-trip/delete;
- peer handshake/challenge/ack/reconnect behavior;
- encrypted attachment round-trip and tamper rejection;
- primary cross-crate journey in `tests/torca-integration`;
- `torca-native` snapshot shape and secret-redaction boundary;
- Flutter identity setup and pairing navigation;
- responsive wide layout reusing the shared `ConversationPane`;
- explicit native-runtime startup failure without memory fallback.

## Build workflow checks

CI and local correctness use the same root entrypoint:

```powershell
./scripts/build.ps1 -Target check -CI
```

The gate includes release metadata, architecture boundaries, formatting/codegen, locked Cargo metadata after resolution, Rust check/Clippy/tests, Flutter dependency resolution, Flutter analysis and Flutter tests.

## Owner/platform matrix

| Journey | Windows | Android |
|---|---|---|
| Fresh `build.ps1` platform bootstrap | Pending after one-client refactor | Pending after one-client refactor |
| Shared native library loads through Dart FFI | Pending | Pending |
| Fresh identity creation with production persistence | Pending | Pending |
| Pair two clean installations | Pending | Pending |
| Direct Tor text exchange | Pending | Pending |
| Kill/restart during queued send | Pending | Pending |
| Read receipt and duplicate replay | Pending | Pending |
| Encrypted image interruption/recovery | Pending | Pending |
| Close-to-tray/background lifecycle | Pending | Pending |
| Redacted diagnostic export | Pending | Pending |
| `deploy.ps1` release artifact/checksum output | Pending | Pending |

Record exact devices, OS versions, commands and results in `0.1_PROGRESS.md`.
