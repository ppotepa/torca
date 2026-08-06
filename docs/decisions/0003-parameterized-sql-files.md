# ADR 0003: Parameterized SQL files owned by storage

- Status: accepted
- Date: 2026-08-06

## Context

Hardcoded SQL spread through runtime code is difficult to audit, test and migrate. A heavy ORM would obscure SQLite-specific behavior and SQLCipher requirements.

## Decision

All application SQL is stored in parameterized `.sql` files owned by storage adapters. Migrations, commands and queries have separate roots. SQLite positional parameters use `?1`, `?2`, and so on. SQL files are embedded at compile time where practical.

Raw connections and row types never cross the storage boundary.

## Consequences

- SQL is centrally searchable and auditable;
- query plans and migrations remain explicit;
- mapping code is still required;
- dynamic SQL is exceptional and requires review;
- compile-time inclusion prevents missing production query files.
