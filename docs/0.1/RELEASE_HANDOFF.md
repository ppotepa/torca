# Torca 0.1 release handoff

This handoff starts **after source implementation and final cleanup**. The remaining work is owner/platform validation and release packaging evidence, not another architectural refactor.

## Source state

The 0.1 source architecture is complete around:

```text
Flutter
 -> Bridge v4
 -> torca-native process runtime
 -> RuntimeHost
    -> PairingDriver
    -> OwnedTorDriver
    -> TorcaCommunicationDriver
       -> SharedPeerLink
       -> durable text/control delivery
       -> inbound exactly-once
       -> read receipts
       -> attachment transfer
```

Superseded peer/native/bridge implementations have been removed. SQLCipher migrations are 1–14. Operational SQL is external/parameterized. Windows and Android use the same Flutter/Rust application code with platform-only lifecycle/key-store hosts.

## Do not change before first validation

Do not perform speculative refactors before observing the first real failure. In particular do not:

- add a second delivery queue;
- move pairing/crypto state into Flutter;
- add plaintext/private-key fallbacks;
- bypass SQLCipher/protected secret storage;
- restore the removed peer-runtime/peer-delivery crates;
- introduce platform-specific business screens;
- embed operational SQL in Rust source.

## Required release inputs

Before real run/deploy, provide the release inputs expected by the build tooling:

1. packaged Tor runtime for the target platform;
2. configured Tor relay onion endpoint;
3. platform signing material when producing distributable release artifacts.

Production runtime intentionally does not discover arbitrary Tor installations through PATH or Tor Browser.

## Windows validation order

Run:

```powershell
./scripts/run.ps1 -Target windows
```

Validate in this order:

1. Rust/native compilation completes;
2. Flutter compilation completes;
3. `torca_bridge.dll` loads;
4. contract v4 matches generated Dart contract;
5. first snapshot loads;
6. identity/database protected stores initialize;
7. packaged Tor starts;
8. Tor state reaches Ready and onion endpoint appears;
9. app close hides to tray rather than exiting;
10. tray Show restores the window;
11. second process launch activates the first instance;
12. tray Quit calls process shutdown and exits Tor/runtime;
13. restart preserves identity, contacts and history.

Record only the first concrete failure. Fix it as one `BUG-*` commit and rerun from the same checkpoint.

## Android validation order

Use the normal build/deploy scripts for the Android target, then validate:

1. native library loads;
2. contract v4 snapshot loads;
3. Android Keystore namespaces for database/identity/peer secrets initialize;
4. foreground service starts with `remoteMessaging`;
5. packaged Tor starts and reaches Ready;
6. Activity recreation does not destroy RuntimeHost/Tor;
7. background/foreground does not lose pending delivery state;
8. true process kill followed by restart recovers SQLCipher/outbox state;
9. service shutdown stops the process runtime intentionally.

## Two-client pairing validation

Use two clean clients A and B.

1. A creates invitation.
2. Confirm code/QR comes from Rust and has a short TTL.
3. B joins using code or QR.
4. Both clients display the verified peer proposal state.
5. Both explicitly Approve.
6. Verify pairing reaches Completed on both sides.
7. Verify relay pairing state is cleaned up.
8. Restart both clients.
9. Verify Contact + Conversation remain and no re-pairing is required.

Expected security invariants:

- relay never receives long-term private keys;
- PairingOffer/Approval/Completion payloads are encrypted/validated;
- pairwise peer secret is referenced by protected handle, not stored in SQL or Flutter;
- remote capability must be validated during authenticated peer handshake.

## Text messaging validation

1. Send A -> B.
2. Verify B gets exactly one inbound message.
3. Verify A reaches Sent/Delivered.
4. Open B conversation and verify A reaches Read.
5. Disconnect Tor/network during a send.
6. Verify message remains durable and retry occurs after reconnect.
7. Force ACK loss/replay and confirm B does not persist a second copy.
8. Confirm Duplicate ACK completes the sender durable job.
9. Restart sender while an outbox row is claimed; verify stale claim recovery.

## Attachment validation

1. Select a small file.
2. Confirm UI shows persistent progress.
3. Interrupt network during transfer.
4. Restart one or both clients.
5. Resume transfer from durable offset.
6. Confirm one final verified file and correct SHA-256 completion behavior.
7. Test Retry and Cancel.
8. Test preparation failure (missing/unreadable source path) and confirm the associated outbound message becomes Failed and its text outbox row is dead-lettered.

## Diagnostics validation

Diagnostics may expose component/state/code/timestamps but must not contain:

- plaintext message bodies beyond explicit UI projections;
- database keys;
- identity private keys;
- pairwise peer secret bytes;
- pairing capability/token values.

Test diagnostics during Tor failure, reconnect, pairing and delivery failure.

## Persistence validation

Validate SQLCipher with real process restarts:

- correct key opens database;
- wrong key fails;
- migrations 1–14 apply in order;
- contacts/conversations/messages/receipts/attachments survive restart;
- control outbox survives restart;
- stale text/control claims recover;
- attachment progress survives restart.

## Cargo.lock note

The workspace path-package graph was reconciled manually because the execution environment could not reach the registry and the one-shot GitHub Actions runner was blocked before startup by the repository account billing state. Existing registry package versions/checksums were preserved and verified after `BUG-037`.

When normal network/runner access is available, run:

```powershell
cargo generate-lockfile
```

or the equivalent build/check workflow and inspect any resulting lockfile diff before accepting it. Do not silently accept registry version/checksum drift.

## Release gate closure

Close gates only with evidence:

- **GATE-001** protected crypto/key lifecycle;
- **GATE-002** SQLCipher persistence/recovery;
- **GATE-003** Bridge/C ABI v4 runtime;
- **GATE-004** Windows lifecycle;
- **GATE-005** Android lifecycle;
- **GATE-006** relay/Tor/P2P/text/receipts/attachments E2E;
- **GATE-007** release artifacts, signing, checksums and platform matrix.

## Failure handling rule

For every validation failure:

1. capture the first real error/log;
2. identify the owning layer;
3. make one focused fix;
4. commit as one `BUG-*` change;
5. rerun the same validation checkpoint;
6. update `0.1_PROGRESS.md` only after the result is known.

Do not batch unrelated fixes into one commit.

## Release-ready definition

Torca 0.1 is release-ready only when both platform hosts and the full two-client scenario pass:

```text
pair -> approve -> persist relationship
 -> restart
 -> direct Tor text
 -> interruption/retry/dedup
 -> Delivered/Read
 -> attachment interruption/resume
 -> lifecycle/background/tray behavior
 -> clean intentional shutdown
```

Source completion alone is not release evidence.
