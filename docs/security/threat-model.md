# Torca 0.2 threat model

## Assets

Installation identity private keys, protected pairwise peer secrets, contact capabilities, message and attachment plaintext, local encrypted history and onion-service private material.

## Trust boundaries

- The local device and its protected key storage are trusted only while uncompromised.
- The rendezvous relay is untrusted and receives only bounded opaque pairing blobs.
- Tor and the network are untrusted transports.
- Remote contacts are authenticated peers, not trusted execution environments.
- Flutter and platform hosts submit typed commands but do not own cryptographic or messaging state.

## Required controls

- No private key or pairwise secret material in domain models, logs or bridge snapshots.
- Capability and identity binding before peer session readiness.
- Fresh challenge and proof verification during handshake and transcript-bound pairing approval.
- Strict frame and blob size limits before allocation.
- Transactional outbox and stable IDs for retry/idempotency.
- Inbound envelope and receipt deduplication.
- SQLCipher local persistence with platform-protected database and identity keys.
- Authenticated message and attachment encryption with integrity checks.
- Safety Number verification is local; a previously verified remote identity changing becomes a distinct security state and new sends are blocked until the new identity is explicitly verified.
- Android and Windows presentation hosts request OS capture protection for private application content.
- Controlled plaintext attachment exports are temporary and use a bounded cleanup namespace; explicit Save As files remain user-owned.
- Redacted diagnostics and disabled Android backup.

## Current message-key guarantee

Torca 0.2 derives a pairwise secret during authenticated pairing and uses that protected secret with fresh nonces and authenticated encryption for peer payloads. The current 0.2 transport does **not** implement MLS or a Double Ratchet-style per-message key evolution mechanism. Therefore 0.2 does not claim cryptographic forward secrecy or post-compromise security for message history. Adding those guarantees requires a reviewed standard ratchet/MLS design; Torca must not invent an ad-hoc ratchet.

## Out of scope guarantees

Torca 0.2 cannot protect against endpoint compromise, malicious operating systems, screen/keylogging outside supported capture controls, global traffic analysis, denial of service, compromised Tor software, or a malicious contact redistributing plaintext. OS capture protection is defense in depth and must not be described as protection against a compromised endpoint.
