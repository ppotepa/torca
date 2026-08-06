# Data and storage architecture

## Ownership

The storage subsystem owns:

- encrypted database initialization;
- connection and transaction lifecycles;
- migrations;
- SQL loading and parameter binding;
- repository implementations;
- durable outbox and work queues;
- inbound deduplication records;
- projection persistence where required.

Domains own persistence interfaces and semantic data requirements, not table shapes.

## SQL layout

Each storage module keeps SQL in files:

```text
sql/
  migrations/
    0001_initial.sql
  commands/
    insert_message.sql
    mark_message_delivered.sql
  queries/
    get_message.sql
    list_conversation_messages.sql
```

Rules:

- no hardcoded application SQL strings;
- parameters use `?1`, `?2`, ...;
- query, command and migration files are separate;
- SQL files are embedded at compile time when practical;
- dynamic identifier construction is prohibited in normal repositories;
- row mapping is explicit and tested;
- actor and UI code never receive a raw connection.

## Transactions

Required atomic groups include:

- message plus outbox item;
- incoming message plus deduplication record;
- message state plus durable receipt work;
- pairing completion plus command idempotency record and resulting local workflow state.

External network calls must not occur while a database transaction is held.

## Migrations

Migrations are append-only after the first distributed test build. Each migration has a monotonic identifier and is tested from an empty database and from every supported prior release schema.

## Encryption and secrets

The database must use SQLCipher-compatible encryption. Database keys are provided through a platform-protected key provider and are never stored beside the database in plaintext.

## Repository contracts

Repositories return domain values or dedicated persistence DTOs mapped inside the adapter. They do not return SQL rows. Domain errors and storage failures remain distinguishable.
