# Torca 0.1 threat model

## Assets

Installation identity private keys, contact capabilities, message and attachment plaintext, MLS/session state, local history and onion-service private material.

## Trust boundaries

- The local device and its protected key storage are trusted only while uncompromised.
- The rendezvous relay is untrusted and receives only bounded opaque pairing blobs.
- Tor and the network are untrusted transports.
- Remote contacts are authenticated peers, not trusted execution environments.
- Flutter and platform hosts submit typed commands but do not own cryptographic or messaging state.

## Required controls

- No private key material in domain models, logs or bridge snapshots.
- Capability and identity binding before peer session readiness.
- Fresh challenge and proof verification during handshake.
- Strict frame and blob size limits before allocation.
- Transactional outbox and stable IDs for retry/idempotency.
- Inbound envelope and receipt deduplication.
- SQLCipher-compatible local persistence with platform-protected database keys.
- Authenticated attachment encryption and atomic file replacement.
- Redacted diagnostics and disabled Android backup.

## Out of scope guarantees

Torca 0.1 cannot protect against endpoint compromise, malicious operating systems, screen/keylogging, global traffic analysis, denial of service, compromised Tor software or a malicious contact redistributing plaintext.
