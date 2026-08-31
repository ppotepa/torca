# Platform libraries

Platform crates form Torca's outer Rust boundary with Flutter/native hosts and operating systems.

- `torca-contract` maps presentation-safe commands/read models to the public application facade and owns deterministic contract serialization.
- `torca-native` composes the production application/infrastructure graph and exposes the process-owned runtime through native host interfaces.
- `torca-platform` defines shared platform services such as paths, protected secret namespaces, provider/runtime configuration, device/install identity and lifecycle capabilities.
- Windows/Android crates implement genuine OS-specific services.

The contract is a serialization/compatibility boundary, not a second application layer. Business/security policy stays in domains/application.

Production native composition wires the Iroh implementation behind provider-neutral application ports. Concrete provider details must not leak into Flutter/domain/application contracts.

Platform-specific conditional compilation belongs in this layer. Flutter host behavior belongs under `apps/client/flutter/lib/platform`.

See [`../../ARCHITECTURE.md`](../../ARCHITECTURE.md) and [`../../SECURITY.md`](../../SECURITY.md).
