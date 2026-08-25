# Torca security

Torca is security-sensitive alpha software. This document describes the security posture visible in the current source and the guarantees the project does **not** claim.

For structured asset/boundary analysis, read [`docs/security/threat-model.md`](docs/security/threat-model.md). For current implementation/provider status, read [`docs/STATUS.md`](docs/STATUS.md).

Torca has not received an independent production security audit. Passing tests/builds is engineering evidence, not an external security certification.

## Security goals

The current design aims to provide:

- explicit contact pairing and local approval;
- authenticated peer sessions independent of the selected network provider;
- application-layer authenticated encryption for peer payloads;
- SQLCipher-backed encrypted structured storage;
- protected secret storage separated from ordinary database/presentation state;
- durable local delivery/retry and inbound deduplication;
- explicit contact verification/security projections;
- redaction-conscious diagnostics and narrow notification events;
- private key/relationship secret ownership below the Flutter boundary; and
- mutual-consent Radio Mode with session-specific directional media keys derived in Rust.

Communication-provider privacy is **not** one uniform guarantee. Tor and Iroh are both selectable providers, but they expose different network metadata. Tor is intended to reduce direct network-location exposure through onion routing. Iroh is a direct-path QUIC provider and should not be described as providing Tor's network-location-hiding properties.

## Trust boundaries

### Local device

The local client is trusted with plaintext and secret material while operating. Torca protects data at rest and narrows secret ownership between layers, but a fully compromised OS, process, debugger or user account can defeat application-level protections.

Structured history/durable workflow state is SQLCipher-backed. Identity, storage, relationship and provider secret material is supplied through protected-secret namespaces/platform services.

### Flutter/native boundary

Flutter receives presentation-safe state and submits typed user intent. Durable workflow policy, cryptographic key ownership, provider composition and security-sensitive state remain in Rust.

Generated contract checks reduce schema drift. The native/contract boundary must not become an alternative implementation of product security policy.

### Communication provider

A deployment selects exactly one provider. Provider selection changes commissioning/reachability and byte transport, but does not replace Torca peer authentication or application-layer encryption.

Current normal selectable providers:

- **Tor** — embedded Arti/onion transport plus managed pairing rendezvous.
- **Iroh** — QUIC/direct-path transport plus direct pairing bootstrap.

WebRTC and memory adapters exist but are hidden from normal deployment today.

A provider or underlying network can observe the metadata available at its position and can delay/drop traffic. Tor routing reduces some direct-address exposure but does not eliminate timing correlation, censorship or denial-of-service risk. Direct-path transports have a different metadata surface and may expose peer/network addressing information to network participants that Tor would route differently.

### Pairing commissioning service

Tor pairing uses an untrusted managed rendezvous service. It may observe connection timing, slot lifecycle and protocol metadata needed to operate rendezvous. It must not be trusted for message confidentiality, relationship authenticity, conversation durability or pairing-completion truth.

Direct providers can use provider-owned bootstrap/signaling instead of this service. Bootstrap descriptors/tickets are still sensitive capabilities and should be short-lived/bounded; they do not replace the encrypted pairing exchange and explicit approval.

### Remote peer

A successfully paired/authenticated peer is trusted only as the approved holder of the relationship credential. It can read intentionally delivered content and can copy, screenshot, export or record it outside Torca.

## Cryptographic ownership

Production cryptographic primitives live in Rust infrastructure/application boundaries using established libraries.

Current building blocks include:

- X25519 ephemeral key agreement during pairing;
- HKDF-SHA256 context-bound key derivation;
- Ed25519 signatures;
- XChaCha20-Poly1305 authenticated encryption; and
- OS/CSPRNG-backed randomness.

Pairing derives protected relationship secret material from ephemeral key agreement and transcript/context material. Peer application payloads use protected relationship secret material with fresh nonces and associated context.

Radio Mode derives session-specific directional media keys from the protected relationship secret plus session/media context. This separates Radio sessions but does **not** turn the underlying relationship into a Signal Double Ratchet/MLS-style key schedule.

Secret-bearing Rust values use redacted diagnostics/best-effort zeroing where practical. This reduces accidental leakage; it is not a hardware or process-memory isolation guarantee.

## Contact verification

Torca derives Safety Number-style identity projections for explicit verification. Verification is tied to the current remote identity. An identity change after verification is security-relevant state rather than silently inheriting prior trust.

Verification matters only when compared through an independent trusted channel or in person.

## Local data and protected secrets

SQLCipher is the encrypted structured-storage backend. Business SQL stays in infrastructure; application/domain code works through repositories/ports.

Attachments are imported into application-controlled storage. Temporary/open/exported plaintext copies are distinct from private encrypted storage and leave Torca's control once written to a user-selected/external destination.

Platform-protected secret guarantees depend on the actual Windows/Android adapter and device/account configuration. Do not claim hardware/biometric protection unless the concrete platform path provides it.

## Observability, notifications and capture surfaces

Diagnostics/connectivity/transport activity should remain payload-free or redacted: states, counters, timing and identifiers only when operationally necessary. They must not intentionally include message/attachment plaintext, Radio audio, identity private keys, database keys, relationship secrets or reusable pairing capabilities.

Notification events use a narrow cursor/event projection instead of handing platform notification code the entire application snapshot. Privacy policy controls user-visible content before OS delivery.

Android blocks screenshots/screen recording through the secure-window flag by default. `torca-deploy -- ... --privacy allow-capture` is an explicit development override. It changes the capture surface only; it is not a cryptographic/provider mode.

## Current non-guarantees

Torca does **not** currently claim:

- an independent production security audit;
- Signal-style forward secrecy or post-compromise security;
- protection from an intended recipient retaining content;
- protection from a compromised local OS/process/account;
- identical privacy properties across communication providers;
- elimination of timing/traffic analysis;
- guaranteed availability against provider/network blocking, peer downtime or OS suspension;
- long-term archival/migration guarantees for alpha state unless explicitly released as such; or
- a central service with zero metadata visibility when that service must perform commissioning/signaling.

## Security-sensitive change rules

For pairing, peer authentication, encryption, provider bootstrap, protected-secret handling, storage, Radio Mode, notifications, diagnostics or platform privacy changes:

1. identify changed assets/trust boundaries;
2. keep cryptographic/security policy out of Flutter/serialization code;
3. keep private key/relationship-secret bytes out of presentation DTOs/logs;
4. preserve bounded/versioned parsing for untrusted inputs;
5. add deterministic negative/failure coverage, not only happy paths;
6. update the threat model if trust/metadata/guarantees change;
7. update `PRIVACY.md` if data handling/provider network exposure/permissions change; and
8. state validation evidence without upgrading it into an audit claim.

Opening a new communication provider for normal deployment is security-sensitive because it changes commissioning and network-metadata boundaries even when application cryptography is unchanged.

## Reporting vulnerabilities

Do not publish live private keys, pairing capabilities, database keys, relationship secrets, message/attachment plaintext, Radio audio or other real-user secrets in public issues/logs/screenshots.

Use a private contact path available from repository maintainers when possible. If no private disclosure path is available, a public issue should contain only minimal coordination information and no exploit details/secrets. A formal disclosure address/process should be documented only after it actually exists.