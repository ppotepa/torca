# Security model

This document defines architectural trust boundaries. A detailed threat model will be completed during stabilization.

## Trusted client boundary

The local client owns identity secrets, contact capabilities, conversation state, message plaintext and local history. The operating system and device are assumed trusted enough to run the client; compromise of either is outside guarantees provided solely by Torca.

## Untrusted relay

The pairing relay is untrusted. It may observe timing, connection metadata and pairing slot identifiers. It must not receive message plaintext, private identity keys or long-term contact secrets. It stores active slots only in process memory.

## Tor network

Tor reduces direct network-address exposure but does not guarantee protection against compromised endpoints, global traffic analysis, malicious local software or Tor implementation vulnerabilities.

## Secret classification

Never log or export:

- private identity or signing keys;
- database encryption keys;
- contact capabilities;
- decrypted message bodies;
- decrypted attachment contents;
- full authentication transcripts when they enable replay.

Public identities, redacted identifiers, protocol versions and bounded error categories may be included in diagnostics where necessary.

## Cryptographic boundaries

Domain code requests semantic operations such as sign, verify, seal, open, derive and rotate through narrow ports. Algorithm identifiers and serialized formats are explicit and versioned. Randomness comes from an approved cryptographic provider.

## Input handling

All external inputs are untrusted:

- relay frames;
- peer frames;
- invitation codes;
- attachment metadata;
- database contents after an unclean shutdown;
- generated bridge commands.

Validate lengths before allocation, reject unsupported versions, use bounded queues, avoid panic on malformed data, and make cancellation available for expensive operations.

## Security changes

Changes to identity, key storage, capability authentication, encryption formats, protocol handshakes or relay trust require an ADR and security-focused tests.
