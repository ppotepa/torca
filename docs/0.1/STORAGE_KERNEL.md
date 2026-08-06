# Storage kernel — Batch 05

The storage kernel embeds SQL at compile time and executes migrations through an injected SQLite-compatible backend.

## Implemented

- strict migration ordering and database-too-new protection;
- transactional migration application with rollback;
- trusted bootstrap PRAGMAs;
- separate `migrations`, `commands` and `queries` roots;
- positional SQLite parameters only;
- foundation and identity schemas;
- compile-time identity statement catalog;
- in-memory backend for transaction-order tests.

## Remaining production work

A concrete SQLCipher/SQLite driver is intentionally not faked. It must implement `StorageBackend` and later repository execution interfaces using a reviewed database dependency.
