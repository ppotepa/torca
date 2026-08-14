# Torca architecture

This document describes the stable shape of the current Torca codebase. It intentionally stays above individual methods, schema numbers, protocol versions and release-specific migration details. When this document and the code disagree, the active workspace and enforced architecture policies are authoritative and the documentation should be corrected.

## Architectural intent

Torca is a modular Rust application behind one responsive Flutter client. Windows and Android are platform hosts of the same product rather than separate business implementations.

The central rule is that presentation expresses **user intent** and renders **application read models**. Durable state, identity, cryptography, Tor, pairing, delivery/retry, Radio Mode policy and background lifecycle remain inside Rust.

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

`crates/domains` owns product semantics and invariants for identity, contacts, conversations, pairing, messaging, receipts, attachments, presence, notifications and Radio Mode.

Domains describe **what a valid Torca state means**. They do not know about Flutter, SQLCipher, Arti, sockets, JNI, Win32 or Android APIs.

Presence is derived from observed connection/activity facts rather than a central presence service. Notification policy is modeled independently from Android/Windows notification APIs. Radio Mode models mutual consent, session/floor state and burst invariants independently from concrete audio/network adapters.

### Protocols

`crates/protocol` owns bounded wire formats and protocol validation for peer, pairing, relay, attachments, Radio Mode and generic framing concerns. Domain objects are not serialized directly merely because their fields look similar to a wire representation.

Protocol crates are below application/infrastructure orchestration and remain free of platform behavior.

### Application

`crates/application` coordinates use cases and defines the interfaces infrastructure must satisfy.

The main presentation-facing boundary is `torca-client-application`. It exposes application commands, queries/read models, security projections and policy. Presentation code should not reconstruct application state by querying unrelated repositories or transports.

`torca-client-engine` remains the single-writer consistency boundary for durable domain transitions.

`torca-runtime` is the long-lived background owner for network-facing work. It coordinates Tor state, pairing, peer connectivity, durable message/control delivery, attachments, Radio Mode, connectivity observations and diagnostics through application-defined driver interfaces.

Supporting application crates isolate concerns such as bootstrap state, connectivity, runtime policy, probing, delivery/control delivery, pairing coordination, radio coordination and diagnostics. These are modules of one client application, not separately deployed services.

`torca-runtime-policy` owns dependency-light decisions around attention, demand leases, health evidence, deadlines and event waiting. It does not own sockets, SQLCipher, Flutter, JNI, Android APIs or Arti execution.

### Infrastructure

`crates/infrastructure` contains concrete adapters for application/domain ports.

Important owners include:

- `torca-storage-sqlite` — SQLCipher-backed repositories, durable queues, read state, security projections and settings;
- `torca-crypto` — RustCrypto-backed algorithms and protected-secret abstractions;
- `torca-tor` — the only owner of Arti and in-process Tor connectivity/onion services;
- peer/link crates — authenticated peer session and transport implementation;
- rendezvous/pairing adapters — concrete pairing relay communication;
- communication adapters — text, receipts, peer health, relationship administration and attachments;
- file/attachment stores — encrypted local attachment state and transfer support;
- radio adapters — Radio Mode control/media I/O, audio capture/playback and session transport; and
- logging — redaction-conscious operational logging.

Infrastructure may depend inward on application/domain ports. Application code must not depend outward on infrastructure implementations.

### Platform

`crates/platform` is the outer composition boundary.

`torca-contract` maps presentation-safe commands/read models to the public application facade. It owns serialization and contract compatibility, not business or security policy.

`torca-native` composes the real application, infrastructure and platform services and exposes the process-owned native runtime to Flutter/host code.

`torca-platform`, `torca-platform-windows` and `torca-platform-android` own genuine OS concerns such as application paths, protected secret stores, installation/device information, lifecycle, notifications, microphone permission and capture/window integration. OS conditional compilation belongs here rather than leaking through domain/application code.

## Client and runtime ownership

There is one Flutter application source under `apps/client/flutter`. Layout may differ by available screen size, but product workflows are shared.

Flutter is responsible for:

- rendering application state;
- navigation and responsive layout;
- transient interaction state such as focus, selection and open dialogs;
- local presentation preferences;
- invoking application intents through `EngineGateway`; and
- routing platform-originated links/notification interactions into shared navigation.

Rust is responsible for:

- identities and contact relationships;
- domain identifiers and security-sensitive timestamps;
- pairing state and approval;
- messages, receipts and attachment lifecycle;
- durable queues, retry and deduplication;
- contact verification and security projections;
- Radio Mode consent/session/floor and media-key ownership;
- Tor/onion lifecycle and peer connectivity;
- background processing, policy/deadlines and diagnostics; and
- encrypted persistence and protected-secret usage.

The native runtime is process-owned. Presentation handles may come and go without implicitly creating independent Torca engines or Tor clients.

## Tor and peer communication

Torca embeds Tor through **Arti** inside the Rust process. Arti imports are restricted to `torca-tor`; other modules consume Tor capabilities through application/infrastructure interfaces.

The Tor backend owns bootstrap state, client streams and local onion-service publication. There is no normal dependency on an externally installed Tor executable or Tor Browser.

After pairing, normal communication is direct between peers' onion endpoints:

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

Health/reconnect scheduling should use real transport evidence and explicit demand rather than unconditional UI polling or one timer framework per feature. Cosmetic connectivity state must not become a correctness dependency for durable local work.

## Pairing and rendezvous relay

Pairing establishes an explicit relationship between two clients. Invitations may be entered as codes or routed from QR/deep links. Pairing exchanges identity/route/capability material and requires explicit approval before the relationship becomes a contact.

The relay under `services/relay` is an **untrusted, ephemeral rendezvous service**. Its job is to connect active pairing participants and forward opaque pairing frames. It is not:

- an account or identity provider;
- a contact directory;
- a normal message relay/offline mailbox;
- a conversation-history store; or
- a central presence service.

Relay degradation may affect creation/joining of new relationships but does not redefine already-established peer communication.

## Persistence and read models

Structured local data is stored through SQLCipher-backed repositories. SQL belongs to storage infrastructure rather than application/domain source. Migrations and storage epochs are implementation details and should remain in code/release tooling rather than duplicated in architecture prose.

Secrets are separate from ordinary relational state. Identity, storage and runtime/peer secret namespaces are supplied through platform-protected secret stores and referenced by handles where practical.

Conversation history is exposed through bounded paging/search read models rather than requiring Flutter to own the complete history. Conversation summaries, security state, connectivity, notifications and Radio Mode state are presentation projections, not alternative sources of truth.

## Attachments

The UI selects files and asks the application to act on them; it does not become the durable transfer owner. Rust validates attachment intent, manages encrypted private storage/metadata, transfer progress and export/open operations. Attachment communication uses the same paired-peer trust boundary as messages.

## Radio Mode

Radio Mode is a mutual-consent, half-duplex peer feature. Domain/application code owns consent, session state, floor/burst rules and durable user-visible state. Infrastructure owns audio capture/playback and transport execution. Platform code owns microphone permission and OS lifecycle integration.

Media uses session-specific directional keys derived in Rust from protected relationship secret material plus session context. Radio Mode does not create a central media service and does not change the project's current non-guarantee around forward secrecy/post-compromise security.

## Connectivity, bootstrap and observability

Startup is represented by application/bootstrap state rather than inferred by Flutter from log strings. Local readiness is distinct from Tor/onion/relay reachability so network degradation does not unnecessarily hide local encrypted state.

Runtime waiting is event/deadline driven: durable work, network events, demand leases and executor deadlines should wake background work; idle presentation polling should not create periodic application-controlled network/CPU activity.

Connectivity observations and the runtime energy ledger are payload-free operational projections. They support diagnostics/performance work without becoming a second source of message, attachment or Radio Mode content.

Notification delivery uses a presentation-safe cursor/event contract. Notification policy belongs to domain/application; Android/Windows code performs OS-specific delivery and interaction handling.

## Enforced dependency rules

The repository does not rely only on prose to preserve architecture. `scripts/modules/Torca.ArchitecturePolicy.ps1` and `Torca.SourcePolicy.ps1` enforce important rules, including:

- domains/protocols may not depend on application, infrastructure or platform layers;
- application may not depend on infrastructure or platform implementations;
- the presentation contract depends on the application facade rather than bypassing it for engine/runtime internals;
- security policy/hashing do not migrate into serialization code;
- Arti is imported only by `torca-tor`;
- platform conditionals stay under platform boundaries;
- SQL/application payload ownership and generated contract boundaries are checked; and
- Flutter platform/dynamic-library access stays in intended boundaries.

If a design change requires breaking one of these rules, change the architecture deliberately and update the policy/documentation rather than adding a silent exception.

## Deployable pieces

Torca currently has two conceptual deployable pieces:

1. **Client** — the shared Flutter/Rust application built for supported platform hosts.
2. **Rendezvous relay** — the small pairing-only service.

The client contains durable product state. The relay is deliberately disposable and does not become a source of truth for conversations or Radio Mode.

## Evolution rule

Update this document when **ownership, trust boundaries or dependency direction** change. Do not update it merely because a timeout, schema number, DTO field, exact protocol version, test count or internal class name changes. Those details belong in code, generated contracts, tests, release metadata and focused engineering ledgers.