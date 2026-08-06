# Contributing to Torca

## Start of work

Read [`0.1_PROGRESS.md`](0.1_PROGRESS.md) before changing the repository. It is the only live status/handoff document and records current implementation, validation evidence, release gates and the exact next action.

All 0.1 work currently lands directly on `main`. Every commit must therefore leave the repository internally coherent; incomplete production behavior must remain explicit and must not be presented as validated.

## Developer workflow

Developers use only three root workflows:

```powershell
./scripts/build.ps1
./scripts/run.ps1
./scripts/deploy.ps1
```

- `build` is the correctness/build gate. It owns formatting, code generation, release/architecture checks, dependency resolution, Rust check/Clippy/tests, Flutter analysis/tests and optional platform compilation.
- `run` is the fast inner loop. It prepares the generated contract and shared native runtime, then launches Flutter with hot reload.
- `deploy` performs a strict release build, packages artifacts and generates SHA-256 checksums.

Do not add a new public script for formatting, validation, codegen, packaging, lock refresh or platform bootstrap. Add such behavior to `tools/build/Torca.Build.psm1` and expose it only through one of the three workflows.

CI uses the same build path:

```powershell
./scripts/build.ps1 -Target check -CI
```

## One-client rule

Torca has one Flutter client source. Windows and Android are target platforms, not separate application implementations.

Responsive differences belong in the shared Flutter widget tree. OS-specific Kotlin/C++ is allowed only for true platform capabilities such as protected key storage, notifications, lifecycle integration and tray behavior. Product workflows and state machines remain in Rust.

## Dependency discipline

Before adding a Rust dependency, verify that it follows [`docs/architecture/DEPENDENCY_RULES.md`](docs/architecture/DEPENDENCY_RULES.md).

- foundation stays dependency-light;
- domains may depend on foundation and approved domain contracts;
- application coordinates domains;
- infrastructure implements inward-defined ports;
- bridge/native/presentation sit outside application/domain code;
- domains never depend on infrastructure, Flutter or FFI.

## Domain library rules

A mini-domain owns value objects, entities, commands/inputs, events, invariants, state transitions, errors and ports required by its use cases. It must not expose database connections, SQL rows, FFI handles, Flutter models, sockets or a global application context.

## SQL rules

All business SQL lives in `.sql` files under the SQLCipher storage crate. Runtime SQL string construction is prohibited except narrowly reviewed connection/key bootstrap operations that cannot be parameterized normally.

Commands, queries and migrations have separate roots and use positional parameters (`?1`, `?2`, ...).

## Native boundary rules

- `torca-native` is the narrowly reviewed unsafe C ABI boundary.
- ABI functions expose primitive arguments and bridge snapshots/results, not Rust domain layouts.
- private key material never crosses into Dart.
- native runtime failure is surfaced as an error; production never silently falls back to the memory gateway.

## Work unit

A coherent work unit should contain:

1. the smallest complete implementation;
2. appropriate unit/contract/integration tests;
3. relevant documentation updates;
4. an update to `0.1_PROGRESS.md` when status, architecture, defects or gates change;
5. exact owner validation evidence when available.

A file existing is not completion. A release gate closes only when its behavior is composed and validated at the relevant layer.
