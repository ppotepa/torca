# torca-rendezvous-client

## Purpose

Implement the pairing domain's rendezvous port using the versioned relay protocol.

## Owns

- connection to configured relay endpoint;
- create, join, publish, receive, close and cancel operations;
- relay protocol encoding and decoding through the protocol crate;
- timeout, reconnect and redacted diagnostics;
- mapping relay failures into pairing-port errors.

## Does not own

Pairing state transitions, approval policy, relay server state, contact creation or long-term message delivery.

## 0.1 completion

The pairing workflow can switch between an in-memory fake and the real relay client without changing domain code.
