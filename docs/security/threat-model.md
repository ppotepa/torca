# Torca threat model

Torca is alpha software and has not received an independent production security audit. This document describes the active Iroh-only architecture.

## Scope and assets

The model covers the client, encrypted local storage, Iroh direct/relay transport, pairing service exchange, authenticated peer sessions, attachments, Radio Mode, the Flutter/native boundary, platform lifecycle and diagnostics. Assets include relationship secrets, invitation capabilities, message and attachment plaintext, encrypted history, delivery state and network metadata.

## Trust boundaries

### Local client and platform

Rust application code owns workflow and security state. Flutter is presentation only. Windows/Android provide lifecycle, protected storage, permissions and capture controls. A compromised OS or user account is outside the application protection boundary.

### Iroh transport and pairing service

Iroh endpoints and any relay/discovery service are transport infrastructure, not proof of peer identity. They may observe connection timing, routing metadata and pairing-service operations, and may deny service. Pairing payloads, peer handshakes and application messages remain cryptographically authenticated and encrypted by Torca.

Direct paths can expose network-location metadata permitted by the active Iroh profile; relay paths change that exposure but do not provide Tor anonymity. Privacy claims must therefore distinguish content confidentiality from network-location privacy.

### Remote peer

A paired peer is authenticated as the holder of the approved relationship capability. It can copy delivered content or record media outside Torca; local retention wishes cannot constrain a recipient.

## Controls

- Pairing uses bounded, capability-authorized service sessions and explicit approval.
- Persisted contacts contain opaque provider routes and capability identifiers; no endpoint is trusted as identity.
- Peer handshake binds credentials, capability and transcript before application traffic.
- Message, receipt and attachment state machines are durable and replay-safe.
- Iroh route generations reject stale routes and accept only monotonic advertisements.
- Diagnostics redact endpoint bytes, addresses and message content.
- Source policy keeps provider-specific Iroh code behind the provider boundary.

## Availability and abuse

Reconnect demand is event-driven and durable; idle peers may close without losing queued work. Bounded retries, deadlines, frame-size limits and storage quotas mitigate resource exhaustion. Transport failure is surfaced separately from user-facing availability.

## Review checklist

Any future provider must satisfy the provider conformance suite and document its direct/relay metadata exposure before production use. Changes to pairing, credentials, route migration or protected storage require focused security review and updated tests.
