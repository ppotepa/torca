# Rust workspace

Torca is a modular Rust application organized by architectural role rather than by operating system.

```text
foundation/       dependency-light primitives
protocol/         bounded external/wire contracts
domains/          product models and invariants
application/      use cases, ports, runtime coordination and policy
infrastructure/   SQLCipher, crypto, files and concrete provider/network adapters
platform/         presentation/native contract, composition and OS services
```

The root [`Cargo.toml`](../Cargo.toml) is the source of truth for active workspace members. Directory README files describe layer responsibilities only; individual crate APIs belong in source/Rustdoc to avoid maintaining a second rapidly changing inventory.

Dependency direction is enforced by repository policy checks. In particular, domain/protocol code stays independent of upper implementation layers and application code does not import infrastructure/platform implementations.

Iroh-specific implementation belongs in infrastructure/platform composition behind provider-neutral application ports. Memory is test-only; retired Tor/WebRTC paths are not part of the active production graph.

See [`../ARCHITECTURE.md`](../ARCHITECTURE.md) for the canonical system model.
