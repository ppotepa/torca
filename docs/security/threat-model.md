# Torca threat model

This threat model records the assets and trust boundaries that should remain visible while Torca evolves. It intentionally avoids treating one release's exact protocol/schema layout as permanent.

Torca is alpha software and has not received an independent production security audit. This model is a design/review tool, not a certification.

## Scope

The model covers the Torca client, local storage/secrets, embedded Tor connectivity, direct peer sessions, pairing/rendezvous, attachments, Radio Mode, the Flutter/native boundary, platform hosts and observability/notification/capture paths.

It does not attempt to model every attack against the Tor network, operating system, hardware, compiler/toolchain or intended message recipient.

## Assets

Security-sensitive assets include:

- local identity private key material;
- protected pairwise/relationship secrets and capabilities;
- database/storage encryption key material;
- message and attachment plaintext;
- Radio Mode control/media plaintext while in use;
- encrypted local history and durable delivery state;
- contact verification/trust state;
- onion endpoints and pairing invitation capabilities;
- pairing ephemeral state before a relationship is completed; and
- metadata that can reveal relationships, communication timing or activity.

Availability is also an asset: a privacy-preserving messenger that silently loses or indefinitely stalls user work is not functioning safely.

## Components and trust assumptions

### Local Torca client

The local Torca process is trusted to handle the user's plaintext and secret material. Rust application/domain code is the main owner of durable workflow and security state. Flutter is a presentation boundary rather than a key store or network-workflow owner.

A fully compromised local OS/process/user account is outside the protection that application-level encryption can provide.

### Platform services

Windows/Android hosts provide OS-specific paths, protected-secret storage, lifecycle, notification, capture/window and permission integration. The strength of at-rest secret protection is bounded by the selected OS adapter and the security of the device/account.

Android microphone permission gates local Radio Mode capture. OS-level screen-capture protection reduces accidental capture exposure when strict mode is active, but a deliberate development override or a compromised OS defeats that protection.

### Tor / Arti

Arti provides Tor connectivity and onion-service functionality inside the process. Tor is trusted for its intended routing properties but **not** as proof of peer application identity. Torca still authenticates peers and encrypted application payloads.

### Rendezvous relay

The relay is untrusted for confidentiality and identity. It may observe operational metadata required to manage ephemeral pairing slots and active connections. Clients must cryptographically protect pairing content and explicitly approve the relationship.

The relay is not trusted with message history, attachment content, Radio Mode media or long-lived peer secrets and is not used as the normal message route.

### Remote peer

A successfully authenticated paired peer is trusted as the current holder of the approved relationship identity/capability. It is not trusted to respect local retention/privacy wishes after content is delivered; a recipient can copy messages/files and record Radio Mode audio outside Torca.

## Trust boundaries

### Flutter -> application/native

Threats:

- presentation forging security-sensitive identifiers/timestamps;
- duplicated business/security policy in Dart;
- private material leaking into DTOs/logs;
- malformed strings/lengths crossing FFI; and
- presentation state incorrectly becoming a correctness dependency for durable/background work.

Controls/direction:

- commands represent user intent;
- Rust/application owns security-sensitive identifiers and durable transitions;
- canonical generated contract is checked for drift;
- FFI/native validates boundary input and exposes presentation-safe read models;
- key material must not cross into Flutter; and
- durable delivery/pairing/Radio correctness is independent of the currently visible Flutter route.

### Application -> infrastructure

Threats:

- application becoming coupled to SQL/network/provider details;
- security policy migrating into adapters/serialization;
- infrastructure-specific types spreading upward and making controlled/fake adapters impossible; and
- multiple timer/supervision frameworks creating conflicting ownership.

Controls/direction:

- application/domain ports define the inward contract;
- source policy rejects application dependencies on infrastructure/platform;
- Arti ownership is restricted to `torca-tor`;
- SQL remains in storage infrastructure; and
- shared runtime policy owns attention/demand/evidence/deadline decisions while executors own concrete I/O.

### Client -> relay

Threats:

- malicious relay modifies/replays/drops pairing frames;
- invitation guessing or capability abuse;
- metadata/timing observation; and
- resource exhaustion.

Controls/direction:

- short-lived/bounded invitation/slot state;
- opaque encrypted pairing material;
- explicit approval and transcript/context binding;
- relay authorization/capability rules and resource bounds; and
- client-owned relationship completion.

A malicious relay can still deny service and observe when clients use rendezvous.

### Client -> peer over Tor

Threats:

- unauthenticated peer attempts;
- replay/duplicate application envelopes;
- ciphertext modification or context confusion;
- reordered/delayed deliveries and receipts;
- interruption causing message/attachment/Radio failure;
- malicious/oversized protocol input; and
- reconnect/probe storms that waste battery or reduce availability.

Controls/direction:

- peer authentication bound to established relationship credentials;
- application-layer AEAD with associated context and fresh nonces;
- bounded versioned protocol framing;
- stable envelope identifiers and inbound deduplication;
- durable local outbox/control work and retry/recovery;
- explicit message/receipt/attachment state transitions; and
- demand/evidence/deadline-based connectivity policy with bounded single-flight work.

Tor hides the direct IP path but does not remove application-layer authentication requirements or all timing-correlation risk.

### Radio Mode media

Threats:

- microphone capture without intended consent/permission;
- concurrent floor ownership or overlapping transmit sessions;
- replay/context confusion across Radio sessions;
- media continuing after release/background/session close;
- peer/network interruption creating duplicate sessions or reconnect storms; and
- live audio leaking into logs/diagnostics/history.

Controls/direction:

- explicit per-contact mutual consent and session state;
- platform microphone permission before capture;
- half-duplex floor/burst invariants and bounded transmit duration;
- session-specific directional media keys derived in Rust from protected relationship secret plus session context;
- authenticated encrypted media frames over the paired peer boundary; and
- diagnostics that count/state-track Radio work without carrying audio payloads.

Session-specific derivation provides separation between Radio sessions but does not create Signal-style forward secrecy or post-compromise recovery for the underlying relationship secret.

### Local storage -> process

Threats:

- database theft while the application is not running;
- accidental plaintext/secret persistence;
- migration/state corruption;
- secrets exposed through debug/log formatting; and
- destructive reset/deploy workflows unintentionally removing identity/history.

Controls/direction:

- SQLCipher-backed structured storage;
- separate protected-secret stores/namespaces;
- redacted secret types and best-effort zeroing;
- transactional durable workflow/storage operations;
- explicit schema/storage migration code;
- SQL/persistence concentrated in infrastructure; and
- deployment policies that distinguish ordinary redeploy from destructive client reset.

A compromised running process can still access data it legitimately needs to use.

### Runtime -> diagnostics/connectivity/notifications

Threats:

- observability becoming a side channel for plaintext/secrets;
- notification code receiving broad application snapshots;
- unbounded telemetry creating privacy/performance problems; and
- partial/stale diagnostic collection being mistaken for complete evidence.

Controls/direction:

- bounded diagnostics/event ledgers;
- payload-free connectivity/transport activity;
- redacted error/status models;
- cursor-oriented notification events with narrow content;
- application privacy policy before OS delivery; and
- fresh incident directories whose manifest does not by itself imply a complete payload.

## Pairing threats

Pairing is the moment a new remote identity becomes trusted, so it deserves stronger review than ordinary UI flows.

Relevant threats include invitation interception/guessing, relay tampering, substitution of identity/route/capability data, stale/replayed sessions and users approving the wrong peer.

The current design uses ephemeral X25519 key agreement, context/transcript-bound derivation/approval, protected pairing state and explicit local/remote approval before durable relationship completion. Safety Number-style verification provides an independent way to compare identities after pairing.

Human verification remains necessary when users need assurance beyond possession of the pairing invitation/channel.

## Key compromise and forward secrecy

Current peer payload encryption uses protected pairwise relationship secret material with fresh nonces. Radio Mode derives session-specific directional keys from that relationship secret.

The current scheme does **not** claim Signal-style forward secrecy or post-compromise security. Compromise of the long-lived relationship secret can therefore have consequences beyond one message or Radio session.

A future ratchet/session-key evolution design changes this threat boundary materially and must update both this document and `SECURITY.md` after implementation and review.

## Denial of service and availability

Torca cannot guarantee availability against a blocked Tor network, unavailable peer, malicious relay, local resource exhaustion or OS suspension.

The client should fail safely and visibly:

- preserve durable user work locally where appropriate;
- distinguish local readiness from network readiness;
- expose degraded/retry states instead of losing data;
- use bounded queues, input sizes and telemetry;
- avoid making the pairing relay a single point of failure for established conversations; and
- avoid periodic/reconnect work that is not justified by durable demand or real transport evidence.

## Out of scope / non-guarantees

This model does not promise protection against:

- a fully compromised local endpoint or OS account;
- an intended recipient copying or externally recording content;
- global traffic analysis or all Tor correlation attacks;
- denial of service by network/peer/relay operators;
- undiscovered implementation vulnerabilities;
- hardware-backed secret storage on every supported platform/configuration; or
- forward secrecy/post-compromise security in the current pairwise payload scheme.

## Review triggers

Revisit this threat model when any of the following changes materially:

- peer/pairing cryptographic protocol or key evolution;
- Radio Mode key/session/media architecture;
- relay responsibilities or persistence;
- a central service gains access to normal message metadata/content;
- multi-device/group/call architecture is introduced;
- platform secret-storage or capture/privacy model changes;
- notification/telemetry starts carrying new user data;
- the contract boundary exposes new security-sensitive fields;
- a new supported platform changes process/lifecycle trust assumptions; or
- local/cloud backup or synchronization is introduced.