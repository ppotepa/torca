# Peer protocol — Batch 12

Implemented:

- handshake hello/ack vocabulary;
- expected identity and capability binding;
- bounded freshness window and challenge nonce;
- external proof-verifier port;
- encrypted data, protocol ack, ping and pong payloads;
- strict proof/data limits and deterministic binary codec;
- protocol acknowledgements kept separate from delivery/read receipts.

Outer framing remains `torca-wire`; cryptographic proof construction remains the production crypto provider's responsibility.
