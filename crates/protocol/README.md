# Protocol libraries

Protocol libraries define versioned external representations. They do not define product invariants or persistence shapes.

Planned components:

- [`torca-wire`](torca-wire/README.md) — framing and shared envelope primitives;
- [`torca-peer-protocol`](torca-peer-protocol/README.md) — authenticated peer messages;
- [`torca-pairing-protocol`](torca-pairing-protocol/README.md) — opaque pairing payload contracts;
- [`torca-relay-protocol`](torca-relay-protocol/README.md) — client/relay rendezvous messages.

Every codec has explicit size limits, unsupported-version behavior and committed test vectors.
