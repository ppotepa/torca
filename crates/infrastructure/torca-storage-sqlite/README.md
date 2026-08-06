# torca-storage-sqlite

## Purpose

Implement Torca persistence ports using SQLite with SQLCipher-compatible encrypted storage.

## Owns

- connection and transaction management;
- migration runner;
- compile-time SQL file loading;
- repository implementations;
- command-id deduplication;
- transactional outbox;
- inbound envelope deduplication;
- persistence row mapping;
- database health and recovery diagnostics.

## SQL layout

```text
sql/
  migrations/
  commands/
  queries/
```

All statements use positional parameters. Raw connections remain private.

## Does not own

Domain state transitions, retry timing, peer sessions, platform key storage or Flutter models.

## 0.1 completion

Identity, pairing, contacts, conversations, messages, receipts and durable work survive restart with tested transaction boundaries and repeatable migrations.
