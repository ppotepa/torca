# Torca architecture

Torca 0.1 is a modular Rust monolith behind one responsive Flutter client. Windows and Android are platform hosts for the same application, not separate business implementations.

## Runtime ownership

```text
Flutter UI
   -> EngineGateway / Bridge DTO v9
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
              +-- transactional read state
              +-- relationship administration
              +-- resumable attachment transfer/export
              +-- diagnostics
```

The process runtime owns Tor, pairing, peer sessions and delivery workers. Flutter owns presentation handles only. Android Activity/Flutter-engine destruction does not destroy Torca. Real application Quit uses the explicit process-wide shutdown capability.

## Deployable units

Version 0.1 has two deployable units:

1. **Torca client** — the same Flutter/Rust client built for Windows or Android.
2. **Torca relay** — an untrusted ephemeral rendezvous service used only while contacts pair.

The relay is not a mailbox, account server, presence service, history store or normal messaging route.

## Layers

### Foundation and domains

`torca-foundation` owns identifiers, timestamps and dependency-light primitives. Focused domain crates own identity, contacts/peer credentials, conversations, pairing, messaging, receipts and attachments. Domain crates do not depend on Flutter, FFI, SQL implementations, sockets, Tor processes or OS key stores.

### Application

Application code coordinates use cases and inward-facing ports:

- `torca-client-engine` — single writer for identity/domain persistence and message state;
- `torca-runtime-host` — process background command/tick owner;
- `torca-pairing-coordinator` — pairing orchestration;
- `torca-delivery` — durable text delivery worker;
- `torca-control-delivery` — durable receipt/control port and worker;
- `torca-communication-driver` — central peer/text/receipt/attachment/relationship dispatcher contract;
- `torca-diagnostics` — bounded redacted diagnostics.

`torca-read-state` is a compatibility facade; concrete transactional Read-state SQL is storage-owned.

Flutter never owns retries, outbox state, pairing cryptography, private keys, peer secrets or SQL.

### Infrastructure

Infrastructure implements application ports:

- `torca-storage-sqlite` — SQLCipher, migrations, durable message/control/read/relationship persistence and external operational SQL;
- `torca-crypto` — Ed25519, X25519, HKDF, XChaCha20-Poly1305 and protected-secret adapters;
- `torca-rendezvous-client` — relay client and Tor-backed relay transport;
- `torca-peer-link` / `torca-peer-shared` — authenticated link ownership and shared handle;
- `torca-transport-tor` — Tor process, SOCKS, hidden-service peer listener and framed streams;
- `torca-communication-adapters` — production text/control/inbound/read/relationship/attachment composition;
- `torca-attachment-sqlite` / `torca-attachment-transfer` — durable resumable attachments;
- `torca-pairing-driver` / `torca-tor-driver` — concrete RuntimeHost drivers;
- `torca-file-storage` — encrypted attachment cache.

The peer-link receives an `ActiveRelationshipStore` projection so blocked relationships are rejected for both outbound dialing and incoming authentication.

### Protocol

Versioned bounded protocol crates own wire representations: `torca-wire`, `torca-relay-protocol`, `torca-pairing-protocol`, `torca-peer-protocol` and `torca-attachment-protocol`. Domain aggregates are never serialized directly as network protocol objects.

`torca-pairing-protocol` is currently **v2**. The encrypted `PairingOffer` includes the peer display name in canonical transcript bytes, so the initial contact name is covered by explicit signed approval together with identity, route and capability metadata.

## Bridge and C ABI v9

`torca-bridge` projects application state to presentation-safe DTOs. Contract **v9** includes:

- local identity name;
- pairing state;
- Tor/onion state;
- contact display name, relationship state, P2P connection state and Safety Number;
- conversations;
- messages with reply reference, timestamps and attempt count;
- attachment progress/control and verified export;
- relationship commands: rename, block/unblock, clear history and remove;
- Retry Now and MarkConversationRead;
- redacted diagnostics.

The C ABI uses opaque process handles and pointer/length UTF-8 arguments. `torca_engine_destroy` releases one presentation handle. `torca_process_shutdown` is the explicit process-wide shutdown used by real Quit. Flutter now has one canonical `ffi_engine_gateway.dart`; superseded `_v4` and `_final` gateways were physically removed.

## Storage and SQL

Operational SQLite/SQLCipher statements live as external `.sql` files in infrastructure/storage and are parameterized. Application/domain Rust source must not embed operational SQL.

The SQLCipher migration catalog is **1 through 17**. Important invariants include:

- outbound message insert creates durable outbox ownership;
- outbox lifecycle atomically updates message lifecycle, attempts and timestamps;
- stale claims recover after restart;
- inbound dedup + message persistence are atomic;
- receipt insertion + message transitions are atomic;
- Delivered/Read control jobs are durable;
- MarkConversationRead + durable Read receipt creation are transactional;
- Contact + Conversation + PeerCredential metadata finalization is atomic;
- peer key bytes are never stored in SQL;
- attachment metadata/progress are persistent;
- attachment preparation failure marks the message Failed and dead-letters its text outbox;
- Retry Now requeues a failed outbound message and dead-lettered outbox atomically;
- local contact aliases are durable metadata;
- remote identity is unique while a contact exists, preventing silent key replacement.

## Cryptographic ownership

Secrets never cross the bridge. Windows uses DPAPI-backed stores; Android uses Android Keystore-backed stores. Database, identity and peer secrets use separate namespaces. SQLCipher receives a protected random database key; identity and pairwise secrets are referenced through opaque handles.

Pairing uses ephemeral X25519, HKDF and authenticated encryption. Explicit approvals sign a canonical transcript with the Ed25519 identity key. Pairwise communication secrets use a separate KDF context and protected peer-secret storage.

Safety Numbers are symmetric hashes of ordered verified public identities and contain no secret material.

## Pairing

```text
Create invitation
 -> Rust CSPRNG code + TTL + QR/deep link
 -> ephemeral X25519 + relay capability/token
Join code/QR/torca://pair
 -> encrypted PairingOffer v2 exchange
 -> display name + identity + route + capability in canonical transcript
 -> explicit approvals
 -> Ed25519 transcript verification
 -> completion confirmation
 -> derive pairwise secret
 -> protected secret store
 -> atomic Contact + Conversation + PeerCredential
 -> signed peer display name persisted as local contact metadata
 -> relay/ephemeral cleanup
```

A second live contact with the same verified remote identity is rejected. Replacing trust requires an explicit local Remove Contact followed by a new pairing.

## Direct messaging

```text
QueueMessage
 -> SQLCipher message + durable outbox
 -> RuntimeHost / DeliveryWorker
 -> SharedPeerLink reuse/connect
 -> ActiveRelationshipStore policy
 -> Tor SOCKS -> peer onion
 -> signed authenticated handshake
 -> application codec + pairwise AEAD
 -> peer decrypt/validate
 -> atomic inbound dedup + persist
 -> durable Delivered receipt
 -> protocol ACK
```

Stable envelope IDs make retries duplicate-safe. `Accepted` and `Duplicate` ACKs both complete durable sender work.

Blocked contacts are absent from the peer-link repository view: normal reconnect, explicit send-triggered connect and incoming handshakes all fail until the relationship is unblocked.

## Relationship administration

RuntimeHost exposes local relationship operations rather than allowing Flutter to mutate SQL directly:

- Rename contact — local metadata only; cryptographic identity is unchanged;
- Block/Unblock — closes ephemeral sessions and gates future peer authentication;
- Clear history — atomically removes message/control/attachment metadata for the conversation and purges encrypted attachment cache/staging;
- Remove contact — removes relationship/history/credential metadata and deletes the protected pairwise secret handle.

## Read receipts

Opening a conversation sends `MarkConversationRead`. The storage transaction changes eligible inbound messages to Read and inserts durable Read receipt jobs. Receipts use the same reconnect/peer-link path as text.

## Attachments

Flutter supplies a selected path and metadata once. Rust copies/encrypts the source into app-private storage and owns persistent chunk/resume state. Transfer uses the same SharedPeerLink, bounded chunks, durable offsets and SHA-256 completion verification.

Open/Save As never exposes the encrypted cache path. Rust loads the encrypted cache, verifies authenticated decryption, size and SHA-256 digest, then atomically writes the user-selected plaintext destination. Temporary decrypted Open exports use controlled `torca-<id>.*` names and stale exports are removed during startup maintenance.

## Tor and platform lifecycle

Tor is a packaged runtime dependency, not discovered through PATH or Tor Browser. `OwnedTorDriver` owns the child process, SOCKS/onion state and bounded restart backoff.

Windows provides:

- single-instance activation;
- close-to-tray and explicit process Quit;
- local private-message notifications;
- `torca://pair` per-user protocol registration and single-instance pending-link handoff.

Android uses a `remoteMessaging` foreground service as process lifetime owner. Activity recreation releases presentation handles only. The service also owns user-message notification detection from a **redacted JNI projection** containing IDs, display names and direction only — no message body, onion address, Safety Number or secret material. Message notification clicks route back into the existing `singleTask` Activity and then to the shared conversation navigator.

Android also registers `torca://pair` as a VIEW/BROWSABLE custom scheme. Both initial links and links received while running are routed through the shared `AppNavigationController`.

## UI and diagnostics

There is one shared responsive Flutter widget tree. Screen width changes layout, not business implementation. Pairing, contact administration, Tor/P2P state, Reply, Retry Now, Message Details, read receipts, attachments and diagnostics are shared features.

Diagnostics are bounded/redacted and must not expose plaintext secrets, private keys, pairwise keys, capability values or database keys. The diagnostics screen can export the already-redacted event stream and run a self-test using only observable runtime states.

## Developer operations

Public entrypoints remain:

```powershell
./scripts/build.ps1
./scripts/run.ps1
./scripts/deploy.ps1
```

Platform bootstrap, code generation, native compilation and packaging live under `tools/build`.

## Validation status

This document describes the current **source architecture**. It does not assert owner Windows/Android/end-to-end validation. Final owner validation remains a separate gate and is tracked in `0.1_PROGRESS.md` and `docs/0.1/RELEASE_HANDOFF.md`.
