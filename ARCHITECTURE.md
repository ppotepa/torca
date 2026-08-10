# Torca architecture

This document describes the stable shape of the current Torca codebase. It intentionally stays above individual methods, schema numbers, protocol versions and release-specific migration details. When this document and the code disagree, the active workspace and enforced architecture policies are authoritative and the documentation should be corrected.

## Architectural intent

Torca is a modular Rust application behind one responsive Flutter client. Windows and Android are platform hosts of the same product rather than separate business implementations.

The central rule is that presentation expresses **user intent** and renders **application read models**. Durable state, identity, cryptography, Tor, pairing, delivery and retry policy remain inside Rust.

```text
Presentation
  Flutter widgets / navigation / local UI preferences
        |
        v
Platform contract
  generated DTOs + command/read-model serialization
        |
        v
Native boundary
  process handle + C/JNI/host integration
        |
        v
Application
  ClientApplicationRuntime / application facade
        |
        +-------------------------------+
        |                               |
        v                               v
  single-writer engine              process runtime
  domain consistency               background ownership
        |                               |
        +---------------+---------------+
                        |
                        v
                 domain/application ports
                        ^
                        |
                 infrastructure adapters
          SQLCipher / crypto / Arti / files / peer IO
```

## Layers

### Foundation

`crates/foundation` contains dependency-light primitives used throughout the system: identifiers, timestamps, cancellation, command/event helpers and classified errors. It should not acquire product workflows or infrastructure dependencies.

### Domains

`crates/domains` owns product semantics and invariants for identity, contacts, conversations, pairing, messaging, receipts, attachments, presence and notification intent.

Domains describe **what a valid Torca state means**. They do not know about Flutter, SQLCipher, Arti, sockets, JNI or Win32 APIs.

Presence is derived from observed connection/activity facts rather than from a central presence service. Notification policy is similarly modeled independently from Android/Windows notification APIs.

### Protocols

`crates/protocol` owns bounded wire formats and protocol validation for peer, pairing, relay, attachment and generic framing concerns. Domain objects are not serialized directly just because they happen to contain similar data.

Protocol crates are below application/infrastructure orchestration and are kept free of platform behavior.

### Application

`crates/application` coordinates use cases and defines the interfaces infrastructure must satisfy.

The main application boundary is `torca-client-application`. It exposes application commands, queries/read models, security projections and policy used by the presentation contract. The presentation layer should not reconstruct application state by querying unrelated repositories itself.

The `torca-client-engine` remains the single-writer consistency boundary for durable domain transitions.

`torca-runtime` is the long-lived background owner for network-facing work. It coordinates Tor state, pairing, peer connectivity, durable message/control delivery, attachments, probes, connectivity observations and diagnostics through application-defined driver interfaces.

Supporting application crates isolate concerns such as bootstrap state, connectivity, probing, delivery, control delivery, pairing coordination and diagnostics. These are not separate services; they are modules of one client application.

### Infrastructure

`crates/infrastructure` contains concrete adapters for application/domain ports.

Important owners include:

- `torca-storage-sqlite` — SQLCipher-backed repositories, durable queues, read state, security projections and settings;
- `torca-crypto` — RustCrypto-backed algorithms and protected-secret abstractions;
- `torca-tor` — the only owner of Arti and in-process Tor connectivity/onion services;
- peer/link crates — authenticated peer session and transport implementation;
- rendezvous and pairing adapters — concrete pairing relay communication;
- communication adapters — composition of text, receipts, peer health, relationship administration and attachments;
- file/attachment stores — encrypted local attachment state and transfer support;
- logging — redaction-conscious operational logging.

Infrastructure may depend inward on application/domain ports. Application code must not depend outward on infrastructure implementations.

### Platform

`crates/platform` is the outer composition boundary.

`torca-contract` maps presentation-safe commands and read models to the public application facade. It owns serialization/contract compatibility, not business or security policy.

`torca-native` composes the real application, infrastructure and platform services and exposes the process-owned native runtime to Flutter/host code.

`torca-platform`, `torca-platform-windows` and `torca-platform-android` own genuine OS concerns such as application paths, protected secret stores, installation/device information and lifecycle capabilities. OS conditional compilation belongs here rather than leaking through domain/application code.

## Client and runtime ownership

There is one Flutter application source under `apps/client/flutter`. Layout may differ by available screen size, but product workflows are shared.

The Flutter side is responsible for:

- rendering application state;
- navigation and responsive layout;
- transient interaction state such as focus, selection and open dialogs;
- local presentation preferences;
- invoking application intents through `EngineGateway`;
- routing platform-originated links/notification interactions into shared navigation.

Rust is responsible for:

- identities and contact relationships;
- domain identifiers and security-sensitive timestamps;
- pairing state and approval;
- messages, receipts and attachment lifecycle;
- durable queues, retry and deduplication;
- contact verification and security projections;
- Tor/onion lifecycle and peer connectivity;
- background processing, probes and diagnostics;
- encrypted persistence and protected-secret usage.

The native runtime is process-owned. Presentation handles may come and go without implicitly creating independent Torca engines.

## Tor and peer communication

Torca embeds Tor through **Arti** inside the Rust process. Arti imports are intentionally restricted to `torca-tor`; other modules consume Tor capabilities through application/infrastructure interfaces.

The Tor backend owns bootstrap state, client streams and the local onion service. There is no normal dependency on an externally installed Tor executable or Tor Browser.

After pairing, normal communication is direct between the peers' onion endpoints:

```text
outbound user intent
    -> durable local message/outbox
    -> runtime delivery
    -> authenticated peer session
    -> application-layer authenticated encryption
    -> Tor stream
    -> peer onion endpoint
    -> validate/decrypt/deduplicate/persist
    -> protocol acknowledgement + durable receipt flow
```

Network failure does not transfer ownership of unsent data to Flutter. Durable state remains local and workers retry according to application policy.

## Pairing and rendezvous relay

Pairing establishes an explicit relationship between two clients. Invitations may be entered as codes or routed from QR/deep links. Pairing exchanges identity/route/capability material and requires explicit approval before the relationship becomes a contact.

The relay under `services/relay` is an **untrusted, ephemeral rendezvous service**. Its job is to connect two active pairing participants and forward opaque pairing frames. It is not:

- an account or identity provider;
- a contact directory;
- a normal message relay;
- an offline mailbox;
- a conversation history store;
- a central presence service.

Relay degradation may affect creation/joining of new relationships but does not redefine already-established peer communication.

## Persistence and read models

Structured local data is stored through SQLCipher-backed repositories. SQL belongs to the storage infrastructure rather than application/domain source. Migrations and storage epochs are implementation details and should be documented in code/release tooling rather than copied into architecture prose.

Secrets are separate from ordinary relational state. Identity, storage and runtime/peer secret namespaces are supplied through platform-protected secret stores and are referenced by handles where possible.

Conversation history is exposed through bounded paging/search read models instead of requiring the Flutter client to own the entire history. Conversation summaries and security state are projections intended for presentation rather than alternative domain stores.

## Attachments

The UI selects files and asks the application to act on them; it does not become the durable transfer owner. Rust validates attachment intent, manages encrypted private storage/metadata, transfer progress and export/open operations. Attachment communication uses the same peer trust/relationship boundary as messages.

## Connectivity, bootstrap and observability

Startup is represented by an application bootstrap model rather than inferred by Flutter from log strings. It tracks readiness of local/native/storage/Tor/onion/relay/profile concerns and distinguishes ready, degraded, failed and blocked conditions.

Connectivity is tracked through payload-free observations. Runtime transport activity, peer health and probes are designed to provide useful diagnostics without turning observability into a second source of message content.

Notification delivery uses a presentation-safe, cursor-oriented event contract. Notification policy belongs to the domain/application side; Android/Windows code only performs OS-specific delivery and interaction handling.

## Enforced dependency rules

The repository does not rely only on documentation to preserve architecture. `scripts/modules/Torca.ArchitecturePolicy.ps1` and `Torca.SourcePolicy.ps1` enforce important rules during the build path, including:

- domains/protocols may not depend on application, infrastructure or platform layers;
- application may not depend on infrastructure or platform implementations;
- the presentation contract depends on the application facade rather than bypassing it for engine/runtime internals;
- security policy and hashing do not migrate into the serialization contract;
- Arti is imported only by `torca-tor`;
- platform conditionals remain under `crates/platform`;
- SQL/application payload ownership and generated contract boundaries are checked;
- Flutter platform detection and dynamic-library access are kept in their intended boundaries.

If a design change requires breaking one of these rules, change the architecture deliberately rather than adding a silent exception.

## Deployable pieces

Torca currently has two conceptual deployable pieces:

1. **Client** — the shared Flutter/Rust application built for supported platform hosts.
2. **Rendezvous relay** — the small pairing-only service.

The client contains the durable product state. The relay is deliberately disposable and does not become a source of truth for conversations.

## Evolution rule

This document should change when **ownership or boundaries** change. It should not be updated merely because a timeout, schema number, DTO field, exact protocol version or internal class name changes. Those details belong in code, generated contracts, tests and release metadata.