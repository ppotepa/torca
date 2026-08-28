# Iroh is the sole production communication provider

Status: accepted (alpha)

Tor and the unfinished WebRTC adapter are removed from the active product
graph. Iroh is the only production provider; the in-memory provider remains a
test-only contract implementation. Application and domain crates depend only
on `ProviderId`, opaque provider routes, `ProviderRouting`, and
`PeerTransportFactory`.

This is a static composition boundary, not a dynamic plugin loader. A future
provider must implement the neutral provider API and pass the conformance
suite before it can enter the production graph. Existing Iroh profiles remain
`always`, `direct`, and `local` (the latter is lab-only).

The storage epoch is bumped so pre-Iroh-only profiles are rejected explicitly
instead of silently interpreting legacy provider data.
