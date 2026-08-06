# Test strategy

## Domain tests

Pure, fast tests cover entities, value objects, invariants, valid transitions, invalid transitions and idempotent command behavior. Domain tests use in-memory ports and no database or network.

## Contract tests

Every infrastructure adapter is tested against the contract of its port. Examples include repository behavior, transaction atomicity, key-provider behavior, protocol codecs and generated bridge compatibility.

## Integration tests

Integration tests compose multiple real components:

- storage with domain/application workflows;
- two engines with in-memory rendezvous;
- two engines with simulated peer streams;
- relay service with real protocol clients;
- Tor adapter with controlled local infrastructure where available.

## End-to-end tests

End-to-end tests cover the primary journey on Windows and Android. They verify user-visible outcomes and restart/reconnect behavior rather than internal implementation details.

## Failure injection

Required failure points include:

- process stop after domain mutation before delivery;
- database failure before and after commit;
- duplicate inbound envelope;
- delayed, fragmented and reordered peer frames;
- relay disconnect during pairing;
- Tor startup timeout and restart;
- platform suspend during active delivery.

## Test data

Protocol test vectors and migration fixtures are committed. Secrets used in tests are synthetic and clearly marked. Tests must not depend on public relays or uncontrolled external services.

## Validation entrypoint

The workspace will expose one documented command that runs formatting, static checks, unit tests, contract tests and dependency-policy checks. Platform-specific suites may be separate but are invoked by the same top-level script when prerequisites are available.
