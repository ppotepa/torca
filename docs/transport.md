# Communication transport

Torca keeps application protocol and network reachability separate. The
application/domain layers consume provider-neutral routes and byte-stream
ports; infrastructure owns the concrete transport.

## Current provider matrix

| Provider | Production use | Pairing bootstrap | Messages | Attachments | Radio |
| --- | --- | --- | --- | --- | --- |
| Iroh | sole production provider | direct QR/full-link route | yes | yes | yes |
| Memory | test double only | in-memory fixture | test-only | test-only | no |

Iroh owns endpoint identity, route generations, incoming routing, pairing
transport, authenticated peer byte transport and direct/relay path selection.
Memory exists to make application and conformance tests deterministic; it is
not a production composition.

## Provider-neutral contract

The application boundary exposes only stable provider identity, opaque routes,
provider routing metadata and `PeerTransportFactory` byte-stream ports. The
native composition is the single point that constructs Iroh. Higher layers do
not depend on QUIC, relay protocol details or provider-specific endpoint
formats.

Provider-specific deployment configuration is interpreted only at provider
composition. An Iroh route is persisted as opaque data keyed by provider and
relationship, with no Tor-era onion column or fallback vocabulary in the
application contract.

## Iroh lifecycle

The Iroh component binds a persisted endpoint identity, publishes the route
needed for pairing, accepts incoming streams and dials persisted peer routes.
Route generations prevent stale discovery data from being treated as fresh;
the peer-link handshake remains authoritative before application traffic is
accepted.

Direct Iroh connectivity can expose network-location metadata. Relay paths,
when selected by Iroh, improve reachability but do not provide anonymity.
Transport privacy must therefore be described separately from message
encryption and authentication.

## Shared behavior above transport

The selected transport does not redefine contact identity, peer
authentication, application-layer encryption, durable queues, receipts,
attachments, conversation persistence, contact verification or Flutter
navigation. A transport changes reachability and commissioning, not the
meaning of a Torca relationship or message.

## Adding another provider

Before a future provider can enter production composition it must implement
the neutral byte-stream and route ports, provide lifecycle and pairing
bootstrap events, wire platform composition without leaking provider types,
and pass `torca-provider-conformance` plus the platform/device evidence
required for the release claim. Until then, Memory remains the only test
double and Iroh remains the only production provider.

See [`ARCHITECTURE.md`](../ARCHITECTURE.md) for layer ownership and
[`testing.md`](testing.md) for evidence language.
