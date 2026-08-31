# Torca architecture

This document is the canonical description of Torca's current system ownership and dependency boundaries. Exact field layouts, crate APIs, timeout constants and protocol bytes remain source/test contracts.

![Torca architecture](docs/diagrams/architecture.svg)

## System context

Torca has one responsive Flutter presentation client and one Rust product/runtime implementation. Windows and Android are hosts of the same application, not separate business implementations.

```text
Flutter widgets/navigation/preferences
             |
       EngineGateway
             |
 generated presentation contract
             |
       torca-native ABI
             |
 process-owned Rust runtime
             |
 application ports + durable state
        /                 \
SQLCipher/crypto/files   communication provider
                              |
                             Iroh
```

Flutter owns presentation. Rust owns product semantics, durability and security-sensitive work. Platform code owns genuine operating-system integration. Concrete providers and repositories implement inward-facing ports; they do not define product meaning.

## Layer model

The Rust workspace is organized by architectural role.

| Layer | Owns | May depend on | Must not own |
| --- | --- | --- | --- |
| `foundation` | dependency-light identifiers, time, cancellation, commands/events, classified errors | minimal shared dependencies | messaging workflows, persistence, provider logic, UI |
| `protocol` | bounded external representations, framing, versioned peer/pairing/radio/attachment contracts | foundation and protocol-level libraries | application orchestration, repositories, OS behavior |
| `domains` | product vocabulary, valid states and invariants | foundation and narrowly required protocol-neutral primitives | SQL, provider SDKs, Flutter/native APIs |
| `application` | use cases, ports, read models, runtime coordination, delivery/pairing/connectivity/power policy | foundation, domains, protocols | concrete SQLCipher/Iroh/OS implementations |
| `infrastructure` | SQLCipher repositories, crypto adapters, files, pairing-service client, peer link and Iroh transport | inward application/domain/protocol contracts | presentation policy or independent product state machines |
| `platform` | generated contract, native process composition, Windows/Android services | application plus concrete infrastructure required at composition | duplicate business workflows |

Repository policy scripts enforce important parts of this dependency direction. A new abstraction is not complete merely because a trait exists; concrete provider/storage/platform types must remain outside upper layers as well.

## Presentation and contract boundary

Flutter owns:

- responsive layout, widgets and navigation;
- transient interaction state;
- presentation preferences such as appearance/locale; and
- translation of user interaction into typed `EngineGateway` intent.

Flutter must not become the owner of durable identifiers, retry/outbox state, pairing completion, peer secrets, provider routing, storage migrations or security decisions.

The generated contract is a serialization/compatibility boundary. It exposes presentation-safe commands, projections, runtime events, diagnostics and capabilities. It is not a second application layer. Production native startup failure remains explicit; the client must not silently replace the Rust runtime with an in-memory business implementation.

## Application/runtime ownership

`torca-client-application` is the presentation-facing application facade. `torca-client-engine` protects consistent durable domain transitions. `torca-runtime` and supporting application crates coordinate long-lived delivery, pairing, connectivity, control delivery, diagnostics, attachments and Radio work through ports.

Runtime scheduling is event/deadline driven. Durable demand, provider events, platform lifecycle events and explicit deadlines wake the relevant owner. Idle Flutter polling must not be required for correctness and must not create unconditional network/CPU work.

Important invariants:

- user intent that requires durability is persisted before transient network success is treated as completion;
- retry ownership remains in Rust;
- provider degradation does not hide usable local encrypted state;
- idle contacts do not justify permanent application-level reconnect/health loops; and
- one process-owned runtime is composed per client process rather than recreated by screens or platform callbacks.

## Communication provider boundary

Application code is provider-neutral. The boundary uses stable provider identity, opaque provider routes/routing metadata and byte-stream/peer-transport ports. Provider-specific endpoint formats, QUIC/Iroh types, relay implementation details and reachability configuration stay in infrastructure/platform composition.

Iroh is the sole production provider. Memory is a deterministic test implementation. Tor and the unfinished WebRTC adapter are retired from the active product graph; see ADR [`0006`](docs/architecture/decisions/0006-IROH-PRODUCTION-PROVIDER.md).

Iroh owns:

- endpoint identity and provider route material;
- route generation/freshness handling;
- incoming stream routing and outgoing dialing;
- direct/relay reachability behavior; and
- provider-specific pairing bootstrap/transport.

The peer-link/application stack above Iroh owns authenticated peer-session semantics and Torca application protocols. Pairing approval, relationship identity, application-layer encryption, receipts, attachments, presence, Radio policy and persistence remain provider-independent.

A future production provider must implement the neutral API, keep its concrete types outside upper layers, define its network-metadata/privacy consequences, and pass provider conformance plus required platform/device evidence.

## Persistence and protected secrets

Structured durable state is stored through SQLCipher-backed infrastructure repositories. Operational SQL belongs under storage infrastructure and is parameterized/bounded; domain/application layers consume repositories and read models rather than raw connections.

Provider endpoints are persisted as opaque provider-owned route data keyed by provider/relationship. Legacy Tor/onion storage is not part of the current application contract. Storage compatibility is guarded by a storage epoch so incompatible historical profiles fail explicitly rather than being silently reinterpreted.

Secret material uses platform-protected stores where available. Private identity keys, relationship secrets and database keys must not cross the presentation DTO/logging boundary.

## Protocol ownership

Versioned peer, pairing, pairing-service, attachment and Radio representations live in protocol crates. Domain aggregates are not serialized directly merely because fields look similar. Protocol-specific versions and byte limits are source/test contracts; long-lived prose should describe ownership and compatibility policy rather than duplicate every constant.

## Platform ownership

Platform/native code owns capabilities that are genuinely host-specific:

- filesystem/application paths;
- protected secret stores;
- lifecycle and background/foreground integration;
- notifications and deep links;
- Android permissions/secure-window behavior;
- Windows window/tray integration; and
- final production composition of application + infrastructure.

Platform adapters may report lifecycle/capabilities into the application, but they do not become a second durable state machine.

## Major flows

The maintained journey-level descriptions are in [`docs/APP-FLOWS.md`](docs/APP-FLOWS.md). Message delivery ownership is also illustrated in [`docs/diagrams/message-delivery.svg`](docs/diagrams/message-delivery.svg).

Across those flows the direction remains:

```text
user intent
  -> application command/use case
  -> durable state when required
  -> infrastructure/provider execution
  -> authenticated remote outcome/event
  -> application projection
  -> Flutter render
```

## Architecture change rules

Update this document when ownership, layer direction, process composition or provider/storage/platform boundaries change. Add or supersede an ADR when a durable architectural decision needs rationale and alternatives preserved.

Do not use dated implementation plans, validation reports or old handoff files as architecture authority. The checked-in source, generated contracts, tests and enforced policies are the final executable contracts when prose and implementation disagree.
