# Contributing to Torca

## Branch policy

During the 0.1 build-out, all work lands directly on `main`. Do not create long-lived feature branches or maintain a parallel legacy branch in this repository.

Because `main` is the only active branch:

- every commit must leave the repository internally consistent;
- incomplete implementation must be isolated behind an unused module or feature flag;
- documentation must be updated in the same change that alters architecture or scope;
- destructive rewrites must preserve the current milestone's accepted behavior;
- generated files must be reproducible from committed sources.

## Work unit

A normal work unit should correspond to one roadmap item and contain:

1. the smallest coherent implementation;
2. unit or contract tests appropriate for the layer;
3. relevant documentation changes;
4. an update to `docs/0.1/STATUS.md`;
5. a concise commit message describing the result.

## Dependency discipline

Before adding a dependency between crates, verify that it follows [`docs/architecture/DEPENDENCY_RULES.md`](docs/architecture/DEPENDENCY_RULES.md). Infrastructure must not leak into domains. Flutter and native hosts must not become alternative implementations of application workflows.

## Domain library rules

A mini-domain library should expose a small public API and keep implementation modules private by default. It should own:

- domain value objects and entities;
- commands or operation inputs;
- domain events;
- invariants and state transitions;
- domain-specific errors;
- ports required to execute its use cases.

It should not expose database connections, SQL rows, JSON wire payloads, FFI handles, Flutter models, sockets, or a global application context.

## SQL rules

All SQL must be stored in `.sql` files. Runtime SQL string construction is prohibited except for narrowly reviewed schema tooling. Parameters use SQLite positional placeholders (`?1`, `?2`, ...). Migrations, commands, and queries have separate roots.

## Documentation rules

- Architecture changes require an ADR when they alter a boundary, dependency direction, persistence rule, protocol rule, or deployment model.
- Version-specific work belongs under `docs/0.1`.
- Long-lived design rules belong under `docs/architecture`.
- README files explain ownership and navigation; they must not become untracked alternative roadmaps.

## Commit style

Use short imperative messages with a clear area, for example:

```text
docs: define messaging domain boundary
storage: add transactional outbox schema
pairing: validate invitation expiry
bridge: generate typed command contract
```

## Validation expectation

When code exists, the repository will provide one supported validation entrypoint. A change is not complete until that entrypoint succeeds or the failure is explicitly documented as an environmental limitation.
