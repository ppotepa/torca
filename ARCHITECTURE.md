# Torca architecture

Torca is a modular monolith built from focused Rust libraries and **one responsive Flutter client**. Deployment remains deliberately simple: one client application per device plus the optional ephemeral pairing relay.

## Top-level composition

```text
                 one responsive Flutter client
                           |
                     FfiEngineGateway
                           |
             torca-native C ABI / cdylib
                           |
                       EngineBridge
                           |
               ClientEngine single writer
                           |
         application workflows / projections
                           |
                    mini-domain crates
                           ^
                           |
       storage / crypto / peer / Tor / OS adapters
```

Windows and Android are build targets, not alternative application implementations. UI behavior may adapt to screen width, input model and lifecycle, but there is only one set of screens, commands and application state.

## Deployable units

Version 0.1 has two deployable units:

1. **Torca client** — the same Flutter/Rust client built for Windows or Android.
2. **Torca relay** — an untrusted, in-memory rendezvous broker used only while contacts pair.

The relay is not a message server, presence server, account server, directory, backup service or mailbox.

## Component groups

### Foundation

Dependency-light identifiers, timestamps, command/event metadata and error primitives.

### Domains

Focused mini-domains own vocabulary and invariants:

- identity;
- contacts;
- pairing;
- conversations;
- messaging;
- receipts;
- attachments.

Domain crates may depend on foundation and explicitly approved domain contracts. They never depend on Flutter, FFI, SQLite implementations, sockets or Tor process APIs.

### Application

Application code coordinates use cases across domains. `ClientEngine` is the single writer for mutable client state. Flutter must not implement a parallel workflow state machine.

### Infrastructure

Infrastructure implements inward-facing ports and owns SQLCipher, cryptographic providers, encrypted file storage, peer sessions and Tor integration.

### Protocol

Protocol crates own explicitly versioned peer/relay/wire representations. Domain aggregates are never serialized directly.

### Platform

`torca-bridge` maps application concepts to presentation-safe bridge DTOs.

`torca-native` owns the narrow shared C ABI and the process-local Rust engine lifetime. It builds as `torca_bridge.dll` on Windows and `libtorca_bridge.so` on Android.

OS-specific adapters remain only for capabilities that actually require an OS API, for example Windows DPAPI, Android Keystore, tray behavior, notifications and lifecycle ownership.

## One-client UI rule

Responsive behavior is expressed inside the shared Flutter widget tree:

```text
compact width
    -> conversation list
    -> routed ConversationScreen
    -> ConversationPane

wide width
    -> conversation list | ConversationPane
```

`ConversationPane` is the same widget in both cases. No desktop/mobile feature fork is permitted for business behavior.

## Native boundary rule

Flutter sends typed bridge commands through `FfiEngineGateway`. The C ABI exposes narrow operations and presentation snapshots; it does not expose Rust domain object layouts.

```text
Dart command DTO
   -> UTF-8 / primitive ABI arguments
   -> EngineBridge command
   -> ClientEngine
   -> BridgeSnapshot
   -> presentation-safe JSON buffer
   -> Dart DTO
```

Memory gateway selection is explicit development/test behavior only. Native-runtime failure is an error state, never a silent fallback.

## Storage rule

All SQL lives under the SQLite/SQLCipher storage crate as `.sql` files. Runtime business SQL in Rust source is prohibited. Storage owns transactions, migrations and raw database connections.

Outbound messages use a durable outbox. Inbound envelopes use deduplication. Recovery behavior must remain idempotent across process interruption.

## Network rule

Pairing may use the relay to exchange opaque short-lived rendezvous material. Once a contact is verified, normal messaging is peer-to-peer through Tor onion services. Peer sessions operate on encrypted protocol envelopes, not Flutter DTOs or domain objects.

## Core dependency direction

```text
foundation
   <- domains
   <- application
   <- bridge
   <- native/client presentation

infrastructure implements ports defined inward
```

Infrastructure does not leak into domains. Flutter and OS hosts do not become alternative application layers.

## Core rules

1. One Flutter client source for every supported platform.
2. One Rust `ClientEngine` owner per running client process.
3. All state-changing client operations pass through that engine.
4. Domain code contains no SQL, sockets, Flutter or FFI types.
5. SQL is external, parameterized and storage-owned.
6. Private keys never cross into Flutter DTOs.
7. Outbound delivery is durable and retryable; inbound delivery is deduplicated.
8. Wire protocols are explicitly versioned.
9. Cross-domain effects are coordinated by application code.
10. Platform-specific code is limited to actual OS integration.
11. `main` remains internally coherent after every commit.

## Primary flows

### Pairing

```text
Flutter command
    -> ClientEngine pairing workflow
    -> ephemeral relay exchange
    -> explicit approvals and verification
    -> contact + direct conversation
    -> direct peer endpoint registered
```

### Sending a message

```text
Flutter command
    -> ClientEngine
    -> messaging domain
    -> SQLCipher message + durable outbox transaction
    -> encrypted peer envelope over Tor
    -> protocol acknowledgement / receipt
    -> projection snapshot
    -> Flutter UI
```

### Receiving a message

```text
Tor peer stream
    -> authenticated peer session
    -> envelope verification/decryption
    -> inbound deduplication
    -> messaging domain/application handler
    -> durable state
    -> projection snapshot
    -> Flutter UI
```

## Developer operations

Only three public scripts exist:

```powershell
./scripts/build.ps1
./scripts/run.ps1
./scripts/deploy.ps1
```

All formatting, code generation, validation, platform bootstrap, native compilation and packaging details live under `tools/build`.
