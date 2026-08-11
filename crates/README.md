# Rust workspace

Torca is a modular Rust application organized by architectural role rather than by operating system.

```text
foundation/       stable low-level primitives
domains/          product models and invariants
protocol/         bounded wire/protocol contracts
application/      use cases, ports, runtime coordination and read models
infrastructure/   SQLCipher, crypto, Arti, files and network adapters
platform/         presentation contract, native composition and OS services
```

The root [`Cargo.toml`](../Cargo.toml) is the source of truth for active workspace members. Directory README files describe only layer responsibilities; individual crate APIs should be documented in source/Rustdoc to avoid maintaining a second copy of rapidly changing implementation details.

Dependency direction is checked by `scripts/modules/Torca.ArchitecturePolicy.ps1`. In particular, domains/protocols do not depend on upper implementation layers, and application code does not depend on infrastructure/platform implementations.

See [`../ARCHITECTURE.md`](../ARCHITECTURE.md) for the maintained system model.