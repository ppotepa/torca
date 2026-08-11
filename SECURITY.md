# Torca security

Torca is security-sensitive alpha software. This document describes the security posture visible in the current codebase and, equally importantly, the guarantees the project does **not** currently claim.

For a structured asset/boundary analysis, see [docs/security/threat-model.md](docs/security/threat-model.md).

## Security goals

Torca is designed to keep normal one-to-one communication independent from a trusted central message service. The current design aims to provide:

- Tor-routed peer connectivity using local onion endpoints;
- explicit contact pairing and approval;
- authenticated peer sessions and application-layer authenticated encryption;
- encrypted structured local storage through SQLCipher;
- platform-protected secret storage separated from ordinary database state;
- durable local delivery/retry and inbound deduplication;
- explicit contact verification/security projections;
- redaction-conscious diagnostics, connectivity telemetry and notification events;
- narrow platform and presentation boundaries that do not require private key material in Flutter.

These properties are architectural goals implemented by the current source, but they are not a substitute for an independent security audit.

## Trust boundaries

### Local device

The local client is trusted with the user's plaintext, local history and key material while it is operating. Torca attempts to protect data at rest and limit secret exposure between layers, but a fully compromised operating system, process, debugger or user account can defeat application-level protections.

Structured history and durable workflow state are stored through SQLCipher-backed repositories. Identity/storage/runtime secrets are separated into protected-secret namespaces supplied by the platform composition.

### Flutter and native boundary

Flutter receives presentation-safe state and sends user intents. Durable workflow policy, secret bytes and cryptographic key ownership remain in Rust. Generated contract checks are used to prevent the presentation schema from silently drifting from the canonical contract.

The contract/native layers serialize application state; they must not become alternative owners of security policy, identity derivation or domain transitions.

### Tor network

Torca uses in-process Arti for Tor connectivity. Normal peer traffic is routed to onion endpoints rather than through the pairing relay. Tor reduces direct network-location exposure between peers, but Tor itself is not treated as an application authentication mechanism: peer identity and payload authentication remain Torca responsibilities.

### Rendezvous relay

The relay is untrusted. It may observe connection timing, slot lifecycle and protocol-level metadata needed to operate the rendezvous service. Clients must not rely on it for identity authenticity, message confidentiality, conversation durability or pairing completion truth.

The relay is not a normal message path and should never require conversation plaintext, private identity keys or stored peer secrets.

### Remote peer

A paired peer is trusted only as the explicitly approved holder of the corresponding relationship identity/capability. A legitimate peer can always see content intentionally sent to it and can copy, screenshot or export that content outside Torca.

## Cryptographic ownership

Production cryptographic primitives are provided by the Rust infrastructure layer. Pairing uses ephemeral key agreement and explicit transcript-bound approval. Long-lived identity and peer secret material is stored behind protected-secret abstractions rather than in Flutter state.

Peer application payloads are authenticated and encrypted with a protected pairwise secret and fresh nonces. Associated context binds encrypted payloads to their intended peer/message context.

Secret-bearing Rust value types use redacted diagnostics and best-effort zeroing where practical. This reduces accidental exposure but does not turn general process memory into a hardware security boundary.

## Contact verification

The application can derive identity fingerprints/Safety Number-style projections for explicit user verification. Verification state is tied to the current remote identity. A change of a previously verified identity is represented as a security-relevant state rather than silently inheriting prior trust.

Verification is meaningful only when users compare the value through an independent trusted channel or in person.

## Local data

SQLCipher is the concrete encrypted database backend. Business SQL is kept in the storage infrastructure and application/domain layers interact through repositories/ports.

Attachments are copied into application-controlled storage and managed by Rust. Temporary plaintext exports are treated differently from encrypted private storage and should be short-lived; explicit user-selected exports are outside Torca's control after they are written.

Platform-protected secret guarantees depend on the actual platform adapter and operating system. Do not document a stronger hardware/biometric guarantee than the selected platform service provides.

## Observability and notifications

Diagnostics, probes, connectivity and transport-activity models are intended to be payload-free or redacted. They should carry states, identifiers, counters and timing only when those values are needed for support/UX.

Notification events use a narrow cursor-oriented projection rather than exposing the general application snapshot to platform notification code. Privacy policy decides whether content may be shown; OS-specific code should not independently fetch private message content.

## Current non-guarantees

The following limitations are intentional to state explicitly:

- Torca has not received an independent production security audit.
- The current pairwise message-key design does **not** claim Signal-style forward secrecy or post-compromise security. Compromise of a long-lived peer secret may have consequences beyond a single message.
- Torca does not protect message content from the intended recipient after delivery.
- A compromised endpoint/OS can access data available to that endpoint.
- Tor does not eliminate all traffic-analysis or timing-correlation risk.
- The pairing relay can observe the metadata required to run an active rendezvous slot even though it is not trusted with plaintext pairing content.
- Availability over Tor is best-effort; network censorship, denial of service and endpoint downtime can delay communication.
- Alpha builds and migration paths should not be treated as long-term archival guarantees unless a release explicitly states otherwise.

Future work may strengthen session-key evolution, endpoint hardening and metadata minimization. Such improvements should use reviewed protocols/primitives rather than inventing custom cryptography merely to obtain a feature label.

## Rules for security-sensitive changes

When changing pairing, peer authentication, encryption, storage, secret handling, notification data or native/platform boundaries:

1. identify the asset and trust boundary being changed;
2. keep cryptographic/security policy out of presentation serialization code;
3. keep private key/peer secret bytes out of Flutter contracts and logs;
4. preserve bounded input parsing and explicit protocol version/size checks;
5. add deterministic negative tests, not only happy-path tests;
6. update the threat model when the trust model or guarantees change;
7. avoid security claims that are stronger than the code and validation evidence.

The source architecture policy already enforces several of these ownership rules and should remain part of normal validation.

## Reporting vulnerabilities

Do not publish live private keys, pairing capabilities, database keys, message contents or other real-user secrets in issues or logs. Report security-sensitive findings privately to the maintainers until an appropriate disclosure channel is formalized.