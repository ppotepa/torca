# Rust libraries

Torca uses focused Rust libraries grouped by architectural role.

- [`foundation`](foundation/README.md) — stable low-level value types.
- [`domains`](domains/README.md) — mini-domain models and invariants.
- [`application`](application/README.md) — workflows, engine and projections.
- [`infrastructure`](infrastructure/README.md) — concrete adapters.
- [`protocol`](protocol/README.md) — versioned wire contracts.
- [`platform`](platform/README.md) — generated bridge and native-facing contracts.

A new crate requires a clear owner, public contract, dependency direction and independent reason to change. Do not introduce crates named `common`, `helpers`, `misc`, `manager` or `new-runtime`.
