# torca-tor

## Purpose

Own the embedded Arti integration used by Torca: bootstrap, persistent cache, onion-service lifecycle,
incoming/outgoing onion streams, health reporting and controlled shutdown.

## Owns

- all direct Arti imports and client configuration;
- Tor bootstrap and progress/health events;
- onion service publication and stream handles; and
- redacted Tor diagnostics and shutdown.

## Does not own

Pairing policy, contact state, message semantics, relay protocol rules, Flutter state, SQL persistence or
platform secret-store implementation.

Consumers use Torca-owned types rather than Arti types. `torca-tor` is an embedded library; no external
Tor executable or SOCKS process is part of the production client.
