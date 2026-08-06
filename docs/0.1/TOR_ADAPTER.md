# Tor adapter — Batch 14

Implemented in `torca-transport-tor`:

- deterministic torrc rendering;
- data directory, SOCKS, control port and v3 hidden-service configuration;
- child-process start, readiness polling, exit detection and shutdown;
- hidden-service hostname reading;
- SOCKS5 domain-name connect for onion targets with bounded I/O timeouts.

The adapter owns streams and process lifecycle only. Peer authentication and message semantics remain outside it. Platform packaging must provide a trusted Tor binary and writable runtime directories.
