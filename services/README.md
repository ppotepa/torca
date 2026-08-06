# Services

This directory contains server-side deployable units.

Version 0.1 has one service:

- [`relay`](relay/README.md) — ephemeral rendezvous broker for pairing.

Services must remain independent from client application internals and use shared protocol crates only where a wire contract is required.
