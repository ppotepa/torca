# Transport and protocol architecture

## Separation of concerns

- `peer` owns authenticated session semantics and connection state;
- `torca-tor` owns embedded Arti, onion services and stream establishment;
- protocol crates own versioned wire envelopes and codecs;
- messaging owns message semantics; and
- the process runtime coordinates delivery work.

A peer session transports encrypted protocol envelopes. It does not understand Flutter views or database
tables.

## Direct delivery

After pairing, normal contact traffic is:

```text
Client A -> local embedded Tor -> contact onion service -> Client B
```

The rendezvous relay does not participate in direct contact delivery.

## Handshake

A peer connection must prove protocol compatibility, expected remote identity, possession of contact
authorization material, replay-resistant freshness, and negotiated size/feature limits. A connected
stream is not an authenticated contact session until the handshake completes.

## Wire versioning

Every top-level envelope carries a protocol family and version. Domain types are mapped to wire DTOs.
Unknown optional fields may be ignored; unknown required message kinds are rejected explicitly.

Codecs enforce strict upper bounds before allocation. Test vectors cover valid, truncated, oversized,
malformed and unsupported payloads.

## Reliability

Transport delivery and user delivery are separate states:

- stream write accepted;
- peer protocol acknowledgement;
- message accepted by remote runtime;
- delivered receipt; and
- read receipt.

The durable outbox remains authoritative until the required protocol acknowledgement is committed.
Retries reuse stable identifiers.

## Tor lifecycle

The Tor adapter exposes start, ready, degraded, failed and stopping states. It supports cancellation,
bounded startup, health checks, onion publication and clean shutdown. It is an embedded library, not an
external `tor.exe` process. Logs are redacted before entering application diagnostics.
