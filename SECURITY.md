# Torca security

Torca is security-sensitive alpha software. This document describes the security posture visible in the current codebase and, equally importantly, the guarantees the project does **not** claim.

For a structured asset/boundary analysis, see [`docs/security/threat-model.md`](docs/security/threat-model.md). For the concise project maturity/validation summary, see [`docs/STATUS.md`](docs/STATUS.md).

Torca has not received an independent production security audit. Passing source/build/test gates is useful engineering evidence but must not be presented as an external security certification.

## Security goals

The current design aims to provide:

- Tor-routed peer connectivity using local onion endpoints;
- explicit contact pairing and approval;
- authenticated peer sessions and application-layer authenticated encryption;
- SQLCipher-backed encrypted structured storage;
- platform-protected secret storage separated from ordinary database state;
- durable local delivery/retry and inbound deduplication;
- explicit contact verification/security projections;
- redaction-conscious diagnostics, connectivity telemetry and notification events;
- narrow platform/presentation boundaries that do not require private key material in Flutter; and
- mutually consented Radio Mode whose per-session directional media keys are derived in Rust from protected relationship secret material.

These are implementation/security goals, not a substitute for independent review.

## Trust boundaries

### Local device

The local client is trusted with the user's plaintext, local history and key material while it is operating. Torca attempts to protect data at rest and limit secret exposure between layers, but a fully compromised operating system, process, debugger or user account can defeat application-level protections.

Structured history and durable workflow state are stored through SQLCipher-backed repositories. Identity/storage/runtime/peer secret material is separated into protected-secret namespaces supplied by platform composition.

### Flutter and native boundary

Flutter receives presentation-safe state and sends user intent. Durable workflow policy, secret bytes and cryptographic key ownership remain in Rust. Generated contract checks are used to prevent the presentation schema from silently drifting from the canonical contract.

The contract/native layers serialize application state and compose the process; they must not become alternative owners of security policy, identity derivation, cryptography or durable domain transitions.

### Tor network

Torca uses in-process Arti for Tor connectivity. Normal peer traffic is routed to onion endpoints rather than through the pairing relay. Tor reduces direct network-location exposure between peers, but Tor itself is not treated as application identity/authentication: peer identity and payload authentication remain Torca responsibilities.

### Rendezvous relay

The pairing relay is untrusted for confidentiality and identity. It may observe connection timing, slot lifecycle and protocol metadata needed to operate rendezvous. Clients must not rely on it for message confidentiality, relationship authenticity, conversation durability or pairing-completion truth.

The relay is not the normal message path and should never require conversation plaintext, Radio Mode media, private identity keys or stored peer secrets.

### Remote peer

A paired peer is trusted only as the explicitly approved holder of the corresponding relationship identity/capability. A legitimate peer can see content intentionally sent to it and can copy, screenshot, export or record that content outside Torca.

## Cryptographic ownership

Production cryptographic primitives are provided by the Rust infrastructure layer using established libraries.

Current building blocks include:

- X25519 ephemeral key agreement during pairing;
- HKDF-SHA256 for context-bound key derivation;
- Ed25519 signatures;
- XChaCha20-Poly1305 authenticated encryption; and
- operating-system CSPRNG-backed randomness.

Pairing derives protected relationship secret material from ephemeral key agreement and transcript/context material. Peer application payloads are authenticated/encrypted with protected pairwise secret material, fresh nonces and associated context.

Radio Mode derives session-specific directional media keys from the protected pairwise relationship secret plus session/media context. The derived session cipher is kept in memory and its key containers use best-effort zeroing. This provides session separation, but it does **not** turn the underlying relationship design into a forward-secret ratchet.

Secret-bearing Rust value types use redacted diagnostics and best-effort zeroing where practical. This reduces accidental exposure but does not create a hardware/process-memory security boundary.

## Contact verification

The application derives Safety Number-style identity projections for explicit user verification. Verification state is tied to the current remote identity. A change to a previously verified identity is represented as a security-relevant state rather than silently inheriting prior trust.

Verification is meaningful only when users compare the value through an independent trusted channel or in person.

## Local data and protected secrets

SQLCipher is the concrete encrypted structured-storage backend. Business SQL is kept in storage infrastructure and application/domain layers interact through repositories/ports.

Attachments are copied into application-controlled storage and managed by Rust. Temporary plaintext exports are distinct from encrypted private storage and should be short-lived; explicit user-selected exports are outside Torca's control after they are written.

Platform-protected secret guarantees depend on the selected operating-system adapter and device/account configuration. Do not document a stronger hardware/biometric guarantee than the actual platform service provides.

## Observability, notifications and capture surfaces

Diagnostics, probes, connectivity and transport-activity models are intended to be payload-free or redacted. They should carry states, identifiers, counters and timing only when needed for support/UX. They must not contain message/attachment plaintext, Radio Mode audio, private keys, pairwise secrets or pairing capabilities.

Notification events use a narrow cursor-oriented projection rather than exposing the general application snapshot to platform notification code. Privacy policy decides whether content may be shown; OS-specific code should not independently fetch private message content.

Android blocks screenshots/screen recording through the OS secure-window flag by default. The deployment tool has an explicit development override to allow capture. That override changes the capture surface only; it is not a cryptographic or transport mode. Test/debug documentation must not describe an override-enabled build as capture-protected.

## Current non-guarantees

The following limitations must remain explicit:

- Torca has not received an independent production security audit.
- The current pairwise message-key design does **not** claim Signal-style forward secrecy or post-compromise security.
- Radio Mode session separation does not provide a Double Ratchet/MLS-style compromise-recovery guarantee.
- Torca does not protect content from the intended recipient after delivery/playback.
- A compromised endpoint/OS can access data available to that endpoint.
- Tor does not eliminate all traffic-analysis or timing-correlation risk.
- The pairing relay can observe the operational metadata required to run an active rendezvous slot.
- Availability over Tor is best-effort; censorship, denial of service, endpoint downtime and OS suspension can delay communication.
- Alpha builds and migration paths should not be treated as long-term archival guarantees unless a release explicitly states otherwise.

Future work may strengthen session/message-key evolution, endpoint hardening and metadata minimization. Security-critical protocol changes should use reviewed designs/primitives rather than inventing custom cryptography merely to obtain a feature label.

## Rules for security-sensitive changes

When changing pairing, peer authentication, encryption, protected-secret handling, storage, Radio Mode, notifications, diagnostics or native/platform privacy boundaries:

1. identify the asset and trust boundary being changed;
2. keep cryptographic/security policy out of presentation serialization code;
3. keep private key/peer secret bytes out of Flutter contracts and logs;
4. preserve bounded input parsing and explicit protocol version/size checks;
5. add deterministic negative/failure tests, not only happy-path tests;
6. update the threat model when the trust model or guarantees change;
7. update `PRIVACY.md` when user data handling or OS permissions/capture behavior changes; and
8. avoid security claims stronger than the code and validation evidence.

Repository architecture/source policy already enforces several ownership rules and should remain part of normal validation.

## Reporting vulnerabilities

Do not publish live private keys, pairing capabilities, database keys, relationship secrets, message/attachment plaintext, Radio Mode audio or other real-user secrets in issues, logs or screenshots.

For a potentially sensitive vulnerability, contact the repository maintainers through a private channel available from their GitHub profile or an existing trusted contact path. Until a dedicated vulnerability-disclosure channel is published, if private contact is not available, open only a minimal public coordination issue that contains **no exploit details or secrets** and asks the maintainers to establish a private disclosure path.

A future formal disclosure channel should be added here when available. Do not invent or document an address/process that has not actually been established.