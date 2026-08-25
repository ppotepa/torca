# Torca threat model

This threat model records the current assets, trust boundaries and review triggers for Torca. It is a design/review aid, not a certification. Torca is alpha software and has not received an independent production security audit.

## Scope

The model covers:

- Windows/Android Torca clients;
- Flutter/native/application boundaries;
- local encrypted storage and protected secrets;
- pairing and provider commissioning;
- authenticated peer sessions over selectable communication providers;
- messages, receipts, attachments and Radio Mode;
- notifications, diagnostics and capture surfaces; and
- deployment/runtime lifecycle that can affect availability or privacy.

It does not attempt to model every attack against the OS/hardware/toolchain, global network infrastructure or an intended recipient.

## Assets

Security-sensitive assets include:

- identity private key material;
- database/storage encryption key material;
- relationship/peer secrets and capabilities;
- provider endpoint/signaling secret material;
- message and attachment plaintext;
- Radio control/media plaintext while in use;
- encrypted local history and durable delivery state;
- contact verification/trust state;
- active pairing invitations/bootstrap capabilities; and
- relationship/activity metadata such as timing and peer/provider endpoints.

Availability is also an asset: silent loss or indefinite stalling of durable user work is a security/reliability failure.

## Components and trust assumptions

### Local Torca process

The local process is trusted to handle the user's plaintext and secrets. Rust application/domain code owns durable workflow/security state. Flutter is presentation, not a keystore or durable network workflow owner.

A fully compromised OS/process/account is outside the protections application-layer encryption can provide.

### Platform services

Windows/Android hosts provide paths, protected-secret storage, lifecycle, notification, capture/window and permission integration. The strength of protected storage is bounded by the actual OS/device/account implementation.

Android microphone permission gates local Radio capture. Secure-window behavior reduces accidental capture exposure when strict mode is active, but a development override or compromised OS defeats it.

### Communication provider

Exactly one provider is selected per deployment/session.

- **Tor**: embedded Arti/onion transport and managed pairing rendezvous.
- **Iroh**: QUIC/direct path and direct pairing bootstrap.
- **WebRTC**: adapter/native boundary exists but normal deployment hides it pending host session/signaling implementation.
- **Memory**: simulated/test provider only.

Providers are trusted only to provide their intended transport properties, not as proof of Torca peer identity or payload authenticity. Application-layer authentication/encryption remains mandatory.

Provider metadata differs. Tor is intended to reduce direct peer-address exposure; direct-path providers expose a different network metadata surface. All providers/networks can delay/drop traffic and observe metadata available at their position.

### Pairing commissioning infrastructure

Tor's managed rendezvous service is untrusted for confidentiality and identity. It can observe bounded operational metadata needed to run pairing slots. Direct providers may instead use bootstrap/signaling material without the managed relay.

Commissioning infrastructure is not trusted with durable relationship truth, conversation content/history or long-lived relationship secrets.

### Remote peer

A successfully authenticated paired peer is trusted as the approved holder of the relationship credential. It is not trusted to honor local retention wishes; it can copy messages/files and record Radio audio outside Torca.

## Trust boundary: Flutter -> native/application

Threats:

- presentation forging security-sensitive identifiers/timestamps;
- duplicated product/security state machines in Dart;
- private material leaking into DTOs/logs;
- malformed boundary input; and
- visible-route/lifecycle state becoming required for durable background correctness.

Controls/direction:

- Flutter submits user intent rather than authoritative durable transitions;
- Rust owns security-sensitive identifiers/state;
- generated contract drift is checked;
- boundary input is validated/bounded;
- secret bytes do not belong in presentation DTOs; and
- durable retry/pairing/Radio correctness does not depend on the current route.

## Trust boundary: application -> infrastructure/provider

Threats:

- application coupling to SQL/Tor/QUIC/WebRTC details;
- security policy migrating into serialization/adapters;
- provider-specific state leaking upward and becoming product state; and
- multiple competing timer/supervision frameworks.

Controls/direction:

- application/domain ports define inward contracts;
- source policy rejects outward application dependencies;
- SQL stays in storage infrastructure;
- Arti stays in Tor infrastructure/provider composition;
- provider selection happens once in native composition; and
- shared runtime policy owns attention/demand/evidence/deadline decisions.

## Trust boundary: client -> commissioning service/bootstrap

Threats:

- invitation interception/guessing;
- malicious rendezvous/signaling modification/replay/drop;
- bootstrap endpoint substitution;
- stale/replayed sessions;
- resource exhaustion; and
- user approving the wrong peer.

Controls/direction:

- bounded/short-lived invitation/bootstrap state;
- encrypted/context-bound pairing exchange;
- explicit local approval before durable relationship completion;
- provider-specific commissioning does not replace the durable encrypted offer; and
- input/session resource bounds.

A malicious commissioning service/provider can still deny service and observe metadata available to it.

## Trust boundary: client -> authenticated peer transport

Threats:

- unauthenticated peer attempts;
- replay/duplicate envelopes;
- ciphertext modification/context confusion;
- malicious/oversized protocol input;
- reorder/delay/interruptions;
- reconnect storms/battery exhaustion; and
- network metadata exposure inconsistent with user expectations.

Controls/direction:

- peer authentication bound to established relationship credentials;
- application-layer AEAD with associated context and fresh nonces;
- bounded/versioned framing;
- stable envelope IDs and inbound deduplication;
- durable local outbox/control state;
- explicit receipt/attachment transitions; and
- demand/evidence/deadline-driven connectivity policy.

Transport privacy is provider-dependent. Tor routing does not remove application authentication needs; Iroh's direct path must not inherit Tor privacy claims simply because payload cryptography is shared.

## Radio Mode

Threats:

- capture without intended consent/permission;
- concurrent floor ownership or overlapping transmit sessions;
- replay/context confusion between sessions;
- media continuing after release/background/close;
- provider interruption causing duplicated sessions/reconnect storms; and
- live audio leaking into logs/diagnostics/history.

Controls/direction:

- explicit mutual consent and session state;
- platform microphone permission before capture;
- half-duplex floor/burst invariants and bounded transmit behavior;
- session-specific directional media keys derived in Rust from relationship secret + session context;
- authenticated/encrypted media over a provider-owned Radio route; and
- payload-free/redacted diagnostics.

Tor and Iroh advertise Radio support today. Provider capability must gate presentation/runtime availability.

## Local storage -> process

Threats:

- database theft at rest;
- plaintext/secret persistence outside intended stores;
- migration/state corruption;
- secrets in debug formatting/logs; and
- destructive deploy/reset unintentionally deleting identity/history.

Controls/direction:

- SQLCipher-backed structured storage;
- protected-secret namespaces;
- redacted secret value types/best-effort zeroing;
- transactional repository/workflow operations;
- persistence concentrated in infrastructure; and
- deploy policy separates normal redeploy from destructive reset.

A compromised running process can still access data it legitimately needs.

## Runtime -> diagnostics/notifications

Threats:

- observability exposing plaintext/secrets;
- broad snapshots handed to platform notification code;
- unbounded telemetry becoming a privacy/performance problem; and
- incomplete diagnostics treated as complete evidence.

Controls/direction:

- bounded diagnostics/event ledgers;
- payload-free connectivity/transport activity;
- redacted errors/status;
- cursor-oriented notification events; and
- application privacy policy before OS delivery.

## Pairing and contact verification

Pairing is the transition where a remote identity becomes trusted and deserves stronger review than ordinary navigation.

The current design uses ephemeral X25519 agreement, context/transcript-bound derivation/approval, protected pairing state and explicit relationship completion. Provider bootstrap material only gets participants to the pairing exchange. Safety Number-style verification offers an independent post-pairing comparison path.

Human verification is still necessary when users require assurance beyond possession of the invitation/bootstrap channel.

## Key compromise and forward secrecy

Current peer payload encryption uses protected relationship secret material with fresh nonces. Radio derives session-specific directional keys from that relationship secret.

The scheme does **not** claim Signal-style forward secrecy or post-compromise security. Compromise of a long-lived relationship secret can therefore have consequences beyond one message/Radio session.

A future ratchet/session-key evolution materially changes this boundary and must update both this model and `SECURITY.md` after implementation/review.

## Denial of service and availability

Torca cannot guarantee availability against a blocked/degraded provider/network, unavailable peer, malicious commissioning service, local resource exhaustion or OS suspension.

The client should fail safely/visibly by preserving durable work, separating local readiness from provider reachability, exposing degraded/retry states, bounding queues/input/telemetry, and avoiding unjustified periodic/reconnect activity.

## Out of scope / non-guarantees

This model does not promise protection against:

- fully compromised local endpoints/OS accounts;
- intended recipients copying/recording content;
- all traffic analysis/correlation;
- provider/network/peer denial of service;
- undiscovered implementation vulnerabilities;
- hardware-backed secret storage on every configuration;
- equal metadata/privacy properties across providers; or
- forward secrecy/post-compromise security in the current relationship-key design.

## Review triggers

Revisit this model when any of these change materially:

- pairing/peer cryptographic protocol or key evolution;
- communication provider opened/removed or its commissioning/routing changes;
- provider metadata/privacy assumptions;
- Radio key/media/provider architecture;
- central service responsibilities/persistence;
- platform secret/capture/notification behavior;
- contract exposure of security-sensitive fields;
- new supported platforms/process models;
- groups, calls, multi-device or cloud sync/backup; or
- diagnostics/telemetry begins carrying new user data.