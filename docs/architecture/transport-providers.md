# Communication provider boundary

The alpha product has one production provider: Iroh. Application and domain
crates depend only on `ProviderId`, opaque `ProviderRoute`, `ProviderRouting` and
`PeerTransportFactory`. They do not import Iroh, QUIC, relay implementation
details or network addresses.

`torca-transport-iroh` owns endpoint identity, route freshness/migration,
pairing-service transport, authenticated peer sessions and direct/relay path
selection. `torca-transport-memory` is test-only and exists to exercise the
neutral contract deterministically.

There is no provider selector, dynamic plugin loader, Tor implementation or
WebRTC placeholder in the production graph. A future provider must implement
the neutral API and pass `torca-provider-conformance` before it can be enabled.
