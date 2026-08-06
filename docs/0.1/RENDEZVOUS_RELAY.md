# Rendezvous relay — Batch 08

The relay is an ephemeral in-memory broker for opaque pairing blobs.

Implemented:

- versioned request/response vocabulary in `torca-relay-protocol`;
- strict code and 64 KiB blob bounds;
- open, join, push, poll, close and expiry behavior;
- one joiner per slot and bounded per-side queues;
- no database, user directory, private-key material or offline message mailbox.

Network hosting and TLS/onion deployment remain later composition work; relay semantics are independent from client domains.
