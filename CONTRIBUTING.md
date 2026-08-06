# Contributing to Torca

## Start-of-work protocol

Every developer or agent must begin by reading [`0.1_PROGRESS.md`](0.1_PROGRESS.md). It identifies the active milestone, current batch, completed work, blockers, validation evidence and the exact next action.

Before changing the repository:

1. verify that the local or connector view matches the current `main` branch;
2. confirm that the intended work belongs to the current batch;
3. read the linked architecture, scope and ADR documents;
4. avoid repeating work already marked complete;
5. preserve any recorded validation and accepted behavior.

If repository state contradicts the progress document, investigate the mismatch and correct the document before beginning unrelated work.

## Branch policy

During the 0.1 build-out, all work lands directly on `main`. Do not create long-lived feature branches or maintain a parallel legacy branch in this repository.

Because `main` is the only active branch:

- every commit must leave the repository internally consistent;
- incomplete implementation must be isolated behind an unused module or feature flag;
- documentation must be updated in the same change that alters architecture or scope;
- destructive rewrites must preserve the current milestone's accepted behavior;
- generated files must be reproducible from committed sources.

## Work unit

A normal work unit should correspond to one roadmap or batch item and contain:

1. the smallest coherent implementation;
2. unit, contract or integration tests appropriate for the layer;
3. relevant documentation changes;
4. an update to [`0.1_PROGRESS.md`](0.1_PROGRESS.md);
5. exact validation commands and results recorded in the handoff log;
6. a concise commit message describing the result.

A work unit is not complete when files merely exist. It is complete when the intended behavior works at the relevant layer, validation evidence is recorded, and the next action is unambiguous.

## Progress document rules

`0.1_PROGRESS.md` is the only live status checklist. Do not create another progress, TODO, status or handoff document.

Every coherent implementation commit must update, where applicable:

- the last-updated date;
- current milestone and batch;
- implemented and not-implemented summaries;
- current work package checkboxes;
- batch queue state;
- milestone progress;
- blockers and risks;
- validation evidence;
- the handoff log;
- one exact next action.

Do not mark an item complete without evidence. If validation cannot run for an environmental reason, record the exact limitation and leave the item incomplete unless its completion criteria do not require that validation.

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
- Version-specific design and acceptance criteria belong under `docs/0.1`.
- Long-lived design rules belong under `docs/architecture`.
- Live execution status belongs only in `0.1_PROGRESS.md`.
- README files explain ownership and navigation; they must not become untracked alternative roadmaps or progress lists.

## Commit style

Use short imperative messages with a clear area, for example:

```text
docs: define messaging domain boundary
storage: add transactional outbox schema
pairing: validate invitation expiry
bridge: generate typed command contract
```

## Validation expectation

When code exists, the repository will provide one supported validation entrypoint. A change is not complete until that entrypoint succeeds or the failure is explicitly documented as an environmental limitation in `0.1_PROGRESS.md`.
