# Torca threat model

This threat model is intentionally medium-depth. It records the assets and trust boundaries that should remain visible while the implementation evolves, without treating one release's exact protocol/schema layout as permanent.

## Scope

The model covers the Torca client, local storage/secrets, embedded Tor connectivity, direct peer sessions, pairing/rendezvous, the Flutter/native boundary, platform hosts and observability/notification paths.

It does not claim to model every attack against the Tor network, operating system, hardware, compiler/toolchain or intended message recipient.

## Assets

Security-sensitive assets include:

- local identity private key material;
- protected pairwise/relationship secrets and capabilities;
- database encryption key material;
- message and attachment plaintext;
- encrypted local history and durable delivery state;
- contact verification/trust state;
- onion endpoints and pairing invitation capabilities;
- pairing ephemeral state before a relationship is completed;
- metadata that can reveal relationships, communication timing or activity.

Availability is also an asset: a privacy-preserving messenger that silently loses or indefinitely stalls user work is not functioning safely.

## Components and trust assumptions

### Local Torca client

The local Torca process is trusted to handle the user's plaintext and secret material. Rust application/domain code is the main owner of durable workflow and security state. Flutter is treated as a presentation boundary rather than a key store or network workflow owner.

A fully compromised local OS/process/user account is outside the protection that application-level encryption can provide.

### Platform services

Windows/Android hosts provide OS-specific paths, protected-secret storage, lifecycle and UI/notification integration. The strength of at-rest secret protection is therefore bounded by the selected OS adapter and the security of the device account/platform.

### Tor / Arti

Arti provides Tor connectivity and onion-service functionality inside the process. Tor is trusted for its intended network-routing properties but **not** as proof of peer application identity. Torca still authenticates peers and encrypted application payloads.

### Rendezvous relay

The relay is untrusted for confidentiality and identity. It is allowed to observe the operational metadata required to manage ephemeral pairing slots and active connections. Clients must cryptographically protect pairing content and explicitly approve the relationship.

The relay is not trusted with message history or used as the normal message route.

### Remote peer

A successfully authenticated paired peer is trusted as the current holder of the approved relationship identity/capability. It is not trusted to respect local retention/privacy wishes after content is delivered; a recipient can copy content outside the protocol.

## Trust boundaries

### Flutter -> application/native

Threats:

- presentation forging security-sensitive identifiers/timestamps;
- duplicated business/security policy in Dart;
- private material leaking into DTOs or logs;
- malformed strings/lengths crossing FFI.

Controls/direction:

- commands represent user intent;
- Rust/application owns security-sensitive identifiers and durable transitions;
- canonical generated contract is checked for drift;
- FFI/native boundary validates lengths/UTF-8 and exposes presentation-safe read models;
- key material should not cross into Flutter.

### Application -> infrastructure

Threats:

- application becoming coupled to SQL/network/provider details;
- security policy migrating into adapters/serialization;
- infrastructure-specific types spreading upward and making alternate/fake adapters impossible.

Controls/direction:

- application/domain ports define the inward contract;
- source policy rejects application dependencies on infrastructure/platform;
- Arti ownership is restricted to `torca-tor`;
- SQL remains in storage infrastructure.

### Client -> relay

Threats:

- malicious relay modifies/replays/drops pairing frames;
- invitation guessing or slot abuse;
- metadata/timing observation;
- resource exhaustion.

Controls/direction:

- short-lived/bounded invitation/slot state;
- opaque encrypted pairing material;
- explicit approval and identity/transcript verification;
- relay authorization/capability rules and resource bounds;
- relationship completion is client-owned, not relay-owned.

A malicious relay can still deny service and observe when clients use the rendezvous service.

### Client -> peer over Tor

Threats:

- unauthenticated peer attempts;
- replay/duplicate application envelopes;
- ciphertext modification or context confusion;
- reordered/delayed deliveries and receipts;
- connection interruption causing message loss;
- malicious/oversized protocol input.

Controls/direction:

- peer authentication bound to established contact/credentials;
- application-layer AEAD with associated context;
- fresh nonces;
- bounded versioned protocol framing;
- stable envelope identifiers and inbound deduplication;
- durable local outbox/control work and retry/recovery;
- explicit message/receipt state transitions.

Tor hides the direct IP path but does not remove application-layer authentication requirements or all timing-correlation risk.

### Local storage -> process

Threats:

- database theft while the application is not running;
- accidental plaintext/secret persistence;
- migration/state corruption;
- secrets exposed through debug/log formatting.

Controls/direction:

- SQLCipher-backed structured storage;
- separate protected-secret stores/namespaces;
- redacted secret types and best-effort zeroing;
- transactional durable workflow/storage operations;
- explicit schema/storage migration code;
- SQL/persistence concentrated in infrastructure.

A compromised running process can still access data it legitimately needs to use.

### Runtime -> diagnostics/connectivity/notifications

Threats:

- observability becoming a side channel for plaintext or secrets;
- notification platform code receiving full application snapshots;
- unbounded telemetry creating privacy/performance problems.

Controls/direction:

- bounded diagnostics/event ledgers;
- payload-free connectivity and transport activity;
- redacted error/status models;
- cursor-oriented notification events with narrow content;
- domain/application notification privacy policy before OS delivery.

## Pairing threats

Pairing is the moment when a new remote identity becomes trusted, so it deserves stronger review than ordinary UI flows.

Relevant threats include invitation interception/guessing, relay tampering, substitution of identity/route/capability data, stale/replayed sessions and users approving the wrong peer.

The current design uses ephemeral pairing key agreement, protected pairing state, transcript-bound approval and explicit local/remote approval before durable relationship completion. Safety Number/fingerprint-style verification provides an independent way to compare identities after pairing.

Human verification remains necessary if the user wants assurance beyond possession of the pairing invitation/channel.

## Key compromise and forward secrecy

Current peer payload encryption uses a protected pairwise secret with fresh nonces. This provides authenticated encryption when the secret remains confidential, but the present scheme does **not** claim Signal-style forward secrecy or post-compromise security.

A future ratchet/session-key evolution design would change this threat boundary significantly and must update both this document and `SECURITY.md` after implementation and review.

## Denial of service and availability

Torca cannot guarantee availability against a blocked Tor network, unavailable peer, malicious relay, local resource exhaustion or OS suspension.

The client should fail safely and visibly:

- preserve durable user work locally where appropriate;
- distinguish local readiness from network readiness;
- expose degraded/retry states instead of losing data;
- use bounded queues, input sizes and telemetry;
- avoid making the pairing relay a single point of failure for established conversations.

## Out of scope / non-guarantees

This model does not promise protection against:

- a fully compromised local endpoint or OS account;
- an intended recipient copying or externally recording content;
- global traffic analysis or all Tor correlation attacks;
- denial of service by network/peer/relay operators;
- undiscovered implementation vulnerabilities;
- hardware-backed secret storage on every possible platform/configuration;
- forward secrecy/post-compromise security in the current pairwise payload scheme.

Torca is alpha software and has not been independently audited.

## Review triggers

Revisit this threat model when any of the following changes:

- peer/pairing cryptographic protocol or key evolution;
- relay responsibilities or persistence;
- a central service gains access to normal message metadata/content;
- new multi-device/group/call architecture is introduced;
- platform secret-storage model changes materially;
- notification/telemetry starts carrying new user data;
- contract boundary begins exposing new security-sensitive fields;
- a new supported platform changes process/lifecycle trust assumptions;
- local/cloud backup or sync is introduced.