# Torca 0.1 definition of done

Torca 0.1 is complete only when all mandatory conditions below are satisfied.

## Product journey

Two fresh installations can:

1. initialize local identities;
2. create and join a short-lived pairing invitation;
3. explicitly approve each other;
4. create matching verified contacts and direct conversations;
5. exchange encrypted text messages directly through Tor;
6. recover queued delivery after temporary disconnection or process restart;
7. exchange delivered and read receipts;
8. restart and restore consistent local state;
9. perform the same journey on Windows and Android.

## Architecture

- Domain libraries contain no infrastructure or UI dependencies.
- Application workflows are coordinated by the ClientEngine actor.
- UI state is derived from engine projections.
- Wire types are separate from domain types and explicitly versioned.
- Cross-domain effects are visible in application handlers.
- Dependency checks enforce the approved direction.

## Data integrity

- Every mutating command has a stable `command_id`.
- Retrying a command does not duplicate its effect.
- Message and outbox writes are atomic.
- Incoming envelope deduplication is durable.
- Migrations are ordered, transactional where possible, and tested from an empty database.
- Database access is contained inside the storage implementation.
- All SQL is parameterized and stored in `.sql` files.

## Security and privacy

- Private keys, capabilities and plaintext message bodies are absent from logs and diagnostics.
- Peer sessions authenticate the expected identity and capability.
- Relay storage is ephemeral and contains no message history.
- Message payloads are not readable by the relay.
- Sensitive local data is encrypted at rest.
- Threat model and known limitations are current.

## Reliability

- Connection interruption does not lose accepted outbound messages.
- Duplicate and reordered transport events do not corrupt state.
- Shutdown and restart are safe during pairing and message delivery.
- Retry loops are bounded, cancellable and observable.
- Platform lifecycle transitions do not create competing engine instances.

## Quality

- Unit tests cover domain invariants and invalid transitions.
- Contract tests cover storage ports, codecs and bridge contracts.
- Integration tests use two independent engine instances.
- End-to-end tests cover the primary product journey on both supported platforms.
- Repository validation succeeds from a clean checkout with documented prerequisites.

## Release material

- Version is consistent across Rust, Flutter and packaging metadata.
- Installation, diagnostics and recovery procedures are documented.
- Known limitations are explicit.
- Test artifacts are reproducible and integrity-verifiable where supported.
- No unresolved critical security, corruption or data-loss defect remains.
