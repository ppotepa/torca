# Infrastructure libraries

Infrastructure crates provide concrete implementations of application/domain ports.

This layer owns SQLCipher persistence, production cryptographic adapters, Iroh peer/network IO, pairing-service access, encrypted file/attachment handling and concrete communication composition.

Important ownership rules:

- operational SQL belongs to storage infrastructure;
- cryptographic primitives and protected-secret handling stay out of presentation/contract code;
- concrete adapters may depend inward on application/domain interfaces, not the reverse;
- logging/observability should remain redacted or payload-free where possible.

Infrastructure implementation details change frequently, so individual APIs are documented in source/Rustdoc rather than duplicated in per-crate README files.

See [`../../ARCHITECTURE.md`](../../ARCHITECTURE.md) and [`../../SECURITY.md`](../../SECURITY.md).
