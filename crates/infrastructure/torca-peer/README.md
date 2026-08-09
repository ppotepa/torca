# torca-peer

## Purpose

Manage authenticated peer sessions independently from the concrete Tor stream provider.

## Owns

- peer session identifiers and lifecycle;
- handshake orchestration;
- expected identity and capability authentication;
- bounded frame read/write loops;
- protocol acknowledgement handling;
- reconnect-relevant session events;
- cancellation and timeout behavior.

## Does not own

Message domain transitions, embedded Tor lifecycle, SQLite outbox queries, rendezvous pairing or Flutter connectivity indicators.

## 0.1 completion

Simulated duplex streams prove authenticated handshake, frame fragmentation, timeout, duplicate acknowledgement and clean reconnect behavior before Tor is integrated.
