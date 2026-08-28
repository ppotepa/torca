# Torca architecture

Torca is a modular Rust application behind one Flutter client. Presentation
renders application read models; durable state, identity, cryptography, pairing,
delivery, background lifecycle and network work remain in Rust.

The application boundary is provider-neutral: `ProviderId`, opaque
`ProviderRoute`, `ProviderRouting` and `PeerTransportFactory` are the only
communication contracts visible above infrastructure. The sole production
provider is Iroh; Memory is test-only. No dynamic plugin loader or provider
selection UI exists.

Iroh owns endpoint identity, route generations, incoming routing, pairing-service
transport, authenticated peer byte transport and relay/direct path selection.
Peer-link owns the authenticated session and application protocols. Pairing
crypto, message encryption, receipts, attachments, presence and persistence are
provider-independent.

Runtime waiting is event/deadline driven. Idle presentation polling must not
create periodic network or CPU work. Native composition creates one process-owned
runtime and one Iroh provider component set. Platform adapters own only OS APIs.

SQLite stores opaque provider endpoints keyed by `ProviderId`; no provider
specific onion columns or fallback paths exist. A future provider must implement
the neutral API and pass `torca-provider-conformance` before production use.
