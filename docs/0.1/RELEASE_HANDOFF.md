# Torca 0.1 release handoff

This handoff starts **after source implementation and static cleanup**. The remaining release work is owner/platform/end-to-end validation and release packaging evidence, not another architectural refactor.

## Source state

```text
Flutter
 -> canonical FFI gateway
 -> Bridge/C ABI v9
 -> process-owned NativeEngineRuntime
 -> RuntimeHost
    -> PairingDriver / Pairing Protocol v2
    -> OwnedTorDriver
    -> TorcaCommunicationDriver
       -> ActiveRelationshipStore / SharedPeerLink
       -> durable text/control delivery
       -> inbound exactly-once
       -> read receipts
       -> relationship administration
       -> attachment transfer + verified export
```

SQLCipher migrations are **1–17**. Operational SQL is external and parameterized. Windows and Android use the same Flutter/Rust application code with platform-only lifecycle/key-store/notification hosts.

## Do not change before first validation

Do not perform speculative refactors before observing the first real failure. In particular do not:

- add a second delivery queue;
- move pairing/crypto state into Flutter;
- add plaintext/private-key fallbacks;
- bypass SQLCipher/protected secret storage;
- restore removed superseded runtime/gateway files;
- introduce platform-specific business screens;
- embed operational SQL in Rust source;
- turn the rendezvous relay into a message mailbox.

## Required release inputs

Before real run/deploy, provide:

1. packaged Tor runtime for the target platform;
2. configured relay onion endpoint;
3. platform signing material for distributable artifacts.

Production runtime intentionally does not discover arbitrary Tor installs through PATH or Tor Browser.

## Windows validation order

Run the normal Windows entrypoint from a clean checkout and validate:

1. Rust/native compilation completes;
2. Flutter dependency resolution/compilation completes;
3. `torca_bridge.dll` loads;
4. Bridge contract **v9** matches generated Dart source;
5. SQLCipher migrations **1–17** apply;
6. identity/database/peer protected stores initialize;
7. packaged Tor starts and reaches Ready;
8. onion endpoint appears;
9. pairing code/QR and `torca://pair` cold start work;
10. second `torca://pair` process hands the link to the existing instance before creating Engine/Tor;
11. two-client pairing reaches Completed and persists signed display name;
12. Safety Numbers match on both clients;
13. direct text/Reply/Retry Now work;
14. Delivered/Read work;
15. Block prevents both outgoing reconnect and incoming authentication; Unblock restores communication;
16. Clear History and Remove Contact perform complete local cleanup;
17. attachment transfer survives interruption and verified Open/Save As works;
18. close hides to tray, Show restores and second normal launch activates the first instance;
19. local message notification click routes to the correct conversation;
20. tray Quit calls explicit process shutdown and exits Tor/runtime;
21. restart preserves identity, remaining contacts and history.

Record the first concrete failure only. Fix it as one focused `BUG-*` commit and rerun from the same checkpoint.

## Android validation order

Use the normal Android build/deploy scripts, then validate:

1. native library loads and Bridge v9 snapshot decodes;
2. Keystore database/identity/peer namespaces initialize;
3. migrations 1–17 apply;
4. foreground service starts using `remoteMessaging` and owns the process RuntimeHost;
5. packaged Tor starts and reaches Ready;
6. Activity recreation does not destroy RuntimeHost/Tor;
7. background/foreground does not lose pending delivery state;
8. background inbound message creates a **separate** Private messages notification in addition to the ongoing foreground-service notification;
9. the private-message notification contains metadata only, not message plaintext;
10. tapping the notification returns to the existing `singleTask` Activity and opens the correct conversation;
11. `torca://pair` VIEW/BROWSABLE flow works on cold and running app paths;
12. process kill followed by restart recovers SQLCipher/outbox state;
13. intentional service/process shutdown stops RuntimeHost/Tor.

## Pairing Protocol v2 validation

Use clean clients A and B.

1. A creates an invitation.
2. Confirm code/TTL originate in Rust.
3. B joins by code/QR/deep link.
4. Both receive encrypted `PairingOffer` v2.
5. Verify display name, public identity, route and capability are covered by the signed transcript.
6. Both explicitly approve.
7. Completion creates one Contact + Conversation + PeerCredential per client.
8. The signed peer display name appears as the initial contact name.
9. Attempt to pair the same remote identity again while the contact exists; it must be rejected.
10. Remove the contact explicitly and confirm a new pairing is then possible.
11. Restart both clients and verify relationship persistence without relay state.

## Messaging validation

1. Send A -> B.
2. Verify exactly one inbound message.
3. Verify Sent/Delivered.
4. Open B conversation and verify Read.
5. Send Reply and verify quoted reference survives transport/restart.
6. Interrupt Tor/network during send.
7. Verify durable retry after reconnect.
8. Exhaust delivery to Failed and use Retry Now; confirm the same durable outbox is requeued.
9. Force ACK loss/replay; B must not persist a duplicate.
10. Confirm Duplicate ACK completes sender work.
11. Restart sender while a claim is stale and verify recovery.

## Relationship validation

- Rename Contact changes local metadata only.
- Block closes current peer sessions and prevents new outgoing/incoming sessions.
- Unblock allows normal authenticated reconnect.
- Clear History removes local messages, control work, attachment metadata/cache/staging but retains the relationship credential.
- Remove Contact removes relationship/history/credential metadata and deletes the protected pairwise-secret handle.

## Attachment validation

1. Select a small file.
2. Confirm persistent progress.
3. Interrupt transfer.
4. Restart one/both clients.
5. Resume from durable offset.
6. Confirm SHA-256 completion behavior.
7. Test Retry and Cancel.
8. Test preparation failure and confirm message Failed + text outbox dead-letter.
9. Test Open: Rust verifies/decrypts into a controlled temporary export before platform open.
10. Test Save As: Rust writes a verified plaintext destination atomically.
11. Confirm encrypted cache paths are never exposed to Flutter.
12. Confirm stale temporary exports are removed by startup maintenance.

## Diagnostics validation

Diagnostics may expose component/state/code/timestamps but must not contain:

- private keys;
- database keys;
- pairwise peer secret bytes;
- capability/token values;
- Android service message plaintext.

Validate diagnostics export and self-test during Tor failure, reconnect, pairing and delivery failure.

## Dependency/lock validation

Rust registry versions/checksums are not intentionally upgraded by the final source changes. The only expected `Cargo.lock` edits are local workspace dependency-array reconciliation for crates whose direct path dependencies changed.

Flutter source uses the packages declared in `pubspec.yaml`, including `app_links`, `shared_preferences` and `open_filex`. `launch_at_startup` is not used by current source and should not appear only because it existed in an earlier plan.

The implementation environment does not contain Flutter/Dart and the local shell cannot resolve GitHub, so owner validation must run the normal dependency resolver and inspect any generated `pubspec.lock`/`Cargo.lock` diff before accepting it. Do not silently accept unrelated registry/package drift.

## Release gate closure

Close gates only with evidence:

- **GATE-001** protected crypto/key lifecycle;
- **GATE-002** SQLCipher migrations/recovery;
- **GATE-003** Bridge/C ABI v9 runtime;
- **GATE-004** Pairing Protocol v2 and trust replacement policy;
- **GATE-005** Windows lifecycle/notifications/deep links;
- **GATE-006** Android lifecycle/notifications/deep links;
- **GATE-007** relay/Tor/P2P/text/receipts/attachments E2E;
- **GATE-008** release artifacts, signing, checksums and platform matrix.

## Failure handling rule

For every validation failure:

1. capture the first real error/log;
2. identify the owning layer;
3. make one focused fix;
4. commit as one `BUG-*` change;
5. rerun the same validation checkpoint;
6. update `0.1_PROGRESS.md` only after the result is known.

## Release-ready definition

Torca 0.1 is release-ready only when both platform hosts and the full two-client scenario pass:

```text
pair v2 -> approve -> persist signed relationship
 -> restart
 -> direct Tor text/reply
 -> interruption/retry/dedup
 -> Delivered/Read
 -> block/unblock/cleanup
 -> attachment interruption/resume/export
 -> Android background notifications / Windows tray/deep link
 -> clean intentional shutdown
```

Source completion alone is not release evidence.
