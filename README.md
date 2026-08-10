# Torca

Torca is an experimental privacy-focused messenger built around local identities, direct peer-to-peer communication over Tor, encrypted local state, and explicit contact pairing.

The project is under active alpha development. The codebase changes quickly, so this README and the linked architecture/security documents describe stable responsibilities and design direction rather than every type, protocol field, timeout, or release-specific migration.

## Product direction

Torca aims to become a practical everyday private messenger without turning normal conversations into traffic through a central message service.

The current direction is:

- one responsive Flutter client shared across supported desktop and mobile targets;
- one Rust application/runtime implementation for identity, pairing, messaging, persistence, security, Tor connectivity, delivery and background work;
- direct authenticated peer communication through Tor onion services after contacts are paired;
- local encrypted history and durable offline/retry state;
- explicit trust and contact verification instead of a central account directory;
- a small untrusted rendezvous relay used for pairing, not for normal messages or message history;
- platform-specific code only for genuine operating-system capabilities such as protected secrets, lifecycle, notifications, window integration and secure display behavior;
- architecture rules that are simple enough to maintain and strict enough to prevent mobile/desktop business logic from diverging.

Torca is not trying to reproduce every Telegram or WhatsApp feature at once. Reliability, privacy, predictable behavior and maintainable cross-platform development come first.

## System shape

```text
Flutter UI
    |
    v
EngineGateway
    |
    v
Torca presentation contract
    |
    v
native C / platform boundary
    |
    v
Client application facade
    |
    +--> single-writer client engine
    +--> process runtime
            |
            +--> embedded Tor (Arti)
            +--> pairing / rendezvous
            +--> authenticated peer links
            +--> durable message/control delivery
            +--> attachments
            +--> connectivity, probes and diagnostics
    |
    v
SQLCipher repositories + protected secret stores
```

Flutter owns presentation, navigation and local UI preferences. Rust owns durable application state, identifiers, security-sensitive workflows, network state and background work. The platform contract exposes presentation-safe commands and read models; private key material and peer secrets do not belong in Flutter DTOs.

Tor runs in-process through Arti. Normal peer traffic is carried directly between onion endpoints. The relay exists only to help two active clients establish an explicitly approved relationship.

See [ARCHITECTURE.md](ARCHITECTURE.md) for the medium-depth system description.

## Repository map

```text
apps/client/flutter/      shared responsive client UI
crates/foundation/        stable low-level primitives
crates/domains/           business/domain models and invariants
crates/protocol/          bounded network/wire contracts
crates/application/       use cases, runtime coordination and read models
crates/infrastructure/    storage, crypto, Tor and concrete adapters
crates/platform/          presentation contract, native ABI and OS adapters
services/relay/           ephemeral pairing rendezvous service
scripts/                  public development/build/deploy entrypoints
tools/                    build support and generated-contract tooling
tests/                    cross-component integration tests
third_party/              narrowly scoped vendored/upstream patches
```

The root `Cargo.toml` is the source of truth for active Rust workspace members.

## Development

Public workflows are intentionally small:

```powershell
# Source, Rust and Flutter validation path
./scripts/build.ps1 -Target check

# Run the shared client
./scripts/run.ps1 -Target windows
./scripts/run.ps1 -Target android

# Build/install/run through the deployment orchestrator
./scripts/deploy.ps1 -Target windows
./scripts/deploy.ps1 -Target android
```

Native builds require the configured Torca stack/relay endpoint used by the build orchestration. Platform assets and generated contracts are handled by the scripts rather than by manual per-platform procedures.

Before contributing, read [CONTRIBUTING.md](CONTRIBUTING.md).

## Documentation

The maintained documentation is deliberately small:

- [ARCHITECTURE.md](ARCHITECTURE.md) — system boundaries and major flows;
- [SECURITY.md](SECURITY.md) — security posture, guarantees and non-guarantees;
- [docs/security/threat-model.md](docs/security/threat-model.md) — assets, trust boundaries and threats;
- [CONTRIBUTING.md](CONTRIBUTING.md) — development and architecture rules;
- [ROADMAP.md](ROADMAP.md) — product/engineering direction, not a release checklist;
- [docs/README.md](docs/README.md) — documentation policy and index.

Historical implementation plans and version-specific audits live in Git history rather than acting as parallel sources of truth.

## Security status

Torca is security-sensitive alpha software and has not been independently audited. Current code uses encrypted local storage, protected secret stores, authenticated application encryption and Tor for network routing, but the present peer-message key scheme does **not** claim Signal-style forward secrecy or post-compromise security. See [SECURITY.md](SECURITY.md) before making security claims or deploying Torca for high-risk use.

## License

Torca is licensed under AGPL-3.0-or-later.