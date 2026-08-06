# Durable delivery storage — Batch 11

Implemented storage contracts and SQL schema for:

- atomic message plus outbox insertion;
- stable command-id deduplication;
- due-work claiming, rescheduling and completion;
- inbound envelope deduplication;
- idempotent receipt persistence;
- deterministic conversation message ordering.

The in-memory durable store models transaction semantics. GAP-002 remains open until these contracts are backed by the concrete SQLCipher-compatible driver.
