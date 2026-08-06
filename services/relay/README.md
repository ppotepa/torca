# Torca rendezvous relay

## Purpose

Temporarily connect two active clients that know the same short-lived invitation code and forward opaque pairing frames between them.

## Owns

- in-memory pairing slots;
- create and join authorization rules;
- slot expiry and bounded resource limits;
- forwarding opaque frames between the two slot participants;
- protocol-level errors and redacted operational metrics;
- graceful connection cleanup.

## Explicit non-goals

The relay is not:

- an account or identity provider;
- a public directory;
- a message relay;
- an offline mailbox;
- a contact-presence service;
- a source of truth for pairing completion;
- a database-backed service in 0.1.

## Security boundary

The relay is untrusted by clients. It may observe timing and slot metadata but must not require plaintext pairing material, private keys, contact capabilities or message content.

## 0.1 completion

Two clients can create, join and close a slot; expiry releases all state; restart removes all active slots; malformed and oversized traffic is bounded and rejected.
