# Transport and protocol architecture

## Separation of concerns

- `peer` owns authenticated session semantics and connection state;
- `transport-tor` owns Tor process, onion service and stream establishment;
- protocol crates own versioned wire envelopes and codecs;
- messaging owns message semantics;
- the ClientEngine coordinates delivery work.

A peer session transports encrypted protocol envelopes. It does not understand Flutter views or database tables.

## Direct delivery

After pairing, normal contact traffic is:

```text
Client A -> local Tor -> contact onion service -> Client B
```

The rendezvous relay does not participate.

## Handshake

A peer connection must prove:

- protocol compatibility;
- expected remote identity;
- possession of the contact capability or equivalent authorization material;
- freshness sufficient to reject simple replay;
- negotiated size and feature limits.

A connected socket is not an authenticated contact session until the handshake completes.

## Wire versioning

Every top-level envelope carries a protocol family and version. Domain types are mapped to wire DTOs. Unknown optional fields may be ignored; unknown required message kinds are rejected explicitly.

Codecs enforce strict upper bounds before allocation. Test vectors cover valid, truncated, oversized, malformed and unsupported payloads.

## Reliability

Transport delivery and user delivery are separate states:

- stream write accepted;
- peer protocol acknowledgement;
- message accepted by remote engine;
- delivered receipt;
- read receipt.

The durable outbox remains authoritative until the required protocol acknowledgement is committed. Retries reuse stable identifiers.

## Tor lifecycle

The Tor adapter exposes start, ready, degraded, failed and stopping states. It supports cancellation, bounded startup, health checks, onion publication and clean shutdown. Process logs are redacted before entering application diagnostics.
