# torca-transport-tor

## Purpose

Implement stream establishment and inbound listening through a managed local Tor process.

## Owns

- Tor binary discovery and configuration;
- process startup, readiness and shutdown;
- SOCKS client connection;
- onion service creation and publication;
- inbound stream acceptance;
- health state and redacted process diagnostics;
- bounded restart policy.

## Does not own

Peer authentication, message envelopes, retry queues, contact state or UI status wording.

## 0.1 completion

Two independent test clients publish onion endpoints and exchange authenticated peer bytes through Tor, including recovery after controlled process restart.
