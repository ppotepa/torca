# Torca architecture

Torca 0.1 is a modular Rust monolith behind one responsive Flutter client. Windows and Android are platform hosts for the same application, not separate products or business-code implementations.

## Runtime ownership

```text
Flutter UI
   |
   v
EngineGateway / Bridge DTO v4
   |
   v
torca-native C ABI
   |
   v
process-owned NativeEngineRuntime
   |
   +-- EngineBridge / ClientEngineActor
   |
   +-- RuntimeHost
       |
       +-- PairingDriver
       |    +-- PairingRuntime
       |    +-- PairingCoordinator
       |    +-- RendezvousClient
       |    +-- protected identity / peer secrets
       |
       +-- TorDriver
       |    +-- owned Tor child process
       |    +-- SOCKS endpoint
       |    +-- local onion service endpoint
       |
       +-- CommunicationDriver
            +-- SharedPeerLink
            +-- durable text DeliveryWorker
            +-- durable control/receipt worker
            +-- inbound exactly-once dispatcher
            +-- read-state workflow
            +-- attachment transfer
            +-- diagnostics
```

The process runtime is the owner of Tor, pairing, peer sessions and delivery workers. Flutter owns presentation handles only. Destroying an Android Activity or Flutter engine does not destroy the Torca runtime. A real application Quit uses the explicit process-shutdown capability.

## Deployable units

Version 0.1 has two deployable units:

1. **Torca client** — one Flutter/Rust client built for Windows or Android.
2. **Torca relay** — an untrusted, in-memory rendezvous service used only while contacts pair.

The relay is not a mailbox, account server, presence service, message history store or normal messaging path.

## Dependency groups

### Foundation

`torca-foundation` contains identifiers, timestamps, command metadata and small dependency-light primitives.

### Domains

Focused domain crates own invariants for:

- identity;
- contacts and peer credential metadata;
- conversations;
- pairing;
- messaging;
- receipts;
- attachments.

Domain crates do not depend on Flutter, FFI, SQLite implementations, Tor process APIs or OS key stores.

### Application

Application crates coordinate use cases and own inward-facing ports. Important owners are:

- `torca-client-engine` — single writer for domain/application state;
- `torca-runtime-host` — process background command/tick owner;
- `torca-pairing-coordinator` — pairing orchestration boundary;
- `torca-delivery` — durable text delivery worker;
- `torca-control-delivery` — durable receipt/control delivery port + worker;
- `torca-communication-driver` — central communication dispatcher contract;
- `torca-read-state` — mark-read/receipt workflow;
- `torca-diagnostics` — bounded redacted diagnostic events.

Flutter never owns retries, outbox state, pairing cryptography, private keys or SQL.

### Infrastructure

Infrastructure implements application ports:

- `torca-storage-sqlite` — SQLCipher, migrations and operational SQL;
- `torca-crypto` — Ed25519, X25519, HKDF, XChaCha20-Poly1305 and protected-secret adapters;
- `torca-rendezvous-client` — relay client and Tor-backed relay transport;
- `torca-peer-link` / `torca-peer-shared` — authenticated peer link ownership and shared link handle;
- `torca-transport-tor` — Tor process, SOCKS, peer listener and framed streams;
- `torca-communication-adapters` — production composition for text/control/inbound/read/attachments;
- `torca-attachment-sqlite` / `torca-attachment-transfer` — durable resumable attachment transfer;
- `torca-pairing-driver` / `torca-tor-driver` — concrete RuntimeHost drivers;
- `torca-file-storage` — encrypted attachment cache/staging.

Superseded `torca-peer-runtime` and `torca-peer-delivery` crates were removed.

### Protocol

Protocol crates own versioned, bounded wire representations:

- `torca-wire` — TCP frame boundaries;
- `torca-relay-protocol` — relay request/response v2;
- `torca-pairing-protocol` — encrypted pairing payloads;
- `torca-peer-protocol` — authenticated peer handshake/data/ACK;
- `torca-attachment-protocol` — attachment metadata/chunk/resume/complete.

Domain aggregates are never serialized directly as the network protocol.

## Bridge and ABI v4

`torca-bridge` projects application state into presentation-safe DTOs. Contract version 4 includes:

- identity;
- pairing sessions;
- contacts and per-contact peer connection state;
- conversations/messages;
- Tor state and local onion endpoint;
- attachment progress;
- diagnostics access;
- `MarkConversationRead`;
- Create/Join/Approve/Reject/Cancel pairing commands.

The C ABI uses opaque process handles and pointer/length UTF-8 arguments. `torca_engine_destroy` releases one presentation handle. `torca_process_shutdown` is the explicit process-wide shutdown used by real Quit.

## Storage and SQL rule

All operational SQLite/SQLCipher statements live as external `.sql` files under storage/infrastructure SQL directories and are executed with parameters. Rust application/domain source must not embed operational SQL.

The final SQLCipher catalog currently contains migrations **1 through 14**. Important invariants include:

- outbound message insert creates durable outbox ownership;
- outbox lifecycle updates user-visible message lifecycle atomically;
- stale claimed sends recover to queued;
- delivery attempt counts and timestamps stay synchronized;
- inbound dedup and message persistence are atomic;
- receipts and message-state transitions are atomic;
- control/receipt jobs are durable;
- peer credential metadata is persistent while key material remains protected outside SQL;
- attachment metadata/progress are persistent;
- attachment preparation failure transitions the outbound message to `Failed` and dead-letters its text outbox row.

## Key material

Secrets do not cross the bridge.

Windows uses DPAPI-backed protected stores. Android uses Android Keystore-backed stores. Separate namespaces are used for database, identity and peer secrets. SQLCipher receives a protected random database key. Identity private keys and pairwise peer secrets are referenced by opaque handles.

Pairing uses ephemeral X25519 + HKDF and authenticated encryption. Explicit approvals sign a canonical transcript with the local Ed25519 identity key. Pairwise communication secrets are derived with a separate KDF context and stored through protected peer-secret storage.

## Pairing flow

```text
Creator: Create invitation
   -> Rust CSPRNG code + TTL
   -> ephemeral X25519 key
   -> relay slot + capability/token

Joiner: Join code / QR
   -> relay join
   -> encrypted PairingOffer exchange

Both
   -> validate peer identity/route/capability
   -> explicit user approval
   -> signed canonical transcript approval
   -> verified remote approval
   -> completion confirmation
   -> derive pairwise peer secret
   -> protected secret store
   -> atomic Contact + Conversation + PeerCredential metadata
   -> relay/ephemeral cleanup
```

The invitation code/QR contains no long-term secret. The relay only carries opaque encrypted pairing material and is not used after the relationship is committed.

## Direct messaging flow

```text
Flutter QueueMessage
   -> ClientEngine
   -> SQLCipher message + durable outbox
   -> RuntimeHost wakes communication driver
   -> PeerLink reuse/connect
   -> Tor SOCKS -> peer onion service
   -> signed authenticated handshake
   -> application payload encode
   -> pairwise AEAD
   -> PeerMessage::Data
   -> peer decrypt/validate
   -> atomic inbound dedup + persist
   -> protocol ACK
   -> durable Delivered receipt
```

`Accepted` and `Duplicate` ACKs both complete the sender outbox because retransmission uses a stable envelope identifier. Rejected/timeouts are rescheduled according to bounded retry policy; dead-letter is durable.

## Read receipts

Opening a conversation emits `MarkConversationRead` through the bridge. Rust atomically changes eligible inbound messages to Read and inserts durable Read receipt jobs. Receipts use the same peer link/reconnect path rather than a separate socket stack.

## Attachments

Flutter selects a local file and passes path + metadata once. Rust immediately copies/encrypts it into app-private storage, then owns transfer state.

```text
picker path
   -> prepare encrypted cache
   -> persistent attachment metadata/progress
   -> same SharedPeerLink
   -> bounded chunks
   -> duplicate-safe offset tracking
   -> interruption/restart resume
   -> SHA-256 completion verification
   -> atomic final cache/staging finalization
```

Dart does not perform chunking, encryption, resume bookkeeping or retry scheduling.

## Tor runtime

Tor is a packaged runtime dependency, not discovered through PATH or Tor Browser. Build/run/deploy stage the packaged binary and relay endpoint configuration. `OwnedTorDriver` starts and monitors the child process, exposes SOCKS/onion state, and restarts failed Tor processes with bounded backoff/jitter.

## Platform lifecycle

### Windows

- runner-level single-instance guard;
- second launch activates the first window;
- window close hides to tray;
- tray Show restores/focuses;
- tray Quit explicitly shuts down the process RuntimeHost, then releases FFI handles/window resources;
- desktop notifications observe Rust snapshots only.

### Android

- foreground service uses the `remoteMessaging` service type;
- service owns the process lifetime of the Rust runtime;
- Activity/Flutter-engine recreation releases only presentation handles;
- true process/service shutdown explicitly stops the Rust process runtime;
- packaged Tor path/runtime root/relay endpoint are supplied by the Android host bridge.

## Diagnostics

Diagnostics are bounded and redacted. They may include component/state/code/timestamps but must not include plaintext messages, private keys, pairwise secrets, capability values or protected database keys.

## One-client UI rule

There is one shared Flutter widget tree. Screen width changes layout only; it does not select a second application implementation.

```text
compact -> routed screens -> ConversationPane
wide    -> conversation list | ConversationPane
```

Pairing Create/Join, Tor/P2P indicators, mark-read, attachments and diagnostics are shared features.

## Developer operations

Public entrypoints remain:

```powershell
./scripts/build.ps1
./scripts/run.ps1
./scripts/deploy.ps1
```

Platform bootstrap, code generation, native compilation and packaging live under `tools/build`.

## Validation status

This document describes the final **source architecture**. It does not assert that the current source has passed the final owner Windows/Android/end-to-end validation matrix. Validation status and exact handoff are tracked in `0.1_PROGRESS.md` and `docs/0.1/RELEASE_HANDOFF.md`.
