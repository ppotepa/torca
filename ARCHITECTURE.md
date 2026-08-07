# Torca architecture

Torca 0.1 is a modular Rust monolith behind one responsive Flutter client. Windows and Android are platform hosts for the same application, not separate business implementations.

## Runtime ownership

```text
Flutter UI
   -> EngineGateway / Bridge DTO v4
   -> torca-native C ABI
   -> process-owned NativeEngineRuntime
      +-- EngineBridge / ClientEngineActor
      +-- RuntimeHost
          +-- PairingDriver
          |   +-- PairingRuntime / PairingCoordinator
          |   +-- RendezvousClient
          |   +-- protected identity / peer secrets
          +-- TorDriver
          |   +-- owned Tor child process
          |   +-- SOCKS + onion endpoint
          +-- CommunicationDriver
              +-- SharedPeerLink
              +-- durable text DeliveryWorker
              +-- durable control/receipt worker
              +-- inbound exactly-once dispatcher
              +-- storage-owned transactional read state
              +-- attachment transfer
              +-- diagnostics
```

The process runtime owns Tor, pairing, peer sessions and delivery workers. Flutter owns presentation handles only. Android Activity/Flutter-engine destruction does not destroy Torca. Real application Quit uses the explicit process-wide shutdown capability.

## Deployable units

Version 0.1 has two deployable units:

1. **Torca client** — the same Flutter/Rust client built for Windows or Android.
2. **Torca relay** — an untrusted in-memory rendezvous service used only while contacts pair.

The relay is not a mailbox, account server, presence service, history store or normal messaging route.

## Layers

### Foundation and domains

`torca-foundation` owns identifiers, timestamps and small dependency-light primitives. Focused domain crates own identity, contacts/peer credentials, conversations, pairing, messaging, receipts and attachments. Domain crates do not depend on Flutter, FFI, SQL implementations, sockets, Tor processes or OS key stores.

### Application

Application code coordinates use cases and inward-facing ports:

- `torca-client-engine` — single writer for domain/application state;
- `torca-runtime-host` — process background command/tick owner;
- `torca-pairing-coordinator` — pairing orchestration;
- `torca-delivery` — durable text delivery worker;
- `torca-control-delivery` — durable receipt/control port and worker;
- `torca-communication-driver` — central communication dispatcher contract;
- `torca-diagnostics` — bounded redacted diagnostics.

`torca-read-state` remains only as a compatibility facade. The concrete transactional Read-state implementation and all of its SQL are owned by `torca-storage-sqlite`.

Flutter never owns retries, outbox state, pairing cryptography, private keys or SQL.

### Infrastructure

Infrastructure implements application ports:

- `torca-storage-sqlite` — SQLCipher, migrations, text/control/read-state persistence and operational SQL;
- `torca-crypto` — Ed25519, X25519, HKDF, XChaCha20-Poly1305 and protected-secret adapters;
- `torca-rendezvous-client` — relay client and Tor-backed relay transport;
- `torca-peer-link` / `torca-peer-shared` — authenticated link ownership and shared handle;
- `torca-transport-tor` — Tor process, SOCKS, peer listener and framed streams;
- `torca-communication-adapters` — production text/control/inbound/read/attachment composition;
- `torca-attachment-sqlite` / `torca-attachment-transfer` — durable resumable attachments;
- `torca-pairing-driver` / `torca-tor-driver` — concrete RuntimeHost drivers;
- `torca-file-storage` — encrypted attachment cache/staging.

Superseded `torca-peer-runtime` and `torca-peer-delivery` were physically removed.

### Protocol

Versioned bounded protocol crates own wire representations: `torca-wire`, `torca-relay-protocol`, `torca-pairing-protocol`, `torca-peer-protocol` and `torca-attachment-protocol`. Domain aggregates are not serialized directly as network protocol objects.

## Bridge and ABI v4

`torca-bridge` projects application state to presentation-safe DTOs. Contract v4 includes identity, pairing, Tor/onion state, per-contact P2P state, conversations/messages, MarkConversationRead, attachment progress/control and diagnostics.

The C ABI uses opaque process handles and pointer/length UTF-8 arguments. `torca_engine_destroy` releases one presentation handle. `torca_process_shutdown` is the explicit process-wide shutdown used by real Quit.

## Storage and SQL

Operational SQLite/SQLCipher statements live as external `.sql` files in infrastructure/storage and are parameterized. Application/domain Rust source must not embed operational SQL.

The final SQLCipher migration catalog is **1 through 14**. Important invariants include:

- outbound message insert creates durable outbox ownership;
- outbox lifecycle atomically updates message lifecycle, attempts and timestamps;
- stale claims recover after restart;
- inbound dedup + message persistence are atomic;
- receipt insertion + message transitions are atomic;
- Delivered/Read control jobs are durable;
- MarkConversationRead + durable Read receipt creation are transactional in storage;
- Contact + Conversation + PeerCredential metadata finalization is atomic;
- peer key bytes are not stored in SQL;
- attachment metadata/progress are persistent;
- attachment preparation failure marks the message Failed and dead-letters its text outbox.

## Cryptographic ownership

Secrets never cross the bridge. Windows uses DPAPI-backed stores; Android uses Android Keystore-backed stores. Database, identity and peer secrets use separate namespaces. SQLCipher receives a protected random database key; identity and pairwise secrets are referenced through opaque handles.

Pairing uses ephemeral X25519, HKDF and authenticated encryption. Explicit approvals sign a canonical transcript with the Ed25519 identity key. Pairwise communication secrets use a separate KDF context and protected peer-secret storage.

## Pairing

```text
Create invitation
 -> Rust CSPRNG code + TTL + QR
 -> ephemeral X25519 + relay capability/token
Join code/QR
 -> encrypted PairingOffer exchange
 -> validate peer identity/route/capability
 -> explicit approvals
 -> signed canonical transcript verification
 -> completion confirmation
 -> derive pairwise secret
 -> protected secret store
 -> atomic Contact + Conversation + PeerCredential
 -> relay/ephemeral cleanup
```

`PeerProposal` and secret material do not cross Flutter.

## Direct messaging

```text
QueueMessage
 -> SQLCipher message + durable outbox
 -> RuntimeHost / DeliveryWorker
 -> SharedPeerLink reuse/connect
 -> Tor SOCKS -> peer onion
 -> signed authenticated handshake
 -> application codec + pairwise AEAD
 -> peer decrypt/validate
 -> atomic inbound dedup + persist
 -> durable Delivered receipt
 -> protocol ACK
```

Stable envelope IDs make retries duplicate-safe. `Accepted` and `Duplicate` ACKs both complete durable sender work.

## Read receipts

Opening a conversation sends `MarkConversationRead`. The storage-owned transaction changes eligible inbound messages to Read and inserts durable Read receipt jobs. Receipts use the same reconnect/peer-link path as text.

## Attachments

Flutter supplies a selected path and metadata once. Rust copies/encrypts the source into app-private storage and owns persistent chunk/resume state. Transfer uses the same SharedPeerLink, bounded chunks, durable offsets and SHA-256 completion verification. Dart does not implement encryption, chunking or retry scheduling.

## Tor and lifecycle

Tor is a packaged runtime dependency, not discovered through PATH or Tor Browser. `OwnedTorDriver` owns the child process, SOCKS/onion state and bounded restart backoff.

Windows provides single-instance activation, close-to-tray, Show, explicit process Quit and notifications. Android uses a `remoteMessaging` foreground service as process lifetime owner; Activity recreation releases presentation handles only.

## UI and diagnostics

There is one shared responsive Flutter widget tree. Screen width changes layout, not business implementation. Pairing, Tor/P2P state, read receipts, attachments and diagnostics are shared features.

Diagnostics are bounded/redacted and must not expose plaintext secrets, private keys, pairwise keys, capability values or database keys.

## Developer operations

Public entrypoints remain:

```powershell
./scripts/build.ps1
./scripts/run.ps1
./scripts/deploy.ps1
```

Platform bootstrap, code generation, native compilation and packaging live under `tools/build`.

## Validation status

This document describes the final **source architecture**. It does not assert final owner Windows/Android/end-to-end validation. Validation status is tracked in `0.1_PROGRESS.md` and `docs/0.1/RELEASE_HANDOFF.md`.
