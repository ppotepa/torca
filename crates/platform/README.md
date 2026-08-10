# Platform libraries

Platform crates form Torca's outer boundary with Flutter/native hosts and operating systems.

- `torca-contract` maps presentation-safe commands/read models to the public application facade and owns deterministic contract serialization.
- `torca-native` composes the production application/infrastructure graph and exposes the process-owned runtime through native host interfaces.
- `torca-platform` defines shared platform services such as paths, protected secret namespaces, relay configuration, device/install identity and lifecycle capabilities.
- Windows/Android crates implement genuine OS-specific services.

The contract is a serialization/compatibility boundary, not a second application layer. Business/security policy should remain in domains/application.

Platform-specific conditional compilation belongs under this layer. Flutter platform behavior belongs under `apps/client/flutter/lib/platform`.

See [`../../ARCHITECTURE.md`](../../ARCHITECTURE.md) and [`../../SECURITY.md`](../../SECURITY.md).