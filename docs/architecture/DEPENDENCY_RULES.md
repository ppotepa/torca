# Dependency rules

## Allowed direction

```text
foundation
    ^
    |
domains
    ^
    |
application
    ^
    |
platform bridge and deployable compositions

infrastructure and protocol implement ports used from the side;
they do not become parents of domain code.
```

A more concrete view:

- foundation may depend only on the standard library and narrowly approved low-level crates;
- domain crates may depend on foundation and explicitly reviewed domain contract crates;
- application crates may depend on domains and foundation;
- infrastructure crates may depend on domain/application port definitions, protocol and foundation;
- protocol crates may depend on foundation wire-safe primitives but not application workflows;
- platform crates may depend on application public contracts and generated bridge tooling;
- deployable apps compose all required implementations.

## Forbidden dependencies

- domain -> SQLite, SQLCipher or ORM library;
- domain -> Tor, socket or HTTP client;
- domain -> Flutter, Dart, JNI, Win32 or FFI representation;
- domain -> concrete clock, filesystem or random-number provider;
- UI -> storage implementation;
- UI -> peer or Tor implementation;
- storage -> Flutter projection models;
- relay -> client domain implementation;
- protocol -> database row types;
- one domain -> another domain's concrete repository implementation.

## Cycles

Cargo dependency cycles are prohibited. Conceptual cycles are resolved by:

1. moving a truly shared value type into foundation;
2. introducing a narrow port owned by the consumer;
3. coordinating the interaction in application code;
4. publishing an event consumed by an application handler.

Do not create a generic `common` crate to hide unclear ownership.

## Enforcement

The workspace will maintain a dependency-policy test or script. New dependencies must be reviewed against this document and reflected in component READMEs.
