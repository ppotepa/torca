# Torca 0.1 test matrix

## Automated workspace tests

- foundation ID/time/command/event/error/cancellation contracts;
- wire framing partial, concatenated, oversized and malformed cases;
- identity create-once, profile update and rotation;
- migration ordering and rollback behavior;
- pairing expiry and approval transitions;
- message/receipt monotonic transitions;
- outbox claim/reschedule/dedup semantics;
- peer handshake/challenge/ack/reconnect behavior;
- encrypted attachment round-trip and tamper rejection;
- primary cross-crate journey in `tests/torca-integration`;
- Flutter identity, pairing navigation and conversation rendering.

## Owner/platform matrix

| Journey | Windows | Android |
|---|---|---|
| Fresh identity creation | Pending | Pending |
| Pair two clean installations | Pending | Pending |
| Direct Tor text exchange | Pending | Pending |
| Kill/restart during queued send | Pending | Pending |
| Read receipt and duplicate replay | Pending | Pending |
| Encrypted image interruption/recovery | Pending | Pending |
| Close-to-tray/background lifecycle | Pending | Pending |
| Redacted diagnostic export | Pending | Pending |

Record exact devices, OS versions and results in `0.1_PROGRESS.md`.
